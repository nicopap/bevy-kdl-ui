use std::{io::Read, marker::PhantomData, path::PathBuf, str::Utf8Error};

use bevy::{
    asset::FileAssetIo,
    ecs::system::SystemParam,
    prelude::*,
    reflect::{ReflectRef, TypeRegistryInternal},
    utils::{HashMap, HashSet},
};
use bevy_kdl_reflect_deser::{from_doc, ConvertErrors, ConvertResult};
use kdl::{KdlDocument, KdlError};
use slotmap::{new_key_type, SecondaryMap, SlotMap};
use template_kdl::ExportedBindings;
use thiserror::Error;

use crate::reload::AssetManager;

pub struct BoxedReflect(pub Box<dyn Reflect>);
#[rustfmt::skip]
impl Reflect for BoxedReflect {
    fn set(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> { self.0.set(value) }
    fn apply(&mut self, value: &dyn Reflect) { self.0.apply(value) }
    fn as_any(&self) -> &dyn std::any::Any { self.0.as_any() }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> { self.0.into_any() }
    fn type_name(&self) -> &str { self.0.type_name() }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self.0.as_any_mut() }
    fn as_reflect(&self) -> &dyn Reflect { self.0.as_reflect() }
    fn reflect_ref(&self) -> bevy::reflect::ReflectRef { self.0.reflect_ref() }
    fn reflect_mut(&mut self) -> bevy::reflect::ReflectMut { self.0.reflect_mut() }
    fn clone_value(&self) -> Box<dyn Reflect> { self.0.clone_value() }
    fn into_reflect(self: Box<Self>) -> Box<dyn Reflect> { self.0.into_reflect() }
    fn get_type_info(&self) -> &'static bevy::reflect::TypeInfo { self.0.get_type_info() }
    fn reflect_owned(self: Box<Self>) -> bevy::reflect::ReflectOwned { self.0.reflect_owned() }
    fn as_reflect_mut(&mut self) -> &mut dyn Reflect { self.0.as_reflect_mut() }
}
impl FromReflect for BoxedReflect {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        Some(BoxedReflect(reflect.clone_value()))
    }
}
impl FromReflect for DeserEntity {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ReflectRef::Struct(reflect) = reflect.reflect_ref() else { return None; };
        Some(DeserEntity {
            refer_by: Option::from_reflect(reflect.field("refer_by")?)?,
            components: HashMap::from_reflect(reflect.field("components")?)?,
            children: Vec::from_reflect(reflect.field("children")?)?,
        })
    }
}

#[derive(Reflect, FromReflect, Clone)]
enum ReferBy {
    Name(String),
    Id(u32),
}
#[derive(Reflect)]
struct DeserEntity {
    refer_by: Option<ReferBy>,
    components: HashMap<String, BoxedReflect>,
    children: Vec<DeserEntity>,
}

#[derive(Error, Debug)]
enum CuddlyError {
    #[error("Template error: {0}")]
    TemplateError(#[from] template_kdl::err::Error),
    #[error("file error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Conversion error: {0}")]
    ConvertError(#[from] ConvertErrors),
    #[error("Parsing error: {0}")]
    KdlError(#[from] KdlError),
    #[error("Utf8 validation error when reading the kdl file: {0}")]
    Utf8(#[from] Utf8Error),
}

#[derive(Component)]
pub struct KdlScene {
    pub file: String,
}

/// Stored in `LoadManager::graph` to manage dependencies.
#[derive(Component)]
struct LoadStatus {
    dependencies: Vec<KdlInstanceKey>,
    state: LoadState,
}

enum LoadState {
    ExportsReady(ExportedBindings),
    // TODO: use concrete type DeserEntity here instead
    SceneReady(Box<dyn Reflect>),
    // TODO(ERR): probably need to accumulate several errors.
    Failed(CuddlyError),
}

// TODO(ERR): Accumulate errors
fn load_kdl_template(
    asset_path: &PathBuf,
    current: &str,
    registry: &TypeRegistryInternal,
    instances: &mut KdlInstances,
) -> Result<KdlInstanceKey, CuddlyError> {
    let mut file = std::fs::File::open(asset_path.join(current))?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)?;
    let document: KdlDocument = file_content.parse()?;
    let deps = template_kdl::get_imports(&document)?;
    let mut list = std::collections::HashMap::new();
    let mut dependencies = HashSet::new();
    for dep in deps.required_files() {
        let exports_key = if let Some(already_loaded_key) = instances.keys.get(dep) {
            *already_loaded_key
        } else {
            load_kdl_template(asset_path, dep, registry, instances)?
        };
        let exports = match &instances.states.get(exports_key).unwrap().state {
            LoadState::SceneReady(_) | LoadState::Failed(_) => panic!("TODO(ERR)"),
            LoadState::ExportsReady(exports) => exports.clone(),
        };
        dependencies.insert(exports_key);
        list.insert(dep, exports);
    }
    let required = deps.bindings(&template_kdl::ExportedBindingsList { list })?;
    let state = match from_doc::<DeserEntity>(document, required, registry) {
        ConvertResult::Deserialized(reflect) => LoadState::SceneReady(reflect),
        ConvertResult::Exports(bindings) => LoadState::ExportsReady(bindings),
        ConvertResult::Errors(errs) => LoadState::Failed(errs.into()),
    };
    let key = instances.states.insert(LoadStatus {
        dependencies: dependencies.into_iter().collect(),
        state,
    });
    instances.keys.insert(current.to_string(), key);
    Ok(key)
}
// TODO(PERF): async (see `1_nonbevy_loader.md`)
fn load_scene(
    asset_server: Res<AssetServer>,
    app_registry: Res<AppTypeRegistry>,
    mut instances: ResMut<KdlInstances>,
    scenes: Query<&KdlScene, Changed<KdlScene>>,
) {
    for scene in &scenes {
        // TODO(COMPAT): wasm support
        let asset_io: &FileAssetIo = asset_server.asset_io().downcast_ref().unwrap();
        let root = asset_io.root_path();
        let registry = app_registry.read();
        load_kdl_template(root, &scene.file, &*registry, &mut instances);
    }
}
new_key_type! { struct KdlInstanceKey; }

#[derive(Component, Clone, Copy)]
pub struct KdlInstance(KdlInstanceKey);

#[derive(Resource)]
#[doc(hidden)]
pub struct KdlInstances {
    // TODO(PERF): theoretically, we could havea signle large Vec<Entity>
    // and store the list of spawned instances as (offset, len)
    spawned: SecondaryMap<KdlInstanceKey, Vec<Entity>>,
    states: SlotMap<KdlInstanceKey, LoadStatus>,
    keys: HashMap<String, KdlInstanceKey>,
}
#[derive(SystemParam)]
pub struct KdlManager<'w, 's> {
    kdl_instances: Res<'w, KdlInstances>,
    #[system_param(ignore)]
    _p: PhantomData<&'s ()>,
}
impl<'w, 's> AssetManager for KdlManager<'w, 's> {
    type Instance = KdlInstance;
    type LoadMarker = KdlScene;

    fn instance_entities(&self, instance: &Self::Instance) -> Option<Vec<Entity>> {
        self.kdl_instances.spawned.get(instance.0).cloned()
    }

    fn load_marker(&self, path: &str) -> Self::LoadMarker {
        KdlScene { file: path.to_string() }
    }
}
