use bevy::{
    asset::{
        io::{
            AssetReader, AssetReaderError, AssetReaderFuture, AssetSourceBuilder,
            AssetSourceBuilders, ErasedAssetReader, PathStream, Reader,
        },
        AssetLoader, LoadContext,
    },
    prelude::*,
    reflect::TypePath,
};

use bevy::tasks::ConditionalSendFuture;

use std::path::Path;

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

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(DatBlob { bytes })
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
            .world_mut()
            .get_resource_or_insert_with::<AssetSourceBuilders>(Default::default);

        builders.insert(
            "mag",
            AssetSourceBuilder::default().with_reader(|| {
                Box::new(MagAssetReader::new("assets")) as Box<dyn ErasedAssetReader>
            }),
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
    fn read<'a>(&'a self, path: &'a Path) -> impl AssetReaderFuture<Value: Reader + 'a> {
        <bevy::asset::io::file::FileAssetReader as AssetReader>::read(&self.fallback, path)
    }

    fn read_meta<'a>(&'a self, path: &'a Path) -> impl AssetReaderFuture<Value: Reader + 'a> {
        <bevy::asset::io::file::FileAssetReader as AssetReader>::read_meta(&self.fallback, path)
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<PathStream>, AssetReaderError>> {
        <bevy::asset::io::file::FileAssetReader as AssetReader>::read_directory(
            &self.fallback,
            path,
        )
    }

    fn is_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<bool, AssetReaderError>> {
        <bevy::asset::io::file::FileAssetReader as AssetReader>::is_directory(&self.fallback, path)
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
