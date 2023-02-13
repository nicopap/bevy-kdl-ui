use std::str::{self, Utf8Error};
use std::sync::Arc;

use bevy_app::AppTypeRegistry;
use bevy_asset::{AssetLoader, LoadContext, LoadedAsset};
use bevy_ecs::prelude::FromWorld;
use bevy_ecs::world::World;
use bevy_reflect::{Reflect, TypeRegistryArc, TypeUuid};
use bevy_utils::BoxedFuture;
use kdl::{KdlDocument, KdlError};

use bevy_reflect_deser::{from_doc_untyped, ConvertErrors, ConvertResult};
use template_kdl::Bindings;

use thiserror::Error;

#[derive(Error, Debug)]
enum CuddlyError {
    #[error("Template error: {0}")]
    ConvertError(#[from] ConvertErrors),
    #[error("Parsing error: {0}")]
    KdlError(#[from] KdlError),
    #[error("Utf8 validation error when reading the kdl file: {0}")]
    Utf8(#[from] Utf8Error),
    #[error("The file is empty")]
    EmptyFile,
    #[error("Cannot read a UI from an empty file")]
    NonUtf8Path,
}
#[derive(Debug)]
enum CuddlyFile {
    Exports(Bindings),
    Value(Box<dyn Reflect>),
}
impl Clone for CuddlyFile {
    fn clone(&self) -> Self {
        match self {
            CuddlyFile::Exports(bindings) => CuddlyFile::Exports(bindings.clone()),
            CuddlyFile::Value(reflect) => CuddlyFile::Value(reflect.clone_value()),
        }
    }
}

/// An individual node in a `.bui.kdl` file.
#[derive(Debug, TypeUuid, Clone)]
#[uuid = "0c4f2b85-4fa7-496f-a27f-7aff5837f7c7"]
pub struct Cuddly {
    file: Arc<str>,
    node: CuddlyFile,
}

pub struct Loader {
    registry: TypeRegistryArc,
}
impl Loader {
    async fn load_cuddly(
        &self,
        ctx: &mut LoadContext<'_>,
        file_content: &[u8],
    ) -> Result<(), CuddlyError> {
        let path_err = CuddlyError::NonUtf8Path;
        let file: Arc<str> = ctx.path().to_str().to_owned().ok_or(path_err)?.into();
        let document: KdlDocument = str::from_utf8(file_content)?.parse()?;
        let node = match from_doc_untyped(document, &self.registry.read()) {
            ConvertResult::Deserialized(reflect) => CuddlyFile::Value(reflect),
            ConvertResult::Exports(bindings) => CuddlyFile::Exports(bindings),
            ConvertResult::Errors(errs) => return Err(errs.into()),
        };
        let cuddly = Cuddly { file, node };
        ctx.set_default_asset(LoadedAsset::new(cuddly));
        Ok(())
    }
}
impl FromWorld for Loader {
    fn from_world(world: &mut World) -> Self {
        let type_registry = world.resource::<AppTypeRegistry>();
        Loader { registry: type_registry.0.clone() }
    }
}
impl AssetLoader for Loader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), anyhow::Error>> {
        Box::pin(async move { Ok(self.load_cuddly(load_context, bytes).await?) })
    }
    fn extensions(&self) -> &[&str] {
        &["scene.kdl"]
    }
}
