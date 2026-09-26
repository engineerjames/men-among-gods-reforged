//! Sprite texture cache backed by the client's graphics zip, decoded on background threads.

use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use zip::ZipArchive;

/// Upper bound on background sprite decoder threads.
const MAX_DECODE_WORKERS: usize = 4;

/// A decode request: sprite id plus its zip entry name.
type DecodeJob = (usize, String);
/// A decode result: sprite id plus the decoded image or an error message.
type DecodeResult = (usize, Result<egui::ColorImage, String>);

/// Channels to the decoder thread pool.
struct DecodeWorkers {
    jobs: Sender<DecodeJob>,
    results: Receiver<DecodeResult>,
}

/// Lazily decodes sprites from a graphics zip into egui textures.
///
/// [`Self::texture_for`] never blocks on I/O or PNG decoding: unknown sprites
/// are queued to worker threads (each holding its own open archive) and appear
/// on a later frame. Workers request a repaint whenever a sprite finishes.
pub(crate) struct GraphicsZipCache {
    zip_path: PathBuf,
    entries: HashMap<usize, String>,
    textures: HashMap<usize, egui::TextureHandle>,
    in_flight: HashSet<usize>,
    failed: HashMap<usize, String>,
    workers: Option<DecodeWorkers>,
}

impl GraphicsZipCache {
    /// Index the sprites in a graphics zip; decoder threads start on first use.
    ///
    /// # Arguments
    ///
    /// * `zip_path` - Archive whose entries are named `<sprite_id>.<ext>`.
    ///
    /// # Returns
    ///
    /// * The cache, or an error when the archive can't be opened or read.
    pub(crate) fn load(zip_path: PathBuf) -> Result<Self, String> {
        let archive = open_archive(&zip_path)?;

        let entries: HashMap<usize, String> = archive
            .file_names()
            .filter(|name| !name.ends_with('/'))
            .filter_map(|name| {
                let id = Path::new(name).file_stem()?.to_str()?.parse().ok()?;
                Some((id, name.to_owned()))
            })
            .collect();

        log::info!(
            "GraphicsZipCache loaded {:?} ({} indexed sprites)",
            zip_path,
            entries.len()
        );

        Ok(Self {
            zip_path,
            entries,
            textures: HashMap::new(),
            in_flight: HashSet::new(),
            failed: HashMap::new(),
            workers: None,
        })
    }

    /// Whether the archive has an entry for `sprite_id` (without decoding it).
    ///
    /// # Arguments
    ///
    /// * `sprite_id` - Sprite to look up.
    ///
    /// # Returns
    ///
    /// * `true` when the sprite exists in the archive index.
    #[allow(dead_code)] // Unused by the template viewer, which shares this file.
    pub(crate) fn contains(&self, sprite_id: usize) -> bool {
        self.entries.contains_key(&sprite_id)
    }

    /// Number of sprites currently queued or being decoded.
    ///
    /// # Returns
    ///
    /// * Count of outstanding decode requests.
    #[allow(dead_code)] // Unused by the template viewer, which shares this file.
    pub(crate) fn pending_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Return the texture for `sprite_id`, queueing a background decode on first request.
    ///
    /// # Arguments
    ///
    /// * `ctx` - egui context used to upload textures and wake the UI.
    /// * `sprite_id` - Sprite to fetch.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(texture))` once decoded.
    /// * `Ok(None)` while decoding, or when the sprite isn't in the archive.
    /// * `Err` when the sprite failed to decode.
    pub(crate) fn texture_for(
        &mut self,
        ctx: &egui::Context,
        sprite_id: usize,
    ) -> Result<Option<&egui::TextureHandle>, String> {
        self.collect_decoded(ctx);

        if self.textures.contains_key(&sprite_id) {
            return Ok(self.textures.get(&sprite_id));
        }
        if let Some(err) = self.failed.get(&sprite_id) {
            return Err(err.clone());
        }
        let Some(entry_name) = self.entries.get(&sprite_id).cloned() else {
            return Ok(None);
        };

        if self.in_flight.insert(sprite_id) {
            let zip_path = self.zip_path.clone();
            let workers = self
                .workers
                .get_or_insert_with(|| spawn_decode_workers(&zip_path, ctx));
            if workers.jobs.send((sprite_id, entry_name)).is_err() {
                self.in_flight.remove(&sprite_id);
                return Err("Sprite decoder threads are not running".to_owned());
            }
        }
        Ok(None)
    }

    /// Upload every sprite the workers have finished decoding.
    fn collect_decoded(&mut self, ctx: &egui::Context) {
        let Some(workers) = &self.workers else {
            return;
        };
        while let Ok((sprite_id, result)) = workers.results.try_recv() {
            self.in_flight.remove(&sprite_id);
            match result {
                Ok(image) => {
                    let texture = ctx.load_texture(
                        format!("sprite:{}:{}", self.zip_path.display(), sprite_id),
                        image,
                        egui::TextureOptions::NEAREST,
                    );
                    self.textures.insert(sprite_id, texture);
                }
                Err(e) => {
                    self.failed.insert(sprite_id, e);
                }
            }
        }
    }
}

