use bevy::pbr::{MaterialPlugin, NotShadowCaster, SpotLight, VolumetricLight};
use bevy::prelude::*;
use bevy::render::{mesh::Indices, render_resource::PrimitiveTopology};

use crate::render_settings::RenderSettings;

use super::VolumetricConeDebugMaterial;

#[derive(Resource, Default, Clone)]
pub struct VolumetricConeAssets {
    pub mesh: Option<Handle<Mesh>>,
    pub debug_material: Option<Handle<VolumetricConeDebugMaterial>>,
}

pub(super) fn register(app: &mut App) {
    app.add_plugins(MaterialPlugin::<VolumetricConeDebugMaterial>::default())
        .init_resource::<VolumetricConeAssets>()
        .register_type::<VolumetricCone>()
        .add_systems(Startup, setup_volumetric_cone_assets)
        .add_systems(Update, sync_spotlight_cones);
}

fn setup_volumetric_cone_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut cone_dbg_mats: ResMut<Assets<VolumetricConeDebugMaterial>>,
    mut assets: ResMut<VolumetricConeAssets>,
) {
    if assets.mesh.is_some() {
        return;
    }

    let debug_handle = cone_dbg_mats.add(VolumetricConeDebugMaterial::default());
    let mesh_handle = meshes.add(make_unit_cone_negz(32));

    assets.debug_material = Some(debug_handle);
    assets.mesh = Some(mesh_handle);
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct VolumetricCone;

#[allow(clippy::type_complexity)]
fn sync_spotlight_cones(
    mut commands: Commands,
    mut spotlights: Query<(Entity, &mut SpotLight, Option<&Children>)>,
    mut cone_transforms: Query<&mut Transform, With<VolumetricCone>>,
    assets: Res<VolumetricConeAssets>,
    render_settings: Option<Res<RenderSettings>>,
) {
    let enabled = render_settings
        .as_ref()
        .map(|settings| settings.volumetric_cones)
        .unwrap_or(true);

    let Some(base_mesh) = assets.mesh.clone() else {
        return;
    };

    for (entity, mut light, children) in &mut spotlights {
        if !enabled || light.range <= 0.1 {
            commands.entity(entity).remove::<VolumetricLight>();
            if let Some(children) = children {
                for child in children.iter() {
                    if cone_transforms.get_mut(child).is_ok() {
                        commands.entity(child).despawn();
                    }
                }
            }
            continue;
        }

        commands.entity(entity).remove::<VolumetricLight>();
        light.shadows_enabled = true;

        let height = light.range;
        let radius = (height * light.outer_angle.tan()).max(0.01);
        let cone_transform = Transform::from_translation(-Vec3::Z * height * 0.001)
            .with_scale(Vec3::new(radius, radius, height));

        let mut found_existing = false;
        if let Some(children) = children {
            for child in children.iter() {
                if let Ok(mut transform) = cone_transforms.get_mut(child) {
                    *transform = cone_transform;
                    commands
                        .entity(child)
                        .insert((Visibility::Inherited, Name::new("VolumetricCone")));
                    found_existing = true;
                    break;
                }
            }
        }

        if !found_existing {
            let id = commands
                .spawn((
                    Mesh3d(base_mesh.clone()),
                    cone_transform,
                    GlobalTransform::default(),
                    VolumetricCone,
                    NotShadowCaster,
                    Name::new("VolumetricCone"),
                    Visibility::Inherited,
                ))
                .id();
            commands.entity(id).insert(ChildOf(entity));
        }
    }
}

fn make_unit_cone_negz(segments: usize) -> Mesh {
    let segments = segments.max(30);
    let mut positions = Vec::with_capacity(segments + 1);
    let mut normals = Vec::with_capacity(segments + 1);
    let mut uvs = Vec::with_capacity(segments + 1);
    let mut indices: Vec<u32> = Vec::with_capacity(segments * 3);

    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push([0.5, 1.0]);

    for i in 0..segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        let p = Vec3::new(c, s, -1.0);
        positions.push(p.to_array());
        let n = Vec3::new(c, s, 1.0).normalize();
        normals.push(n.to_array());
        uvs.push([i as f32 / segments as f32, 0.0]);
    }

    for i in 0..segments {
        let i0 = 1 + i as u32;
        let i1 = 1 + ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[0, i0, i1]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
