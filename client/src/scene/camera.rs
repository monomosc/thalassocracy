use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CamMode {
    FirstPerson,
    Follow,
    Free,
}

#[derive(Component)]
pub struct GameCamera;

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

#[allow(clippy::type_complexity)]
pub fn update_game_camera(
    time: Res<Time>,
    q_sub: Query<&Transform, With<Submarine>>,
    mut q_cam: Query<
        (&mut Transform, &FollowCam, &mut FollowCamState, &CamMode),
        (With<GameCamera>, Without<Submarine>),
    >,
) {
    let Ok(sub_t) = q_sub.single() else {
        return;
    };
    let sub_pos = sub_t.translation;
    let orient_dir = (sub_t.rotation * Vec3::X).normalize_or_zero();

    for (mut cam_t, cam, mut state, mode) in &mut q_cam {
        match *mode {
            CamMode::Follow => {
                let dir = if orient_dir.length_squared() > 1e-6 {
                    orient_dir
                } else {
                    state.last_dir
                };
                state.last_dir = dir;
                let desired_pos = sub_pos - dir * cam.distance + Vec3::Y * cam.height;
                let stiffness = cam.stiffness.max(0.0);
                let dt = time.delta_secs();
                let lerp = 1.0 - (-stiffness * dt).exp();
                cam_t.translation = cam_t.translation.lerp(desired_pos, lerp);
                cam_t.look_at(sub_pos, Vec3::Y);
            }
            CamMode::FirstPerson => {
                // Lock camera to the submarine with an orientation offset:
                // Camera looks along its local -Z; sub "forward" is +X (mesh space).
                // Apply a -90 deg yaw so camera -Z aligns with sub +X.
                let fp_offset = Vec3::new(1.0, 0.0, 0.0);
                cam_t.translation = sub_pos + (sub_t.rotation * fp_offset);
                let yaw_minus_90 = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
                cam_t.rotation = sub_t.rotation * yaw_minus_90;
                state.last_dir = orient_dir;
            }
            CamMode::Free => { /* handled by free_fly_camera */ }
        }
    }
}

pub fn switch_cameras_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<&mut CamMode, With<GameCamera>>,
) {
    let Ok(mut mode) = q.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::F11) {
        *mode = if *mode == CamMode::Follow {
            CamMode::FirstPerson
        } else {
            CamMode::Follow
        };
    }
    if keys.just_pressed(KeyCode::F12) {
        *mode = if *mode == CamMode::Free {
            CamMode::FirstPerson
        } else {
            CamMode::Free
        };
    }
}

pub fn free_fly_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut q: Query<(&mut Transform, &mut FreeFlyState, &CamMode), With<GameCamera>>,
) {
    let Ok((mut t, mut state, mode)) = q.single_mut() else {
        return;
    };
    if *mode != CamMode::Free {
        return;
    }

    // Mouse look (hold right mouse button)
    if mouse_buttons.pressed(MouseButton::Right) {
        const SENS: f32 = 0.0025;
        let mut delta = Vec2::ZERO;
        for ev in mouse_motion.read() {
            delta += ev.delta;
        }
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
    if keys.pressed(KeyCode::KeyW) {
        dir += *t.forward();
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= *t.forward();
    }
    if keys.pressed(KeyCode::KeyA) {
        dir -= *t.right();
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += *t.right();
    }
    if keys.pressed(KeyCode::KeyE) {
        dir += Vec3::Y;
    }
    if keys.pressed(KeyCode::KeyQ) {
        dir -= Vec3::Y;
    }

    let mut speed = state.speed;
    if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        speed *= 4.0;
    }
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        speed *= 0.25;
    }

    if dir.length_squared() > 0.0 {
        t.translation += dir.normalize() * speed * time.delta_secs();
    }
}
