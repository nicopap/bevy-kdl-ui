use bevy::{
    ecs::{
        system::{Command, SystemState},
        world::EntityRef,
    },
    prelude::*,
    utils::HashMap,
};

use crate::depends::{self, DeserEntity, KdlInstance, KdlInstances, LoadState};

#[derive(Component)]
pub struct KdlOrigin {
    pub file: String,
}

fn load_instance(world: &mut World) {
    // TODO(PERF): huurrr, accumulating in a Vec, sad.
    let mut to_spawn = Vec::new();
    {
        let mut state: SystemState<(
            Res<KdlInstances>,
            Res<AppTypeRegistry>,
            // TODO: Changed<KdlInstance> (read "Warning" section of SystemState doc first)
            Query<(Entity, &KdlInstance), Added<KdlInstance>>,
        )> = SystemState::new(world);
        let (instances, app_registry, added) = state.get_mut(world);

        for (entity, instance) in &added {
            // TODO: Do not filthy up change detection by prematurely &mut instances
            let KdlInstances { states, .. } = &*instances;
            let status = states.get(instance.0).unwrap();
            let foo = match &status.state {
                LoadState::SceneReady(scene) => DeserEntity::from_reflect(scene.as_ref()).unwrap(),
                any_else => panic!("A spawned KdlInstance wasn't a node file: {any_else:?}"),
            };
            let mut refs = HashMap::new();
            let mut sub_world = World::new();
            foo.spawn_hierarchy(&mut sub_world, entity, &mut refs, &app_registry.read());
            to_spawn.push((Scene::new(sub_world), entity, status.source.clone()));
        }
    }
    world.resource_scope(|world, registry: Mut<AppTypeRegistry>| {
        for (scene, parent, source) in to_spawn.into_iter() {
            // TODO(ERR)
            let infos = scene.write_to_world_with(world, &registry).unwrap();
            for entity in infos.entity_map.values() {
                let mut entity_mut = world.entity_mut(entity);
                entity_mut.insert(KdlOrigin { file: source.clone() });
                // Add the `Parent` component to the scene root, and update the `Children` component of
                // the scene parent
                let has_parent = |entity: EntityRef| entity.contains::<Parent>();
                if !world.get_entity(entity).map_or(true, has_parent) {
                    AddChild { parent, child: entity }.write(world);
                }
            }
        }
    });
}
pub struct Plug;
impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_system(load_instance.after(depends::Systems::LoadScene));
    }
}
