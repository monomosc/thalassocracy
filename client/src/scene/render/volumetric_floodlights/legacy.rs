use bevy::pbr::{MaterialPlugin, MeshMaterial3d, NotShadowCaster, VolumetricLight};
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_resource::PrimitiveTopology;

use crate::render_settings::RenderSettings;

use super::volumetric_cone_material::VolumetricConeMaterial;
use super::{VolumetricConeDebugMaterial, VolumetricLightingMode, VolumetricLightingState};

#[derive(Resource, Default, Clone)]
pub struct VolumetricConeAssets {
    pub mesh: Option<Handle<Mesh>>,
    pub legacy_material: Option<Handle<VolumetricConeMaterial>>,
    pub debug_material: Option<Handle<VolumetricConeDebugMaterial>>,
}

pub(super) fn register(app: &mut App) {
    app.add_plugins(MaterialPlugin::<VolumetricConeMaterial>::default())
        .add_plugins(MaterialPlugin::<VolumetricConeDebugMaterial>::default())
        .init_resource::<VolumetricConeAssets>()
        .register_type::<VolumetricCone>()
        .register_type::<VolumetricConeMaterial>()
        .add_systems(Startup, setup_volumetric_cone_assets)
        .add_systems(Update, attach_or_update_volumetrics);
}

