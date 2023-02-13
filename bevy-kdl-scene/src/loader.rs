use std::str::{self, Utf8Error};
use std::sync::Arc;

use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use thiserror::Error;

use bevy_reflect_deser::{convert_doc, from_doc, Exports};

#[derive(Error, Debug)]
enum CuddlyError {
    #[error("Parsing error: {0}")]
    KdlError(#[from] KdlError),
    #[error("Utf8 validation error when reading the kdl file: {0}")]
    Utf8(#[from] Utf8Error),
    #[error("The file is empty")]
    EmptyFile,
    #[error("Cannot read a UI from an empty file")]
    NonUtf8Path,
}

enum CuddlyFile {
    Scene(Box<dyn Reflect>),
    Exports(Exports),
}

/// An individual node in a `.bui.kdl` file.
#[derive(Debug, TypeUuid, Clone)]
#[uuid = "0c4f2b85-4fa7-496f-a27f-7aff5837f7c7"]
pub struct Cuddly {
    file: Arc<str>,
    node: CuddlyFile,
}
impl Cuddly {
    fn load(ctx: &mut LoadContext, file_content: &[u8]) -> Result<(), CuddlyError> {
        let mut document: KdlDocument = str::from_utf8(file_content)?.parse()?;
        let path_err = CuddlyError::NonUtf8Path;
        let cuddly = Cuddly {
            file: ctx.path().to_str().to_owned().ok_or(path_err)?.into(),
            node: document.nodes_mut().pop().ok_or(CuddlyError::EmptyFile)?,
        };
        ctx.set_default_asset(LoadedAsset::new(cuddly));
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct Loader;
impl AssetLoader for Loader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), anyhow::Error>> {
        Box::pin(async move { Ok(Cuddly::load(load_context, bytes)?) })
    }
    fn extensions(&self) -> &[&str] {
        &["scene.kdl"]
    }
}
