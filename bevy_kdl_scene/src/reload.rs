//! Defines reloading [`Hook`]s and supporting system.

use std::marker::PhantomData;

use bevy::{
    ecs::{
        system::{Command, EntityCommands, SystemParam, SystemParamItem},
        world::EntityRef,
    },
    prelude::*,
    scene::SceneInstance,
};

#[derive(PartialEq, Eq, Clone, Copy, Debug, Reflect)]
pub enum Rstate {
    Loading,
    Hooked,
    MustReload,
    MustDelete,
}
#[derive(Component, Reflect)]
pub struct Hook {
    pub file_path: String,
    pub state: Rstate,
    #[reflect(ignore)]
    pub hook: Box<dyn Fn(&EntityRef, &mut EntityCommands, &World, Entity) + Send + Sync + 'static>,
}
impl Hook {
    pub fn new<F>(hook: F, file_path: String) -> Self
    where
        F: Fn(&EntityRef, &mut EntityCommands, &World, Entity) + Send + Sync + 'static,
    {
        Hook {
            state: Rstate::Loading,
            file_path,
            hook: Box::new(hook),
        }
    }
}
struct UpdateHook {
    entity: Entity,
    new_state: Rstate,
}
impl Command for UpdateHook {
    fn write(self, world: &mut World) {
        if let Some(mut hook) = world.get_mut::<Hook>(self.entity) {
            hook.state = self.new_state;
        }
    }
}

pub fn run_hooks<MNG: SystemParam + AssetManager>(
    instances: Query<(Entity, &MNG::Instance, &Hook)>,
    scene_manager: SystemParamItem<MNG>,
    world: &World,
    mut cmds: Commands,
) where
    for<'w, 's> SystemParamItem<'w, 's, MNG>:
        AssetManager<Instance = MNG::Instance, LoadMarker = MNG::LoadMarker>,
{
    use Rstate::*;
    for (entity, instance, reload) in instances.iter() {
        let Some(entities) = scene_manager.instance_entities(instance) else {
            continue;
        };
        match reload.state {
            Loading => {
                cmds.add(UpdateHook { entity, new_state: Hooked });
                for entity_ref in entities.iter().filter_map(|e| world.get_entity(*e)) {
                    let mut cmd = cmds.entity(entity_ref.id());
                    (reload.hook)(&entity_ref, &mut cmd, world, entity);
                }
            }
            Hooked => continue,
            MustReload => {
                for entity in entities.iter().filter(|e| world.get_entity(**e).is_some()) {
                    cmds.entity(*entity).despawn_recursive();
                }
                cmds.add(UpdateHook { entity, new_state: Loading });
                cmds.entity(entity)
                    .insert(scene_manager.load_marker(&reload.file_path))
                    .remove::<MNG::Instance>();
            }
            MustDelete => {
                for entity in entities.iter().filter(|e| world.get_entity(**e).is_some()) {
                    cmds.entity(*entity).despawn_recursive();
                }
                cmds.entity(entity).despawn_recursive();
            }
        }
    }
}

#[derive(SystemParam)]
struct SceneManager<'w, 's> {
    scene: Res<'w, SceneSpawner>,
    assets: Res<'w, AssetServer>,
    #[system_param(ignore)]
    _p: PhantomData<&'s ()>,
}

impl<'w, 's> AssetManager for SceneManager<'w, 's> {
    type Instance = SceneInstance;
    type LoadMarker = Handle<Scene>;

    fn instance_entities(&self, instance: &Self::Instance) -> Option<Vec<Entity>> {
        // TODO: highly inneficient
        self.scene
            .instance_is_ready(**instance)
            .then(|| self.scene.iter_instance_entities(**instance).collect())
    }
    fn load_marker(&self, path: &str) -> Self::LoadMarker {
        self.assets.load::<Scene, _>(path)
    }
}
pub trait AssetManager {
    type Instance: Component;
    type LoadMarker: Component;
    fn instance_entities(&self, instance: &Self::Instance) -> Option<Vec<Entity>>;
    fn load_marker(&self, path: &str) -> Self::LoadMarker;
}

pub struct Plug<T: AssetManager>(PhantomData<fn(T)>);

impl<MNG: AssetManager + SystemParam + 'static> Plugin for Plug<MNG>
where
    for<'w, 's> SystemParamItem<'w, 's, MNG>:
        AssetManager<Instance = MNG::Instance, LoadMarker = MNG::LoadMarker>,
{
    fn build(&self, app: &mut App) {
        app.register_type::<Hook>()
            .register_type::<Rstate>()
            .add_system(run_hooks::<MNG>);
    }
}
