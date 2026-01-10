use bevy::{
    asset::{
        io::{
            AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceBuilders, PathStream,
            Reader,
        },
        AssetLoader, AsyncReadExt, LoadContext,
    },
    prelude::*,
    reflect::TypePath,
    utils::BoxedFuture,
};

#[allow(dead_code)]
#[derive(Asset, TypePath, Debug)]
pub struct DatBlob {
    pub bytes: Vec<u8>,
}

#[derive(Default)]
pub struct DatBlobLoader;

impl AssetLoader for DatBlobLoader {
    type Asset = DatBlob;
    type Settings = ();
    type Error = std::io::Error;

    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a (),
        _load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(DatBlob { bytes })
        })
    }

    fn extensions(&self) -> &[&str] {
        &["dat"]
    }
}

pub struct MagCustomAssetsPlugin;

impl Plugin for MagCustomAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<DatBlob>()
            .init_asset_loader::<DatBlobLoader>();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct MagAssetSourcePlugin;

#[cfg(not(target_arch = "wasm32"))]
impl Plugin for MagAssetSourcePlugin {
    fn build(&self, app: &mut App) {
        let mut builders = app
            .world
            .get_resource_or_insert_with::<AssetSourceBuilders>(Default::default);

        builders.insert(
            "mag",
            AssetSourceBuilder::default().with_reader(|| Box::new(MagAssetReader::new("assets"))),
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct MagAssetReader {
    fallback: bevy::asset::io::file::FileAssetReader,
}

#[cfg(not(target_arch = "wasm32"))]
impl MagAssetReader {
    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            fallback: bevy::asset::io::file::FileAssetReader::new(path),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl AssetReader for MagAssetReader {
    fn read<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> BoxedFuture<'a, Result<Box<Reader<'a>>, AssetReaderError>> {
        self.fallback.read(path)
    }

    fn read_meta<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> BoxedFuture<'a, Result<Box<Reader<'a>>, AssetReaderError>> {
        self.fallback.read_meta(path)
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> BoxedFuture<'a, Result<Box<PathStream>, AssetReaderError>> {
        self.fallback.read_directory(path)
    }

    fn is_directory<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> BoxedFuture<'a, Result<bool, AssetReaderError>> {
        self.fallback.is_directory(path)
    }
}

#[cfg(target_arch = "wasm32")]
pub struct MagAssetSourcePlugin;

#[cfg(target_arch = "wasm32")]
impl Plugin for MagAssetSourcePlugin {
    fn build(&self, _app: &mut App) {
        // File-backed custom AssetReader isn't available on wasm32.
    }
}
