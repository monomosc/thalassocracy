use bevy::pbr::{MeshMaterial3d, NotShadowCaster, StandardMaterial};
use bevy::prelude::AlphaMode;
use bevy::prelude::*;

use super::submarine::Submarine;
use crate::scene::flow_field::{FlowField, Tunnel, TunnelBounds};

// ---------- Plugin ----------

pub struct WaterFxPlugin;

impl Plugin for WaterFxPlugin {
    fn build(&self, app: &mut App) {
        /* app.init_resource::<UnderwaterAssets>()
        .init_resource::<UnderwaterSettings>()
        .add_systems(Startup, setup_underwater_assets)
        .add_systems(
            Update,
            (
                tune_camera_underwater,
                //ensure_mote_field,
                //tick_motes,
                ensure_bubble_emitter,
                spawn_bubbles,
                tick_bubbles,
            ),
        );*/
    }
}

// ---------- Assets / Materials ----------

#[derive(Resource, Default)]
pub struct UnderwaterAssets {
    mote_mesh: Handle<Mesh>,
    mote_mat: Handle<StandardMaterial>,
    bubble_mesh: Handle<Mesh>,
    bubble_mat: Handle<StandardMaterial>,
}

/// Runtime toggles for underwater FX.
#[derive(Resource, Default)]
pub struct UnderwaterSettings {
    /// Leave bubbles off by default for now.
    pub bubbles_enabled: bool,
}

fn setup_underwater_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut assets: ResMut<UnderwaterAssets>,
) {
    // Tiny unlit sphere for dust motes
    let mote_mesh = meshes.add(Mesh::from(bevy::math::primitives::Sphere::new(0.02)));
    let mote_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.65, 0.85, 0.9).with_alpha(0.2),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..Default::default()
    });

    // Slightly larger transparent sphere for bubbles
    let bubble_mesh = meshes.add(Mesh::from(bevy::math::primitives::Sphere::new(0.03)));
    let bubble_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.95, 1.0).with_alpha(0.4),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        // Fresnel-like hack: non-shadowing and double-sided feel via cull off
        cull_mode: None,
        ..Default::default()
    });

    *assets = UnderwaterAssets {
        mote_mesh,
        mote_mat,
        bubble_mesh,
        bubble_mat,
    };
}
// ---------- Camera tuning ----------

#[derive(Component)]
struct UnderwaterCameraTuned;

#[allow(clippy::type_complexity)]
fn tune_camera_underwater(
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &mut Camera3d,
            Option<&DistanceFog>,
            Option<&UnderwaterCameraTuned>,
            &Transform,
            &Camera,
        ),
        Without<UnderwaterCameraTuned>,
    >,
) {
    for (e, _cam3d, _fog_opt, tuned, _t, cam) in &mut q {
        let _ = tuned; // filter via query
        if !cam.is_active {
            continue;
        }
        commands.entity(e).insert(UnderwaterCameraTuned);
    }
}

// ---------- Dust motes ----------

#[derive(Component)]
struct MoteField {
    radius: f32,
}

#[derive(Component)]
struct UnderwaterMote {
    vel: Vec3,
}

