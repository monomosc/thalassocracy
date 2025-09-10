use bevy::core_pipeline::bloom::Bloom;
use bevy::prelude::*;
use bevy::math::primitives::Cuboid;
use bevy::pbr::{MeshMaterial3d, StandardMaterial, DistanceFog, FogFalloff};
use bevy::render::render_resource::TextureUsages;
use bevy::prelude::Mesh3d;
use bevy::core_pipeline::tonemapping::Tonemapping;

use super::camera::{FollowCam, FollowCamState, CameraMode};

pub fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    size: Vec3,
    pos: Vec3,
    color: Color,
) -> Entity {
    let mesh = meshes.add(Mesh::from(Cuboid::new(size.x, size.y, size.z)));
    let material = materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..Default::default()
    });
    commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(pos),
            GlobalTransform::default(),
        ))
        .id()
}

pub fn setup_scene(mut commands: Commands) {
    // Global background clear to deep teal
    commands.insert_resource(ClearColor(Color::srgb(0.02, 0.06, 0.08)));
    // Lower ambient to feel subterranean; adjust later with floodlights.
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.25, 0.55, 0.65),
        brightness: 80.0,
        affects_lightmapped_meshes: true,
    });

    // Keep a dim directional light for basic shading; we'll revisit later.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.65, 0.8, 0.9),
            illuminance: 3_500.0,
            shadows_enabled: false,
            ..Default::default()
        },
        Transform::from_xyz(8.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::default(),
        Name::new("Sun Light"),
    ));

    // Follow camera with fog and tonemapping.
    commands.spawn((
        Camera3d { depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING).into(), ..Default::default() },
        Camera {
            hdr: true,
            ..default()
        },
        Bloom::OLD_SCHOOL,
        // Subtle blue‑green fog tuned for underwater caves.
        DistanceFog {
            color: Color::srgb(0.04, 0.11, 0.12),
            falloff: FogFalloff::Exponential { density: 0.05 },
            ..Default::default()
        },
        Tonemapping::TonyMcMapface,
        Msaa::Off,
        Transform::from_xyz(0.0, 4.0, 10.0),
        GlobalTransform::default(),
        FollowCam {
            distance: 7.0,
            height: 2.2,
            stiffness: 6.0,
        },
        FollowCamState { last_dir: Vec3::NEG_X },
        CameraMode { free: false },
        Name::new("Follow Camera"),
    ));
}
