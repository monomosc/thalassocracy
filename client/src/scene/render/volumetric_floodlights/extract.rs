use std::collections::HashMap;

use bevy::pbr::SpotLight;
use bevy::prelude::*;
use bevy::render::{mesh::Mesh3d, view::ViewVisibility, Extract};

use crate::render_settings::{RenderSettings, VolumetricConeShaderDebugSettings};

use super::{
    pipeline::{ExtractedConeLights, RenderConeLight},
    ExtractedVolumetricDebugSettings, ExtractedVolumetricSettings, RenderVolumetricLightingMode,
    VolumetricCone, VolumetricLightingMode, VolumetricLightingState,
};

pub(super) fn extract_volumetric_mode(
    mut commands: Commands,
    mode: Extract<Res<VolumetricLightingState>>,
) {
    commands.insert_resource(RenderVolumetricLightingMode(mode.mode));
}

pub(super) fn extract_volumetric_settings(
    mut commands: Commands,
    settings: Extract<Res<RenderSettings>>,
) {
    debug_assert!(
        settings.volumetric_cone_intensity.is_finite(),
        "volumetric_cone_intensity is not finite"
    );
    debug_assert!(
        settings.volumetric_cone_distance_falloff.is_finite(),
        "volumetric_cone_distance_falloff is not finite"
    );
    debug_assert!(
        settings.volumetric_cone_angular_softness.is_finite(),
        "volumetric_cone_angular_softness is not finite"
    );
    debug_assert!(
        settings.volumetric_cone_extinction.is_finite(),
        "volumetric_cone_extinction is not finite"
    );
    commands.insert_resource(ExtractedVolumetricSettings {
        scatter_strength: settings.volumetric_cone_intensity.max(0.0),
        distance_falloff: settings.volumetric_cone_distance_falloff.clamp(0.0, 10.0),
        angular_softness: settings.volumetric_cone_angular_softness.clamp(0.0, 0.5),
        extinction: settings.volumetric_cone_extinction.clamp(0.0, 10.0),
    });
}

pub(super) fn extract_volumetric_debug_settings(
    mut commands: Commands,
    settings: Extract<Res<VolumetricConeShaderDebugSettings>>,
) {
    commands.insert_resource(ExtractedVolumetricDebugSettings {
        debug_mode: settings.debug_mode,
    });
}

#[allow(clippy::type_complexity)]
pub(super) fn extract_cone_lights(
    mut commands: Commands,
    state: Extract<Res<VolumetricLightingState>>,
    lights: Extract<
        Query<(
            Entity,
            &SpotLight,
            &GlobalTransform,
            Option<&Children>,
            Option<&ViewVisibility>,
        )>,
    >,
    cones_query: Extract<
        Query<(Entity, &GlobalTransform, &Mesh3d, Option<&ViewVisibility>), With<VolumetricCone>>,
    >,
) {
    let mut cones = Vec::new();
    if matches!(state.mode, VolumetricLightingMode::RaymarchCones) {
        let mut cone_data: HashMap<Entity, (Handle<Mesh>, Mat4, bool)> = HashMap::default();
        for (entity, transform, mesh, visibility) in cones_query.iter() {
            let visible = visibility.is_none_or(|v| v.get());
            cone_data.insert(
                entity,
                (mesh.0.clone(), transform.compute_matrix(), visible),
            );
        }

        for (entity, light, transform, children, visibility) in lights.iter() {
            if let Some(view_visibility) = visibility {
                if !view_visibility.get() {
                    continue;
                }
            }

            debug_assert!(
                light.range.is_finite(),
                "SpotLight {entity:?} has non-finite range"
            );
            debug_assert!(
                light.intensity.is_finite(),
                "SpotLight {entity:?} has non-finite intensity"
            );
            if light.range <= 0.1 {
                continue;
            }

            let mut mesh_and_model = None;
            if let Some(children) = children {
                for child in children.iter() {
                    if let Some((mesh, model, cone_visible)) = cone_data.get(&child) {
                        mesh_and_model = Some((mesh.clone(), *model, *cone_visible));
                        break;
                    }
                }
            }
            let Some((mesh, model, cone_visible)) = mesh_and_model else {
                continue;
            };
            if !cone_visible {
                continue;
            };

            let world_transform = transform.compute_transform();
            let direction = (world_transform.rotation * Vec3::NEG_Z).normalize_or_zero();
            debug_assert!(
                direction.length_squared() > 0.0,
                "SpotLight {entity:?} produced zero direction"
            );
            debug_assert!(
                (direction.length_squared() - 1.0).abs() < 1e-3,
                "SpotLight {entity:?} direction not normalized: {direction:?}"
            );

            let cos_inner = light.inner_angle.cos();
            let cos_outer = light.outer_angle.cos();
            debug_assert!(
                cos_inner >= cos_outer - 1e-3,
                "SpotLight {entity:?} inner angle >= outer angle violated: cos_inner={cos_inner:?}, cos_outer={cos_outer:?}"
            );

            cones.push(RenderConeLight {
                light_entity: entity,
                apex: world_transform.translation,
                direction,
                range: light.range,
                intensity: light.intensity,
                color: light.color.into(),
                cos_inner,
                cos_outer,
                mesh,
                model,
            });
        }
    }

    commands.insert_resource(ExtractedConeLights { cones });
}