#[allow(clippy::type_complexity)]
fn ensure_mote_field(
    mut commands: Commands,
    q_field: Query<Entity, With<MoteField>>,
    q_cam: Query<(Entity, &Transform, &Camera), (With<Camera3d>, Without<MoteField>)>,
    assets: Res<UnderwaterAssets>,
) {
    if q_field.iter().next().is_some() {
        return;
    }
    // Choose the active camera
    let mut active: Option<Transform> = None;
    for (_e, t, cam) in &q_cam {
        if cam.is_active {
            active = Some(*t);
            break;
        }
    }
    let Some(cam_t) = active else {
        return;
    };

    let radius = 8.0_f32;
    let count = 160_usize;

    let field_e = commands
        .spawn((
            Transform::from_translation(cam_t.translation),
            GlobalTransform::default(),
            Visibility::default(),
            MoteField { radius },
            Name::new("MoteField"),
        ))
        .id();

    let mut rng_seed = 0x1234_5678_u32;
    let mut frand = || {
        // xorshift32
        rng_seed ^= rng_seed << 13;
        rng_seed ^= rng_seed >> 17;
        rng_seed ^= rng_seed << 5;
        (rng_seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };

    for _ in 0..count {
        let pos = cam_t.translation
            + Vec3::new(frand(), frand(), frand()).normalize_or_zero()
                * (radius * 0.9 * frand().abs());
        let vel = Vec3::new(frand() * 0.05, 0.05 + frand() * 0.02, frand() * 0.05);
        commands.spawn((
            Mesh3d(assets.mote_mesh.clone()),
            MeshMaterial3d(assets.mote_mat.clone()),
            Transform::from_translation(pos),
            GlobalTransform::default(),
            UnderwaterMote { vel },
            NotShadowCaster,
            Name::new("Mote"),
            ChildOf(field_e),
        ));
    }
}

#[allow(clippy::type_complexity)]
fn tick_motes(
    time: Res<Time>,
    mut q_field: Query<(&mut Transform, &MoteField)>,
    q_cam: Query<(&Transform, &Camera), (With<Camera3d>, Without<MoteField>)>,
    mut q_motes: Query<
        (&mut Transform, &mut UnderwaterMote),
        (Without<Camera3d>, Without<MoteField>),
    >,
    q_flow: Query<(&GlobalTransform, &FlowField, &TunnelBounds), With<Tunnel>>,
) {
    let mut cam_opt: Option<Transform> = None;
    for (t, cam) in &q_cam {
        if cam.is_active {
            cam_opt = Some(*t);
            break;
        }
    }
    let Some(cam_t) = cam_opt else {
        return;
    };
    let dt = time.delta_secs().clamp(0.0, 0.05);

    if let Ok((mut field_t, field)) = q_field.single_mut() {
        // Keep field centered on camera smoothly
        let lerp = 1.0 - (-4.0 * dt).exp();
        field_t.translation = field_t.translation.lerp(cam_t.translation, lerp);

        // Sample first flow field if available
        let flow = if let Ok((_gt, ff, _tb)) = q_flow.single() {
            let (v, variance) = ff.sample(field_t.translation, time.elapsed_secs());
            v + Vec3::new(0.0, 0.05 + variance * 0.02, 0.0)
        } else {
            Vec3::new(0.0, 0.06, 0.0)
        };

        for (mut t, mut mote) in &mut q_motes {
            let jitter = Vec3::new(
                (time.elapsed_secs() * 0.9 + t.translation.x).sin() * 0.01,
                (time.elapsed_secs() * 1.1 + t.translation.y).cos() * 0.01,
                (time.elapsed_secs() * 1.3 + t.translation.z).sin() * 0.01,
            );
            mote.vel = mote.vel.lerp(flow + jitter, 0.1);
            t.translation += mote.vel * dt;

            // Recycle motes far outside the sphere
            let d = (t.translation - field_t.translation).length();
            if d > field.radius {
                let dir = (t.translation - field_t.translation).normalize_or_zero();
                t.translation = field_t.translation - dir * (field.radius * 0.9);
            }
        }
    }
}

// ---------- Bubbles ----------

#[derive(Component)]
struct BubbleEmitter {
    cooldown: f32,
}

#[derive(Component)]
struct Bubble {
    ttl: f32,
    rise: f32,
}

fn ensure_bubble_emitter(
    mut commands: Commands,
    q_emit: Query<Entity, With<BubbleEmitter>>,
    q_sub: Query<Entity, With<Submarine>>,
) {
    if q_emit.single().is_ok() {
        return;
    }
    let Ok(sub_e) = q_sub.single() else {
        return;
    };
    commands
        .entity(sub_e)
        .insert(BubbleEmitter { cooldown: 0.1 });
}

fn spawn_bubbles(
    time: Res<Time>,
    mut commands: Commands,
    mut q_emit: Query<(&mut BubbleEmitter, &GlobalTransform), With<Submarine>>,
    assets: Res<UnderwaterAssets>,
    settings: Option<Res<UnderwaterSettings>>,
) {
    if !settings.map(|s| s.bubbles_enabled).unwrap_or(false) {
        return;
    }
    let Ok((mut em, gt)) = q_emit.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    em.cooldown -= dt;
    if em.cooldown > 0.0 {
        return;
    }
    em.cooldown = 0.06; // spawn rate

    // Spawn a small cluster near the stern (-X of sub local space)
    let stern =
        gt.translation() + (gt.rotation() * Vec3::NEG_Z) * 1.2 + (gt.rotation() * Vec3::Y) * 0.1;
    let right = gt.rotation() * Vec3::X;
    let up = gt.rotation() * Vec3::Y;

    for i in 0..3 {
        let f = i as f32 * 0.37;
        let pos = stern + right * (f.sin() * 0.05) + up * (f.cos() * 0.04);
        commands.spawn((
            Mesh3d(assets.bubble_mesh.clone()),
            MeshMaterial3d(assets.bubble_mat.clone()),
            Transform::from_translation(pos),
            GlobalTransform::default(),
            Bubble {
                ttl: 1.8,
                rise: 0.9,
            },
            NotShadowCaster,
            Name::new("Bubble"),
        ));
    }
}

fn tick_bubbles(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut Bubble)>,
    settings: Option<Res<UnderwaterSettings>>,
) {
    if !settings.map(|s| s.bubbles_enabled).unwrap_or(false) {
        return;
    }
    let dt = time.delta_secs();
    for (e, mut t, mut b) in &mut q {
        b.ttl -= dt;
        if b.ttl <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Rise and drift
        let s = 1.0 + (1.8 - b.ttl) * 0.1;
        t.translation += Vec3::new(0.0, b.rise * dt, 0.0);
        t.translation.x += (time.elapsed_secs() * 2.3 + t.translation.y).sin() * 0.01;
        t.translation.z += (time.elapsed_secs() * 1.9 + t.translation.x).cos() * 0.01;
        t.scale = Vec3::splat(s);
    }
}