fn setup_volumetric_cone_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut cone_mats: ResMut<Assets<VolumetricConeMaterial>>,
    mut cone_dbg_mats: ResMut<Assets<VolumetricConeDebugMaterial>>,
    mut assets: ResMut<VolumetricConeAssets>,
) {
    if assets.mesh.is_some() {
        return;
    }

    let legacy_handle = cone_mats.add(VolumetricConeMaterial {
        color: LinearRgba::new(0.18, 0.6, 0.9, 0.12),
        params0: Vec4::new(2.2, 1.0, 0.6, 0.6),
        params1: Vec4::new(0.25, 8.0, 1.2, 0.08),
        flicker_amps: Vec4::new(0.05, 0.03, 0.02, 0.0),
        flicker_freqs: Vec4::new(0.27, 1.3, 7.7, 0.0),
        flicker_phases: Vec4::ZERO,
        hdr_params: Vec4::new(30.0, 0.0, 0.0, 0.0),
        alpha_mode: AlphaMode::Add,
        fog_enabled: true,
    });

    let debug_handle = cone_dbg_mats.add(VolumetricConeDebugMaterial::default());
    let mesh_handle = meshes.add(make_unit_cone_negz(32));

    assets.legacy_material = Some(legacy_handle);
    assets.debug_material = Some(debug_handle);
    assets.mesh = Some(mesh_handle);
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct VolumetricCone;

#[derive(Component)]
pub struct VolumetricHalo;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn attach_or_update_volumetrics(
    mut commands: Commands,
    mut q_spot: Query<(Entity, &mut SpotLight, Option<&Children>)>,
    mut q_cone: Query<
        (
            Entity,
            &mut Transform,
            &MeshMaterial3d<VolumetricConeMaterial>,
        ),
        (
            With<VolumetricCone>,
            Without<VolumetricHalo>,
            Without<MeshMaterial3d<VolumetricConeDebugMaterial>>,
        ),
    >,
    mut q_cone_dbg: Query<
        (
            Entity,
            &mut Transform,
            &MeshMaterial3d<VolumetricConeDebugMaterial>,
        ),
        (
            With<VolumetricCone>,
            Without<VolumetricHalo>,
            Without<MeshMaterial3d<VolumetricConeMaterial>>,
        ),
    >,
    mut _q_point: Query<(Entity, &PointLight, Option<&Children>)>,
    mut q_halo: Query<(Entity, &mut Transform), (With<VolumetricHalo>, Without<VolumetricCone>)>,
    assets: Res<VolumetricConeAssets>,
    mut cone_mats: ResMut<Assets<VolumetricConeMaterial>>,
    _cone_dbg_mats: ResMut<Assets<VolumetricConeDebugMaterial>>,
    render_settings: Option<Res<RenderSettings>>,
    state: Res<VolumetricLightingState>,
) {
    if render_settings
        .as_ref()
        .map(|s| !s.volumetric_cones)
        .unwrap_or(false)
    {
        for (e, _light, _) in &mut q_spot {
            commands.entity(e).remove::<VolumetricLight>();
        }
        for (e, _, _) in &mut q_cone {
            commands.entity(e).despawn();
        }
        for (e, _) in &mut q_halo {
            commands.entity(e).despawn();
        }
        return;
    }

    let Some(base_mesh) = assets.mesh.clone() else {
        return;
    };
    let Some(legacy_handle) = assets.legacy_material.clone() else {
        return;
    };
    let Some(debug_handle) = assets.debug_material.clone() else {
        return;
    };

    let raymarch_mode = matches!(state.mode, VolumetricLightingMode::RaymarchCones);

    for (e, mut light, children) in &mut q_spot {
        if light.range <= 0.1 {
            commands.entity(e).remove::<VolumetricLight>();
            if let Some(children) = children {
                for child in children.iter() {
                    if q_cone.get_mut(child).is_ok() {
                        commands.entity(child).despawn();
                    }
                }
            }
            continue;
        }

        let height = light.range;
        let radius = (height * light.outer_angle.tan()).max(0.01);
        let cone_t = Transform::from_translation(-Vec3::Z * height * 0.001)
            .with_scale(Vec3::new(radius, radius, height));

        if raymarch_mode {
            light.shadows_enabled = false;
            commands.entity(e).remove::<VolumetricLight>();

            let mut found = None;
            if let Some(children) = children {
                for child in children.iter() {
                    if q_cone_dbg.get_mut(child).is_ok() || q_cone.get_mut(child).is_ok() {
                        found = Some(child);
                        break;
                    }
                }
            }

            match found {
                Some(child) => {
                    commands
                        .entity(child)
                        .remove::<MeshMaterial3d<VolumetricConeMaterial>>()
                        .insert(MeshMaterial3d(debug_handle.clone()));
                    if let Ok((_e, mut t, _)) = q_cone_dbg.get_mut(child) {
                        *t = cone_t;
                    }
                    if let Ok((_e, mut t, _)) = q_cone.get_mut(child) {
                        *t = cone_t;
                    }
                }
                None => {
                    let id = commands
                        .spawn((
                            Mesh3d(base_mesh.clone()),
                            MeshMaterial3d(debug_handle.clone()),
                            cone_t,
                            GlobalTransform::default(),
                            VolumetricCone,
                            NotShadowCaster,
                            Name::new("VolumetricConeDebug"),
                        ))
                        .id();
                    commands.entity(id).insert(ChildOf(e));
                }
            }
            continue;
        }

        commands.entity(e).remove::<VolumetricLight>();

        let mut found = None;
        if let Some(children) = children {
            for child in children.iter() {
                if q_cone.get_mut(child).is_ok() || q_cone_dbg.get_mut(child).is_ok() {
                    found = Some(child);
                    break;
                }
            }
        }

        match found {
            Some(child) => {
                commands
                    .entity(child)
                    .remove::<MeshMaterial3d<VolumetricConeDebugMaterial>>()
                    .insert(MeshMaterial3d(legacy_handle.clone()));
                if let Ok((_e, mut t, _)) = q_cone.get_mut(child) {
                    *t = cone_t;
                }
                if let Ok((_e, mut t, _)) = q_cone_dbg.get_mut(child) {
                    *t = cone_t;
                }
            }
            None => {
                let id = commands
                    .spawn((
                        Mesh3d(base_mesh.clone()),
                        MeshMaterial3d(legacy_handle.clone()),
                        cone_t,
                        GlobalTransform::default(),
                        VolumetricCone,
                        NotShadowCaster,
                        Name::new("VolumetricCone"),
                    ))
                    .id();
                commands.entity(id).insert(ChildOf(e));
            }
        }

        let mut cone_entity = None;
        if let Some(children) = children {
            for child in children.iter() {
                if q_cone.get_mut(child).is_ok() {
                    cone_entity = Some(child);
                    break;
                }
            }
        }

        let intensity = light.intensity.max(0.0);
        let emissive_boost = (intensity / 10_000_000.0).powf(0.75).clamp(0.02, 600.0);
        let alpha_scale = (intensity / 100_000.0).powf(0.5).clamp(0.01, 0.95);

        match cone_entity {
            Some(entity) => {
                if let Ok((_e, _, mat_handle)) = q_cone.get_mut(entity) {
                    let base_alpha = if let Some(base) = cone_mats.get(&legacy_handle) {
                        base.color.alpha
                    } else if let Some(current) = cone_mats.get(&**mat_handle) {
                        current.color.alpha
                    } else {
                        VolumetricConeMaterial::default().color.alpha
                    };

                    let mut seed = e.index() ^ 0x9E37_79B9;
                    let mut frand = || {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        seed as f32 / u32::MAX as f32
                    };
                    let phase_low = frand() * std::f32::consts::TAU;
                    let phase_mid = frand() * std::f32::consts::TAU;
                    let phase_high = frand() * std::f32::consts::TAU;

                    if let Some(material) = cone_mats.get_mut(&**mat_handle) {
                        material.color.alpha = (base_alpha * alpha_scale).clamp(0.0, 1.0);
                        material.hdr_params.x = emissive_boost;
                        if material.flicker_phases == Vec4::ZERO {
                            material.flicker_phases =
                                Vec4::new(phase_low, phase_mid, phase_high, 0.0);
                        }
                    }
                }
            }
            None => {
                let mut seed = e.index() ^ 0x9E37_79B9;
                let mut frand = || {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    seed as f32 / u32::MAX as f32
                };
                let phase_low = frand() * std::f32::consts::TAU;
                let phase_mid = frand() * std::f32::consts::TAU;
                let phase_high = frand() * std::f32::consts::TAU;

                let new_handle = if let Some(base) = cone_mats.get(&legacy_handle).cloned() {
                    let mut material = base;
                    material.color.alpha = (material.color.alpha * alpha_scale).clamp(0.0, 1.0);
                    material.hdr_params.x = emissive_boost;
                    material.flicker_phases = Vec4::new(phase_low, phase_mid, phase_high, 0.0);
                    cone_mats.add(material)
                } else {
                    let mut material = VolumetricConeMaterial::default();
                    material.color.alpha = (material.color.alpha * alpha_scale).clamp(0.0, 1.0);
                    material.hdr_params.x = emissive_boost;
                    material.flicker_phases = Vec4::new(phase_low, phase_mid, phase_high, 0.0);
                    cone_mats.add(material)
                };

                let id = commands
                    .spawn((
                        Mesh3d(base_mesh.clone()),
                        MeshMaterial3d(new_handle),
                        cone_t,
                        GlobalTransform::default(),
                        VolumetricCone,
                        NotShadowCaster,
                        Name::new("VolumetricCone"),
                    ))
                    .id();
                commands.entity(id).insert(ChildOf(e));
            }
        }
    }

    for (entity, _) in &mut q_halo {
        commands.entity(entity).despawn();
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

