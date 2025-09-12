use bevy::prelude::*;
use bevy::math::primitives::Cuboid;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::Mesh3d;

// Cameras are spawned in world.rs (single unified camera)

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
            illuminance: 100.0, //very dim, the floodlight works OK for now
            shadows_enabled: false,
            ..Default::default()
        },
        Transform::from_xyz(8.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::default(),
        Name::new("Sun Light"),
    ));

    // No camera spawned here; world::spawn_greybox creates the single GameCamera.
}