/// Open a zip archive, mapping errors to user-facing messages.
fn open_archive(zip_path: &Path) -> Result<ZipArchive<File>, String> {
    let file = File::open(zip_path)
        .map_err(|e| format!("Failed to open graphics zip {:?}: {e}", zip_path))?;
    ZipArchive::new(file).map_err(|e| format!("Failed to read zip {:?}: {e}", zip_path))
}

/// Start the decoder pool (leaving one core for the UI thread).
fn spawn_decode_workers(zip_path: &Path, ctx: &egui::Context) -> DecodeWorkers {
    let (job_tx, job_rx) = mpsc::channel::<DecodeJob>();
    let (result_tx, result_rx) = mpsc::channel::<DecodeResult>();
    let job_rx = Arc::new(Mutex::new(job_rx));

    let count = std::thread::available_parallelism()
        .map_or(1, |n| n.get().saturating_sub(1))
        .clamp(1, MAX_DECODE_WORKERS);
    for i in 0..count {
        let job_rx = Arc::clone(&job_rx);
        let result_tx = result_tx.clone();
        let ctx = ctx.clone();
        let zip_path = zip_path.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name(format!("sprite-decode-{i}"))
            .spawn(move || decode_worker(&zip_path, &job_rx, &result_tx, &ctx));
        if let Err(e) = spawned {
            log::warn!("Failed to spawn sprite decoder thread {i}: {e}");
        }
    }

    DecodeWorkers {
        jobs: job_tx,
        results: result_rx,
    }
}

/// Decoder thread body; exits when the cache (job sender or result receiver) is dropped.
fn decode_worker(
    zip_path: &Path,
    jobs: &Mutex<Receiver<DecodeJob>>,
    results: &Sender<DecodeResult>,
    ctx: &egui::Context,
) {
    let mut archive = open_archive(zip_path);
    loop {
        let job = match jobs.lock() {
            Ok(rx) => rx.recv(),
            Err(_) => return,
        };
        let Ok((sprite_id, entry_name)) = job else {
            return;
        };
        let result = match archive.as_mut() {
            Ok(archive) => decode_entry(archive, &entry_name),
            Err(e) => Err(e.clone()),
        };
        if results.send((sprite_id, result)).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

/// Read and decode one zip entry into an egui image.
fn decode_entry(
    archive: &mut ZipArchive<File>,
    entry_name: &str,
) -> Result<egui::ColorImage, String> {
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| format!("Failed to read zip entry {:?}: {e}", entry_name))?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read zip entry {:?} bytes: {e}", entry_name))?;

    let rgba = image::load_from_memory(&bytes)
        .map_err(|e| format!("Failed to decode {:?}: {e}", entry_name))?
        .to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        rgba.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::GraphicsZipCache;
    use eframe::egui;
    use std::io::{Cursor, Write};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// Write a zip with one valid 2x3 PNG (`5.png`), one corrupt entry (`6.png`), and a
    /// non-numeric name that must not be indexed.
    fn write_test_zip(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("{name}_{}.zip", std::process::id()));
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 3, image::Rgba([10, 20, 30, 255]))
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode png");

        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("create zip"));
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (entry, bytes) in [
            ("sprites/5.png", png.as_slice()),
            ("6.png", b"not a png".as_slice()),
            ("readme.txt", b"ignored".as_slice()),
        ] {
            zip.start_file(entry, options).expect("start entry");
            zip.write_all(bytes).expect("write entry");
        }
        zip.finish().expect("finish zip");
        path
    }

    /// Poll `texture_for` until it stops returning `Ok(None)` or times out.
    fn wait_for(
        cache: &mut GraphicsZipCache,
        ctx: &egui::Context,
        sprite_id: usize,
    ) -> Result<Option<[usize; 2]>, String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match cache.texture_for(ctx, sprite_id) {
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                other => return other.map(|t| t.map(|t| t.size())),
            }
        }
    }

    #[test]
    fn load_indexes_numeric_entries_only() {
        let path = write_test_zip("gfx_index");
        let cache = GraphicsZipCache::load(path.clone()).expect("load zip");
        assert!(cache.contains(5));
        assert!(cache.contains(6));
        assert_eq!(cache.entries.len(), 2);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn texture_for_decodes_in_background() {
        let path = write_test_zip("gfx_decode");
        let mut cache = GraphicsZipCache::load(path.clone()).expect("load zip");
        let ctx = egui::Context::default();

        assert!(matches!(cache.texture_for(&ctx, 5), Ok(None)));
        assert_eq!(wait_for(&mut cache, &ctx, 5), Ok(Some([2, 3])));
        assert_eq!(cache.pending_count(), 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn texture_for_reports_decode_errors_and_missing_sprites() {
        let path = write_test_zip("gfx_errors");
        let mut cache = GraphicsZipCache::load(path.clone()).expect("load zip");
        let ctx = egui::Context::default();

        assert!(matches!(cache.texture_for(&ctx, 999), Ok(None)));
        assert_eq!(cache.pending_count(), 0);
        assert!(wait_for(&mut cache, &ctx, 6).is_err());
        assert!(cache.texture_for(&ctx, 6).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_rejects_missing_archive() {
        assert!(GraphicsZipCache::load(PathBuf::from("/nonexistent/gfx.zip")).is_err());
    }
}
