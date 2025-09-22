use bevy::math::primitives::{Cuboid, Sphere};
use bevy::prelude::*;
use levels::builtins::greybox_level;

#[derive(Component)]
pub struct OreNode;

#[derive(Component)]
struct OrePulse {
    phase: f32,
    amp: f32,
}

pub struct OrePlugin;

impl Plugin for OrePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_demo_ore)
            .add_systems(Update, pulse_ore_emissive);
    }
}

fn spawn_demo_ore(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let level = greybox_level();
    // Place ore somewhere visible in the chamber, off-center a bit
    let p = level.chamber.pos;
    let pos = Vec3::new(p.x + 6.0, p.y - 17.0, p.z + 5.0);
    let root = commands
        .spawn((
            Transform::from_translation(pos),
            GlobalTransform::default(),
            Visibility::default(),
            OreNode,
            OrePulse {
                phase: 0.0,
                amp: 1.0,
            },
            Name::new("Ore Node"),
        ))
        .id();

    // Core glow sphere
    let core_mesh = meshes.add(Mesh::from(Sphere::new(0.3)));
    let core_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.9, 0.7),
        emissive: LinearRgba::rgb(25.0, 18.0, 4.0),
        metallic: 0.9,
        perceptual_roughness: 0.35,
        ..Default::default()
    });

    commands.spawn((
        Mesh3d(core_mesh),
        MeshMaterial3d(core_mat),
        Transform::IDENTITY,
        GlobalTransform::default(),
        Name::new("Ore Core"),
        ChildOf(root),
    ));

    // Crystals: stretched cuboids in radial layout
    let shard_mesh = meshes.add(Mesh::from(Cuboid::new(0.15, 0.15, 1.2)));
    for i in 0..6 {
        // 6 shards
        let t = i as f32 * std::f32::consts::TAU / 6.0;
        let yaw = Quat::from_rotation_y(t);
        let pitch = Quat::from_rotation_x((t * 1.7).sin() * 0.3);
        let rot = yaw * pitch;
        let off = rot * Vec3::new(0.0, 0.0, 0.5);
        let shard_t = Transform::from_translation(off).with_rotation(rot);
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.95, 0.85),
            emissive: LinearRgba::rgb(15.0, 11.0, 3.0),
            metallic: 1.0,
            perceptual_roughness: 0.28,
            reflectance: 0.9,
            ..Default::default()
        });
        commands.spawn((
            Mesh3d(shard_mesh.clone()),
            MeshMaterial3d(mat),
            shard_t,
            GlobalTransform::default(),
            Name::new(format!("Ore Shard #{i}")),
            ChildOf(root),
        ));
    }

    // Light bulb child: couples emissive and spotlight via strength
    let bulb = commands
        .spawn((
            super::light_bulb::LightBulb {
                color: Color::srgb(1.0, 0.95, 0.8),
                strength: 1.6,
            },
            Transform::from_translation(Vec3::new(0.0, 0.5, 0.0))
                .looking_at(Vec3::new(0.0, 0.5, 1.0), Vec3::Y),
            GlobalTransform::default(),
            Name::new("Ore LightBulb"),
            ChildOf(root),
        ))
        .id();
    let _ = bulb;
}

fn pulse_ore_emissive(
    time: Res<Time>,
    q_roots: Query<(&OrePulse, &Children), With<OreNode>>,
    mut q_mat: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut q_lights: Query<&mut PointLight>,
) {
    let t = time.elapsed_secs();
    for (pulse, children) in &q_roots {
        // Compute a gentle pulse
        let s = 0.75 + 0.25 * (t * 1.3 + pulse.phase).sin() * pulse.amp.max(0.0);
        for c in children.iter() {
            if let Ok(mh) = q_mat.get_mut(c) {
                if let Some(m) = mats.get_mut(&mh.0) {
                    m.emissive = LinearRgba::rgb(25.0 * s, 18.0 * s, 4.0 * s);
                }
            } else if let Ok(mut pl) = q_lights.get_mut(c) {
                pl.intensity = 20_000.0 * s * s;
            }
        }
    }
}
