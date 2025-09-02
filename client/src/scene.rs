use bevy::prelude::*;
use bevy::color::{LinearRgba, Srgba};
use bevy::math::primitives::Cuboid;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};

#[derive(Component)]
pub struct StationRoom;

#[derive(Component)]
pub struct Tunnel;

#[derive(Component)]
pub struct Chamber;

#[derive(Component)]
pub struct DockPad;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_scene, spawn_greybox));
    }
}

fn spawn_box(
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

fn setup_scene(mut commands: Commands) {
    // Gentle ambient light so unlit meshes are visible.
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        affects_lightmapped_meshes: true,
    });

    // Directional light for basic shading.
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: false,
            ..Default::default()
        },
        Transform::from_xyz(8.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::default(),
    ));

    // Simple 3D camera looking at origin.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(12.0, 10.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::default(),
    ));
}

fn spawn_greybox(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Reusable colors (StandardMaterial base_color: Color, emissive: LinearRgba)
    let wall_color: Color = Color::from(Srgba::new(0.35, 0.38, 0.42, 1.0));
    let floor_color: Color = Color::from(Srgba::new(0.20, 0.22, 0.25, 1.0));
    let tunnel_color: Color = Color::from(Srgba::new(0.25, 0.30, 0.35, 1.0));
    let chamber_color: Color = Color::from(Srgba::new(0.30, 0.32, 0.34, 1.0));
    let dock_emissive: LinearRgba = LinearRgba::from(Srgba::new(0.0, 0.8, 0.9, 1.0));

    // Station room (centered near origin)
    let room_w = 20.0;
    let room_h = 6.0;
    let room_d = 20.0;
    let wall_thick = 0.5;

    // Floor
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, wall_thick, room_d),
        Vec3::new(0.0, -wall_thick * 0.5, 0.0),
        floor_color,
    );
    // Walls
    // +X wall
    let wall_e = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(wall_thick, room_h, room_d),
        Vec3::new(room_w * 0.5, room_h * 0.5 - wall_thick, 0.0),
        wall_color,
    );
    commands.entity(wall_e).insert(StationRoom);
    // -X wall
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(wall_thick, room_h, room_d),
        Vec3::new(-room_w * 0.5, room_h * 0.5 - wall_thick, 0.0),
        wall_color,
    );
    // +Z wall
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, room_h, wall_thick),
        Vec3::new(0.0, room_h * 0.5 - wall_thick, room_d * 0.5),
        wall_color,
    );
    // -Z wall
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, room_h, wall_thick),
        Vec3::new(0.0, room_h * 0.5 - wall_thick, -room_d * 0.5),
        wall_color,
    );

    // Docking pad in the station
    {
        let mesh = meshes.add(Mesh::from(Cuboid::new(3.0, 0.2, 3.0)));
        let material = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: dock_emissive, ..Default::default() });
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(-4.0, 0.1, -4.0),
            GlobalTransform::default(),
            DockPad,
        ));
    }

    // Tunnel from +X wall outward
    let tunnel_len = 24.0;
    let tunnel_size = Vec3::new(tunnel_len, 3.0, 4.0);
    let tunnel_pos = Vec3::new(room_w * 0.5 + tunnel_len * 0.5, 1.0, 0.0);
    {
        let mesh = meshes.add(Mesh::from(Cuboid::new(
            tunnel_size.x,
            tunnel_size.y,
            tunnel_size.z,
        )));
        let material = materials.add(StandardMaterial { base_color: tunnel_color, ..Default::default() });
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(tunnel_pos),
            GlobalTransform::default(),
            Tunnel,
        ));
    }

    // Mining chamber at the end of the tunnel
    let chamber_size = Vec3::new(16.0, 6.0, 16.0);
    let chamber_pos = Vec3::new(room_w * 0.5 + tunnel_len + chamber_size.x * 0.5, 1.0, 0.0);
    {
        let mesh = meshes.add(Mesh::from(Cuboid::new(
            chamber_size.x,
            chamber_size.y,
            chamber_size.z,
        )));
        let material = materials.add(StandardMaterial { base_color: chamber_color, ..Default::default() });
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(chamber_pos),
            GlobalTransform::default(),
            Chamber,
        ));
    }
}
