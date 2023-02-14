use std::str::{self, Utf8Error};
use std::sync::Arc;

use bevy_asset::{AssetLoader, LoadContext, LoadedAsset};
use bevy_reflect::{Reflect, TypeUuid};
use bevy_utils::BoxedFuture;
use kdl::{KdlDocument, KdlError};

use bevy_kdl_reflect_deser::ConvertErrors;
use template_kdl::ExportedBindings;

use thiserror::Error;

#[derive(Error, Debug)]
enum CuddlyError {
    #[error("Template error: {0}")]
    TemplateError(#[from] template_kdl::err::Error),
    #[error("Conversion error: {0}")]
    ConvertError(#[from] ConvertErrors),
    #[error("Parsing error: {0}")]
    KdlError(#[from] KdlError),
    #[error("Utf8 validation error when reading the kdl file: {0}")]
    Utf8(#[from] Utf8Error),
    #[error("Cannot read a UI from an empty file")]
    NonUtf8Path,
}
#[derive(Debug)]
enum CuddlyFile {
    Exports(ExportedBindings),
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

#[derive(Debug, Clone)]
struct ImportedCuddly(KdlDocument);

/// An individual node in a `.bui.kdl` file.
#[derive(Debug, TypeUuid, Clone)]
#[uuid = "0c4f2b85-4fa7-496f-a27f-7aff5837f7c7"]
pub struct Cuddly {
    file: Arc<str>,
    node: ImportedCuddly,
}

#[derive(Default)]
pub struct Loader;
impl Loader {
    async fn load_cuddly(
        &self,
        ctx: &mut LoadContext<'_>,
        file_content: &[u8],
    ) -> Result<(), CuddlyError> {
        let path_err = CuddlyError::NonUtf8Path;
        let file: Arc<str> = ctx.path().to_str().to_owned().ok_or(path_err)?.into();
        let document: KdlDocument = str::from_utf8(file_content)?.parse()?;
        let deps = template_kdl::get_imports(&document)?;
        let mut loaded_asset = LoadedAsset::new(Cuddly { file, node: ImportedCuddly(document) });
        for dep in deps.required_bindings() {
            let (file_def, _) = dep.rsplit_once('/').unwrap();
            loaded_asset.add_dependency(file_def.into());
        }
        // let node = match from_doc_untyped(document, &self.registry.read()) {
        //     ConvertResult::Deserialized(reflect) => CuddlyFile::Value(reflect),
        //     ConvertResult::Exports(bindings) => CuddlyFile::Exports(bindings),
        //     ConvertResult::Errors(errs) => return Err(errs.into()),
        // };
        ctx.set_default_asset(loaded_asset);
        Ok(())
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
        &["scene.tpl.kdl"]
    }
}
