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

use crate::reload::{self, AssetManager};

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
            components: Vec::from_reflect(reflect.field("components")?)?,
            children: Vec::from_reflect(reflect.field("children")?)?,
        })
    }
}

#[derive(Error, Debug)]
pub(crate) enum SpawnError {
    #[error(
        "scene contains the unregistered type `{0}`. \
        Consider registering the type using `app.register_type::<{0}>()`"
    )]
    Missing(String),
    #[error(
        "scene contains the unregistered component `{0}`. \
        Consider adding `#[reflect(Component)]` to your type"
    )]
    MissingComponent(String),
}
#[derive(Reflect, FromReflect, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ReferBy {
    Name(String),
    Id(u32),
}
#[derive(Reflect)]
pub(crate) struct DeserEntity {
    pub(crate) refer_by: Option<ReferBy>,
    pub(crate) components: Vec<BoxedReflect>,
    pub(crate) children: Vec<DeserEntity>,
}
impl DeserEntity {
    pub(crate) fn spawn_hierarchy<'a>(
        &'a self,
        world: &mut World,
        current: Entity,
        entity_references: &mut HashMap<&'a ReferBy, Entity>,
        registry: &TypeRegistryInternal,
    ) -> Result<(), SpawnError> {
        if let Some(reference) = &self.refer_by {
            entity_references.insert(reference, current);
        }
        for component in &self.components {
            let get_name = || component.type_name().to_string();
            let registration = registry
                .get_with_name(component.type_name())
                .ok_or_else(|| SpawnError::Missing(get_name()))?;

            let reflect_component = registration
                .data::<ReflectComponent>()
                .ok_or_else(|| SpawnError::MissingComponent(get_name()))?;

            reflect_component.apply_or_insert(world, current, component.0.as_ref());
        }

        for child in &self.children {
            let new_child = world.spawn_empty().id();
            child.spawn_hierarchy(world, new_child, entity_references, registry)?;
            let mut entity = world.entity_mut(current);
            entity.push_children(&[new_child]);
        }
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum CuddlyError {
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
pub(crate) struct LoadStatus {
    dependencies: Vec<KdlInstanceKey>,
    pub(crate) state: LoadState,
    pub(crate) source: String,
}

#[derive(Debug)]
pub(crate) enum LoadState {
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
        // TODO: return value of from_doc should be the type given as argument.
        ConvertResult::Deserialized(reflect) => LoadState::SceneReady(reflect),
        ConvertResult::Exports(bindings) => LoadState::ExportsReady(bindings),
        ConvertResult::Errors(errs) => LoadState::Failed(errs.into()),
    };
    let key = instances.states.insert(LoadStatus {
        dependencies: dependencies.into_iter().collect(),
        state,
        source: current.to_owned(),
    });
    instances.keys.insert(current.to_string(), key);
    Ok(key)
}
// TODO(PERF): async (see `1_nonbevy_loader.md`)
fn load_scene(
    asset_server: Res<AssetServer>,
    app_registry: Res<AppTypeRegistry>,
    mut instances: ResMut<KdlInstances>,
    scenes: Query<(Entity, &KdlScene), Changed<KdlScene>>,
    mut cmds: Commands,
) {
    for (entity, scene) in &scenes {
        // TODO(COMPAT): wasm support
        let asset_io: &FileAssetIo = asset_server.asset_io().downcast_ref().unwrap();
        let root = asset_io.root_path();
        let registry = app_registry.read();
        // TODO(ERR): gahhhh
        let instance = load_kdl_template(root, &scene.file, &registry, &mut instances).unwrap();
        cmds.entity(entity).insert(KdlInstance(instance));
    }
}
new_key_type! { pub(crate) struct KdlInstanceKey; }

#[derive(Component, Clone, Copy)]
pub struct KdlInstance(pub(crate) KdlInstanceKey);

#[derive(Resource)]
#[doc(hidden)]
pub struct KdlInstances {
    // TODO(PERF): theoretically, we could havea signle large Vec<Entity>
    // and store the list of spawned instances as (offset, len)
    pub(crate) spawned: SecondaryMap<KdlInstanceKey, Vec<Entity>>,
    pub(crate) states: SlotMap<KdlInstanceKey, LoadStatus>,
    pub(crate) keys: HashMap<String, KdlInstanceKey>,
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

#[derive(SystemLabel)]
pub enum Systems {
    LoadScene,
}
pub struct Plug;
impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_plugin(reload::Plug::<KdlManager>::new())
            .add_system(load_scene.label(Systems::LoadScene));
    }
}
