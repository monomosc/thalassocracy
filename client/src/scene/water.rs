use bevy::prelude::*;
use bevy::pbr::{MeshMaterial3d, NotShadowCaster, StandardMaterial};
use bevy::render::mesh::Indices;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::prelude::AlphaMode;

use super::camera::{CameraMode, FollowCam};
use super::submarine::Submarine;
use crate::scene::world::{FlowField, Tunnel, TunnelBounds};

// ---------- Plugin ----------

pub struct WaterFxPlugin;

impl Plugin for WaterFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnderwaterAssets>()
            .init_resource::<UnderwaterSettings>()
            .add_systems(Startup, setup_underwater_assets)
            .add_systems(
                Update,
                (
            tune_camera_underwater,
            ensure_mote_field,
            tick_motes,
            ensure_bubble_emitter,
            spawn_bubbles,
            tick_bubbles,
            attach_or_update_volumetrics,
            ),
        );
    }
}

// ---------- Assets / Materials ----------

#[derive(Resource, Default)]
pub struct UnderwaterAssets {
    mote_mesh: Handle<Mesh>,
    mote_mat: Handle<StandardMaterial>,
    bubble_mesh: Handle<Mesh>,
    bubble_mat: Handle<StandardMaterial>,
    cone_mat: Handle<StandardMaterial>,
    cone_mesh: Handle<Mesh>,
    halo_mesh: Handle<Mesh>,
    halo_mat: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub struct UnderwaterSettings {
    pub bubbles_enabled: bool,
    pub volumetrics_enabled: bool,
}

impl Default for UnderwaterSettings {
    fn default() -> Self {
        // Leave bubbles in code, but off by default for now.
        Self { bubbles_enabled: false, volumetrics_enabled: true }
    }
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

    // Additive, unlit blue-green for the volumetric cone
    let cone_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.8, 1.0).with_alpha(0.12),
        emissive: LinearRgba::new(0.18, 0.6, 0.9, 0.0),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..Default::default()
    });

    // Volumetric halo for point lights
    let halo_mesh = meshes.add(Mesh::from(bevy::math::primitives::Sphere::new(1.0)));
    let halo_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.85, 1.0).with_alpha(0.06),
        emissive: LinearRgba::new(0.15, 0.5, 0.8, 0.0),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        ..Default::default()
    });

    // Unit cone along -Z, apex at origin, base at z=-1
    let cone_mesh = meshes.add(make_unit_cone_negz(32));

    *assets = UnderwaterAssets {
        mote_mesh,
        mote_mat,
        bubble_mesh,
        bubble_mat,
        cone_mat,
        cone_mesh,
        halo_mesh,
        halo_mat,
    };
}

// ---------- Camera tuning ----------

#[derive(Component)]
struct UnderwaterCameraTuned;

