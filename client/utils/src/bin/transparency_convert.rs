use image::{DynamicImage, ImageError, ImageOutputFormat, RgbaImage};
use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

const KEYS: &'static [(u8, u8, u8)] = &[
    (0xff, 0x00, 0xff), // #ff00ff
    (0xfe, 0x00, 0xfe), // #fe00fe
    (0xfd, 0x00, 0xfd), // #fd00fd
    (0xfc, 0x00, 0xfc), // #fc00fc
    (0xfb, 0x00, 0xfb), // #fb00fb
    (0xfa, 0x00, 0xfa), // #fa00fa
    (0xf9, 0x00, 0xf9), // #f900f9
    (0xf8, 0x00, 0xf8), // #f800f8
    (0xf7, 0x00, 0xf7), // #f700f7
];

fn force_png_path<P: AsRef<Path>>(p: P) -> std::path::PathBuf {
    let mut pb = p.as_ref().to_path_buf();
    match pb.extension().and_then(|s| s.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("png") => pb,
        _ => {
            pb.set_extension("png");
            pb
        }
    }
}

fn pixel_matches_key(r: u8, g: u8, b: u8) -> bool {
    KEYS.iter()
        .any(|&(kr, kg, kb)| kr == r && kg == g && kb == b)
}

fn process_image(img: DynamicImage) -> RgbaImage {
    let mut rgba = img.to_rgba8();
    for px in rgba.pixels_mut() {
        let r = px[0];
        let g = px[1];
        let b = px[2];
        if pixel_matches_key(r, g, b) {
            px[3] = 0;
        }
    }
    rgba
}

fn convert_file(input_path: &Path, output_path: &Path) -> Result<(), ImageError> {
    let img = image::open(input_path)?;
    let out = process_image(img);

    // Save as PNG, enforcing PNG format
    let fout = File::create(&output_path).map_err(|e| ImageError::IoError(e))?;
    let mut w = BufWriter::new(fout);
    out.write_to(&mut w, ImageOutputFormat::Png)?;
    println!("Wrote {}", output_path.display());
    Ok(())
}

fn run() -> Result<(), ImageError> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: transparency_convert <input-file|directory> <output-file|directory.png>");
        std::process::exit(1);
    }

    let is_input_directory = std::fs::metadata(&args[1])
        .map(|m| m.is_dir())
        .unwrap_or(false);

    let is_output_directory = std::fs::metadata(&args[2])
        .map(|m| m.is_dir())
        .unwrap_or(false);

    if is_input_directory != is_output_directory {
        eprintln!("Error: both input and output paths must be either files or directories");
        std::process::exit(1);
    }

    const EXTENSIONS_TO_CONVERT: [&'static str; 2] = ["png", "bmp"];

    // Intentionally non-recursive directory processing
    let files_to_convert: Vec<(std::path::PathBuf, std::path::PathBuf)> = if is_input_directory {
        let input_dir = Path::new(&args[1]);
        let output_dir = Path::new(&args[2]);
        std::fs::read_dir(input_dir)
            .map_err(|e| ImageError::IoError(e))?
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    let path = e.path();
                    let extension = path.extension()?.to_str()?.to_lowercase();
                    if path.is_file() && EXTENSIONS_TO_CONVERT.contains(&extension.as_str()) {
                        Some((
                            path.clone(),
                            output_dir.join(force_png_path(path.file_name()?)),
                        ))
                    } else {
                        None
                    }
                })
            })
            .collect()
    } else {
        vec![(Path::new(&args[1]).to_path_buf(), force_png_path(&args[2]))]
    };

    for (input_path, output_path) in &files_to_convert {
        convert_file(input_path, output_path)?;
    }

    println!("Converted {} file(s)", files_to_convert.len());

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba};

    #[test]
    fn force_png_path_rewrites_extension() {
        assert_eq!(force_png_path("foo.bmp").to_string_lossy(), "foo.png");
        assert_eq!(force_png_path("bar.PNG").to_string_lossy(), "bar.PNG");
        assert_eq!(force_png_path("baz").to_string_lossy(), "baz.png");
    }

    #[test]
    fn pixel_matches_key_detects_magic_pinks() {
        assert!(pixel_matches_key(0xff, 0x00, 0xff));
        assert!(pixel_matches_key(0xfa, 0x00, 0xfa));
        assert!(!pixel_matches_key(0x00, 0x00, 0x00));
    }

    #[test]
    fn process_image_sets_alpha_to_zero_for_key_color() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0xff, 0x00, 0xff, 0xff]));
        img.put_pixel(1, 0, Rgba([0x00, 0x01, 0x02, 0xff]));

        let out = process_image(DynamicImage::ImageRgba8(img));
        assert_eq!(out.get_pixel(0, 0)[3], 0);
        assert_eq!(out.get_pixel(1, 0)[3], 0xff);
    }
}
