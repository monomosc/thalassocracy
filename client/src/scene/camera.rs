use bevy::prelude::*;
use bevy::input::mouse::MouseMotion;

#[derive(Component)]
pub struct CameraMode {
    pub free: bool,
}

#[derive(Component)]
pub struct FreeFlyState {
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
}

#[derive(Component)]
pub struct FollowCam {
    pub distance: f32,
    pub height: f32,
    pub stiffness: f32, // larger = snappier follow
}

#[derive(Component)]
pub struct FollowCamState {
    pub last_dir: Vec3,
}

use super::submarine::Submarine;

pub fn update_follow_camera(
    time: Res<Time>,
    q_sub: Query<&Transform, With<Submarine>>,
    mut q_cam: Query<(&mut Transform, &FollowCam, &mut FollowCamState, &CameraMode), Without<Submarine>>,
) {
    let Ok(sub_t) = q_sub.single() else { return; };
    let sub_pos = sub_t.translation;
    // Forward direction from orientation (body +X)
    let orient_dir = (sub_t.rotation * Vec3::X).normalize_or_zero();

    for (mut cam_t, cam, mut state, mode) in &mut q_cam {
        if mode.free { continue; }
        let dir = if orient_dir.length_squared() > 1e-6 { orient_dir } else { state.last_dir };
        state.last_dir = dir;

        // Place camera behind the submarine relative to its orientation
        let desired_pos = sub_pos - dir * cam.distance + Vec3::Y * cam.height;

        let stiffness = cam.stiffness.max(0.0);
        let dt = time.delta_secs();
        let lerp = 1.0 - (-stiffness * dt).exp();
        cam_t.translation = cam_t.translation.lerp(desired_pos, lerp);
        cam_t.look_at(sub_pos, Vec3::Y);
    }
}

pub fn toggle_camera_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut q: Query<(Entity, &Transform, &mut CameraMode, Option<&mut FreeFlyState>)>,
) {
    if !keys.just_pressed(KeyCode::F12) { return; }
    if let Ok((e, t, mut mode, free_state)) = q.single_mut() {
        mode.free = !mode.free;
        if mode.free {
            // Enter free mode: initialize yaw/pitch from current transform
            let (yaw, pitch, _roll) = t.rotation.to_euler(EulerRot::YXZ);
            if free_state.is_none() {
                commands.entity(e).insert(FreeFlyState { yaw, pitch, speed: 8.0 });
            }
        }
    }
}

pub fn free_fly_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut q: Query<(&mut Transform, &mut FreeFlyState, &CameraMode)>,
) {
    let Ok((mut t, mut state, mode)) = q.single_mut() else { return; };
    if !mode.free { return; }

    // Mouse look (hold right mouse button)
    if mouse_buttons.pressed(MouseButton::Right) {
        const SENS: f32 = 0.0025;
        let mut delta = Vec2::ZERO;
        for ev in mouse_motion.read() { delta += ev.delta; }
        state.yaw -= delta.x * SENS;
        state.pitch -= delta.y * SENS;
        state.pitch = state.pitch.clamp(-1.5, 1.5);
        t.rotation = Quat::from_euler(EulerRot::YXZ, state.yaw, state.pitch, 0.0);
    } else {
        // Drain motion to avoid bursts when RMB is pressed next
        for _ in mouse_motion.read() {}
    }

    // Movement
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) { dir += *t.forward(); }
    if keys.pressed(KeyCode::KeyS) { dir -= *t.forward(); }
    if keys.pressed(KeyCode::KeyA) { dir -= *t.right(); }
    if keys.pressed(KeyCode::KeyD) { dir += *t.right(); }
    if keys.pressed(KeyCode::KeyE) { dir += Vec3::Y; }
    if keys.pressed(KeyCode::KeyQ) { dir -= Vec3::Y; }

    let mut speed = state.speed;
    if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) { speed *= 4.0; }
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) { speed *= 0.25; }

    if dir.length_squared() > 0.0 {
        t.translation += dir.normalize() * speed * time.delta_secs();
    }
}