fn tune_camera_underwater(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Camera3d, Option<&DistanceFog>, Option<&UnderwaterCameraTuned>, &Transform, &CameraMode, &FollowCam), Without<UnderwaterCameraTuned>>,
) {
    for (e, _cam3d, _fog_opt, tuned, _t, mode, _follow) in &mut q {
        let _ = tuned; // filter via query
        if mode.free { continue; }
        let _ = e; // keep extension point for later
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

fn ensure_mote_field(
    mut commands: Commands,
    q_field: Query<Entity, With<MoteField>>,
    q_cam: Query<(Entity, &Transform), (With<Camera3d>, Without<MoteField>)>,
    assets: Res<UnderwaterAssets>,
) {
    if q_field.iter().next().is_some() { return; }
    let Ok((_cam_e, cam_t)) = q_cam.get_single() else { return; };

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
            + Vec3::new(frand(), frand(), frand()).normalize_or_zero() * (radius * 0.9 * frand().abs());
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

fn tick_motes(
    time: Res<Time>,
    mut q_field: Query<(&mut Transform, &MoteField)>,
    q_cam: Query<&Transform, (With<Camera3d>, Without<MoteField>)>,
    mut q_motes: Query<(&mut Transform, &mut UnderwaterMote), (Without<Camera3d>, Without<MoteField>)>,
    q_flow: Query<(&GlobalTransform, &FlowField, &TunnelBounds), With<Tunnel>>,
) {
    let Ok(cam_t) = q_cam.get_single() else { return; };
    let dt = time.delta_secs().clamp(0.0, 0.05);

    if let Ok((mut field_t, field)) = q_field.get_single_mut() {
        // Keep field centered on camera smoothly
        let lerp = 1.0 - (-4.0 * dt).exp();
        field_t.translation = field_t.translation.lerp(cam_t.translation, lerp);

        // Sample first flow field if available
        let flow = if let Ok((_gt, ff, _tb)) = q_flow.get_single() {
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
    if q_emit.single().is_ok() { return; }
    let Ok(sub_e) = q_sub.get_single() else { return; };
    commands.entity(sub_e).insert(BubbleEmitter { cooldown: 0.1 });
}

fn spawn_bubbles(
    time: Res<Time>,
    mut commands: Commands,
    mut q_emit: Query<(&mut BubbleEmitter, &GlobalTransform), With<Submarine>>,
    assets: Res<UnderwaterAssets>,
    settings: Option<Res<UnderwaterSettings>>,
) {
    if !settings.map(|s| s.bubbles_enabled).unwrap_or(false) { return; }
    let Ok((mut em, gt)) = q_emit.get_single_mut() else { return; };
    let dt = time.delta_secs();
    em.cooldown -= dt;
    if em.cooldown > 0.0 { return; }
    em.cooldown = 0.06; // spawn rate

    // Spawn a small cluster near the stern (-X of sub local space)
    let stern = gt.translation() + (gt.rotation() * Vec3::NEG_X) * 1.2 + (gt.rotation() * Vec3::Y) * 0.1;
    let right = gt.rotation() * Vec3::Z;
    let up = gt.rotation() * Vec3::Y;

    for i in 0..3 {
        let f = i as f32 * 0.37;
        let pos = stern + right * (f.sin() * 0.05) + up * (f.cos() * 0.04);
        commands.spawn((
            Mesh3d(assets.bubble_mesh.clone()),
            MeshMaterial3d(assets.bubble_mat.clone()),
            Transform::from_translation(pos),
            GlobalTransform::default(),
            Bubble { ttl: 1.8, rise: 0.9 },
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
    if !settings.map(|s| s.bubbles_enabled).unwrap_or(false) { return; }
    let dt = time.delta_secs();
    for (e, mut t, mut b) in &mut q {
        b.ttl -= dt;
        if b.ttl <= 0.0 { commands.entity(e).despawn(); continue; }
        // Rise and drift
        let s = 1.0 + (1.8 - b.ttl) * 0.1;
        t.translation += Vec3::new(0.0, b.rise * dt, 0.0);
        t.translation.x += (time.elapsed_secs() * 2.3 + t.translation.y).sin() * 0.01;
        t.translation.z += (time.elapsed_secs() * 1.9 + t.translation.x).cos() * 0.01;
        t.scale = Vec3::splat(s as f32);
    }
}

// ---------- Volumetric proxies for lights ----------

#[derive(Component)]
struct VolumetricCone;

#[derive(Component)]
struct VolumetricHalo;

fn attach_or_update_volumetrics(
    mut commands: Commands,
    mut q_spot: Query<(Entity, &SpotLight, Option<&Children>)>,
    mut q_cone: Query<(Entity, &mut Transform), (With<VolumetricCone>, Without<VolumetricHalo>)>,
    mut q_point: Query<(Entity, &PointLight, Option<&Children>)>,
    mut q_halo: Query<(Entity, &mut Transform), (With<VolumetricHalo>, Without<VolumetricCone>)>,
    assets: Res<UnderwaterAssets>,
    settings: Option<Res<UnderwaterSettings>>,
    render_settings: Option<Res<crate::render_settings::RenderSettings>>,
) {
    if let Some(s) = settings { if !s.volumetrics_enabled { return; } }
    if let Some(v) = render_settings { 
        if !v.volumetric_cones { 
            // Despawn any existing volumetric proxies when disabled
            for (e, _) in &mut q_cone { commands.entity(e).despawn(); }
            for (e, _) in &mut q_halo { commands.entity(e).despawn(); }
            return; 
        } 
    }

    // SpotLights → volumetric cones
    for (e, light, children) in &mut q_spot {
        if light.range <= 0.1 { continue; }

        let mut cone_e = None;
        if let Some(ch) = children {
            for c in ch.iter() {
                if q_cone.get_mut(c).is_ok() { cone_e = Some(c); break; }
            }
        }

        let height = light.range;
        let radius = (height * light.outer_angle.tan()).max(0.01);
        // Our unit cone points along -Z with apex at origin and base at z=-1.
        // Place it in front of the light along local -Z.
        let cone_t = Transform::from_translation(-Vec3::Z * height * 0.01)
            .with_scale(Vec3::new(radius, radius, height));

        match cone_e {
            Some(c) => {
                if let Ok((_ce, mut t)) = q_cone.get_mut(c) { *t = cone_t; }
            }
            None => {
                let id = commands
                    .spawn((
                        Mesh3d(assets.cone_mesh.clone()),
                        MeshMaterial3d(assets.cone_mat.clone()),
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

    // PointLights → volumetric halos
    for (e, light, children) in &mut q_point {
        if light.range <= 0.1 { continue; }
        let mut halo_e = None;
        if let Some(ch) = children {
            for c in ch.iter() {
                if q_halo.get_mut(c).is_ok() { halo_e = Some(c); break; }
            }
        }
        let scale = (light.range * 0.5).max(0.01);
        let halo_t = Transform::from_scale(Vec3::splat(scale));
        match halo_e {
            Some(h) => {
                if let Ok((_he, mut t)) = q_halo.get_mut(h) { *t = halo_t; }
            }
            None => {
                let id = commands
                    .spawn((
                        Mesh3d(assets.halo_mesh.clone()),
                        MeshMaterial3d(assets.halo_mat.clone()),
                        halo_t,
                        GlobalTransform::default(),
                        VolumetricHalo,
                        NotShadowCaster,
                        Name::new("VolumetricHalo"),
                    ))
                    .id();
                commands.entity(id).insert(ChildOf(e));
            }
        }
    }
}

// Unit cone along -Z: apex at (0,0,0), base circle at z=-1, radius=1
fn make_unit_cone_negz(segments: usize) -> Mesh {
    let segments = segments.max(3);
    let mut positions = Vec::with_capacity(segments + 1);
    let mut normals = Vec::with_capacity(segments + 1);
    let mut uvs = Vec::with_capacity(segments + 1);
    let mut indices: Vec<u32> = Vec::with_capacity(segments * 3);

    // Apex
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push([0.5, 1.0]);

    // Base ring
    for i in 0..segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        let p = Vec3::new(c, s, -1.0);
        positions.push(p.to_array());
        // Side normal approx: direction halfway between radial and axis
        let n = Vec3::new(c, s, 1.0).normalize();
        normals.push(n.to_array());
        uvs.push([i as f32 / segments as f32, 0.0]);
    }

    // Side triangles (winding for front faces outward)
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
