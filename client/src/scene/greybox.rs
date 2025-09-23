use bevy::color::{LinearRgba, Srgba};
use bevy::core_pipeline::bloom::BloomPrefilter;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSamplerDescriptor};
use bevy::math::primitives::{Cuboid, Plane3d, Sphere};
use bevy::math::{Affine2, Vec2};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;

use levels::subspecs::small_skiff_spec;
use levels::{builtins::greybox_level, LevelSpec, Vec3f};

use super::camera::{CamMode, FollowCam, FollowCamState, FreeFlyState, GameCamera};
use super::flow_field::{FlowField, Tunnel, TunnelBounds};
use super::light_bulb::{BlinkingLight, LightBulb};
use super::proctex::ProcTexAssets;
use super::setup::spawn_box;
use super::submarine::{
    make_rudder_prism_mesh, AngularVelocity, Rudder, SubPhysics, Submarine, Velocity,
};
use bevy::render::render_resource::{Face, TextureUsages};

#[derive(Component)]
pub struct StationRoom;

#[derive(Component)]
pub struct Chamber;

// BlinkingLight moved into light_bulb.rs and handled by LightBulbPlugin.

#[derive(Component)]
pub struct DockPad;

pub fn spawn_greybox(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    proc_tex: Option<Res<ProcTexAssets>>,
) {
    // Convert helpers
    fn v(v: Vec3f) -> Vec3 {
        Vec3::new(v.x, v.y, v.z)
    }

    // Reusable colors (StandardMaterial base_color: Color, emissive: LinearRgba)
    let wall_color: Color = Color::from(Srgba::new(0.35, 0.38, 0.42, 1.0));
    let floor_color: Color = Color::from(Srgba::new(0.20, 0.22, 0.25, 1.0));
    let _tunnel_color: Color = Color::from(Srgba::new(0.25, 0.30, 0.35, 1.0));
    let chamber_color: Color = Color::from(Srgba::new(0.30, 0.32, 0.34, 1.0));
    let dock_emissive: LinearRgba = LinearRgba::from(Srgba::new(0.0, 0.8, 0.9, 1.0));

    // Load level spec from shared crate
    let level: LevelSpec = greybox_level();
    let room_w = level.room.size.x;
    let room_h = level.room.size.y;
    let room_d = level.room.size.z;
    let wall_thick = level.room.wall_thickness;

    // Floor
    let e_floor = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, wall_thick, room_d),
        Vec3::new(0.0, -wall_thick * 0.5, 0.0),
        floor_color,
    );
    commands.entity(e_floor).insert(Name::new("Station Floor"));
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
    let e_wall_negx = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(wall_thick, room_h, room_d),
        Vec3::new(-room_w * 0.5, room_h * 0.5 - wall_thick, 0.0),
        wall_color,
    );
    commands
        .entity(e_wall_negx)
        .insert(Name::new("Station Wall -X"));
    // +Z wall
    let e_wall_posz = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, room_h, wall_thick),
        Vec3::new(0.0, room_h * 0.5 - wall_thick, room_d * 0.5),
        wall_color,
    );
    commands
        .entity(e_wall_posz)
        .insert(Name::new("Station Wall +Z"));
    // -Z wall
    let e_wall_negz = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, room_h, wall_thick),
        Vec3::new(0.0, room_h * 0.5 - wall_thick, -room_d * 0.5),
        wall_color,
    );
    commands
        .entity(e_wall_negz)
        .insert(Name::new("Station Wall -Z"));

    // Docking pad in the station
    {
        let dsz = v(level.room.dock_size);
        let mesh = meshes.add(Mesh::from(Cuboid::new(dsz.x, dsz.y, dsz.z)));
        let material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: dock_emissive,
            ..Default::default()
        });
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(v(level.room.dock_pos)),
            GlobalTransform::default(),
            DockPad,
            Name::new("Dock Pad"),
        ));
    }

    // Tunnel from +X wall outward (hollow shell: floor/ceiling/side walls)
    let tunnel_size = Vec3::new(
        level.tunnel.size.x,
        level.tunnel.size.y,
        level.tunnel.size.z,
    );
    let tunnel_pos = Vec3::new(level.tunnel.pos.x, level.tunnel.pos.y, level.tunnel.pos.z);
    let tunnel_entity = {
        // Parent holds the field and bounds. Children are the shell meshes.
        let parent = commands
            .spawn((
                Transform::from_translation(tunnel_pos),
                GlobalTransform::default(),
                Tunnel,
                TunnelBounds { size: tunnel_size },
                Visibility::default(),
                // Flow field from spec
                match level.tunnel.flow {
                    levels::FlowFieldSpec::Uniform { flow, variance } => {
                        FlowField::uniform(v(flow), variance)
                    }
                },
                Name::new("Tunnel"),
            ))
            .id();

        // Use the provided rock albedo; disable depth_map for now to avoid sampler type mismatch from 16-bit PNG
        let tex_albedo: Handle<Image> = asset_server.load_with_settings(
            "textures/rock_face_03_diff_4k.jpg",
            |settings: &mut ImageLoaderSettings| {
                settings.sampler = bevy::image::ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    address_mode_w: ImageAddressMode::Repeat,
                    ..default()
                });
            },
        );

        // Helper to build a material with custom UV tiling and optional flips
        let mut make_mat = |repeats: Vec2, flip_x: bool, flip_y: bool| {
            let mut uv = Affine2::from_scale(repeats);
            if flip_x {
                uv = StandardMaterial::FLIP_VERTICAL * uv;
            }
            if flip_y {
                uv = StandardMaterial::FLIP_HORIZONTAL * uv;
            }
            materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(tex_albedo.clone()),
                metallic: 0.1,
                perceptual_roughness: 0.95,
                // Ensure interior faces render correctly when viewed from inside the tunnel
                cull_mode: None,
                double_sided: true,
                uv_transform: uv,
                ..Default::default()
            })
        };

        // Repeats tuned per face to avoid stretching on long axes
        let rx = 8.0;
        let rz = 2.0;
        let ry = 2.0;
        let mat_floor = make_mat(Vec2::new(rx, rz), false, false);
        let mat_ceil = make_mat(Vec2::new(rx, rz), false, false);
        let mat_wall_pz = make_mat(Vec2::new(rx, ry), false, false);
        let mat_wall_nz = make_mat(Vec2::new(rx, ry), false, false);
        let half = tunnel_size * 0.5;

        // Helper to spawn a single textured plane as a child (avoids cuboid UV issues)
        let mut spawn_plane =
            |size: Vec2, local: Vec3, rot: Quat, name: &str, mat: Handle<StandardMaterial>| {
                let mesh = meshes.add(Plane3d::default().mesh().size(size.x, size.y));
                let child = commands
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(mat),
                        Transform::from_translation(local).with_rotation(rot),
                        GlobalTransform::default(),
                        Name::new(name.to_string()),
                    ))
                    .id();
                commands.entity(child).insert(ChildOf(parent));
            };

        // Floor (XZ plane, normal +Y)
        spawn_plane(
            Vec2::new(tunnel_size.x, tunnel_size.z),
            Vec3::new(0.0, -half.y, 0.0),
            Quat::IDENTITY,
            "Tunnel Floor",
            mat_floor.clone(),
        );
        // Ceiling (XZ plane, normal -Y)
        spawn_plane(
            Vec2::new(tunnel_size.x, tunnel_size.z),
            Vec3::new(0.0, half.y, 0.0),
            Quat::from_rotation_x(std::f32::consts::PI),
            "Tunnel Ceiling",
            mat_ceil.clone(),
        );
        // +Z wall (XY plane, normal -Z)
        spawn_plane(
            Vec2::new(tunnel_size.x, tunnel_size.y),
            Vec3::new(0.0, 0.0, half.z),
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            "Tunnel Wall +Z",
            mat_wall_pz.clone(),
        );
        // -Z wall (XY plane, normal +Z)
        spawn_plane(
            Vec2::new(tunnel_size.x, tunnel_size.y),
            Vec3::new(0.0, 0.0, -half.z),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            "Tunnel Wall -Z",
            mat_wall_nz.clone(),
        );

        // Blinking red bulbs along the top (ceiling centerline) using LightBulb + BlinkingLight
        {
            let inset = 0.02;
            let y = half.y - inset;
            // Denser spacing: 7 bulbs evenly along the tunnel length
            let count = 7;
            for i in 0..count {
                let t = i as f32 / (count - 1) as f32; // 0..1
                let x = -half.x * 0.6 + (half.x * 1.2) * t; // span a bit inside both ends
                let pos = Vec3::new(x, y, 0.0);
                commands
                    .spawn((
                        LightBulb {
                            color: Color::srgb(1.0, 0.2, 0.2),
                            strength: 0.0,
                        },
                        // Brighter blink
                        BlinkingLight {
                            period: 1.0,
                            on_fraction: 0.35,
                            on_intensity: 3.8,
                            off_intensity: 0.0,
                        },
                        Transform::from_translation(pos),
                        GlobalTransform::default(),
                        Name::new(format!("Tunnel Blink Bulb #{i}")),
                    ))
                    .insert(ChildOf(parent));
            }
        }

        parent
    };

    // Mining chamber as a hollow shell with an open entrance toward the tunnel (remove -X wall)
    let chamber_size = Vec3::new(
        level.chamber.size.x,
        level.chamber.size.y,
        level.chamber.size.z,
    );
    let chamber_pos = Vec3::new(
        level.chamber.pos.x,
        level.chamber.pos.y,
        level.chamber.pos.z,
    );
    {
        let parent = commands
            .spawn((
                Transform::from_translation(chamber_pos),
                GlobalTransform::default(),
                Visibility::default(),
                Chamber,
                Name::new("Chamber"),
            ))
            .id();

        // Material for chamber faces (prefer procedural stone)
        let chamber_mat: Handle<StandardMaterial> = if let Some(p) = proc_tex.as_ref() {
            materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(p.stone_albedo.clone()),
                perceptual_roughness: 0.95,
                metallic: 0.02,
                cull_mode: None,
                double_sided: true,
                uv_transform: Affine2::from_scale(Vec2::splat(6.0)),
                ..Default::default()
            })
        } else {
            materials.add(StandardMaterial {
                base_color: chamber_color,
                perceptual_roughness: 0.95,
                metallic: 0.02,
                cull_mode: None,
                double_sided: true,
                ..Default::default()
            })
        };

        let half = chamber_size * 0.5;
        // Helper to spawn a plane as a child of the chamber
        let mut spawn_plane = |size: Vec2, local: Vec3, rot: Quat, name: &str| {
            let mesh = meshes.add(Plane3d::default().mesh().size(size.x, size.y));
            let child = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(chamber_mat.clone()),
                    Transform::from_translation(local).with_rotation(rot),
                    GlobalTransform::default(),
                    Name::new(name.to_string()),
                ))
                .id();
            commands.entity(child).insert(ChildOf(parent));
        };

        // Floor (XZ plane, normal +Y)
        spawn_plane(
            Vec2::new(chamber_size.x, chamber_size.z),
            Vec3::new(0.0, -half.y, 0.0),
            Quat::IDENTITY,
            "Chamber Floor",
        );
        // Ceiling (XZ plane, normal -Y)
        spawn_plane(
            Vec2::new(chamber_size.x, chamber_size.z),
            Vec3::new(0.0, half.y, 0.0),
            Quat::from_rotation_x(std::f32::consts::PI),
            "Chamber Ceiling",
        );
        // +Z wall (XY plane, normal -Z)
        spawn_plane(
            Vec2::new(chamber_size.x, chamber_size.y),
            Vec3::new(0.0, 0.0, half.z),
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            "Chamber Wall +Z",
        );
        // -Z wall (XY plane, normal +Z)
        spawn_plane(
            Vec2::new(chamber_size.x, chamber_size.y),
            Vec3::new(0.0, 0.0, -half.z),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            "Chamber Wall -Z",
        );
        // +X wall (YZ plane). Rotate default XZ plane so normal points +X
        spawn_plane(
            Vec2::new(chamber_size.y, chamber_size.z),
            Vec3::new(half.x, 0.0, 0.0),
            Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
            "Chamber Wall +X",
        );
        // Intentionally omit -X wall to create an open entrance from the tunnel
    }

    // Spawn a submarine (parent) with child hull and child rudder
    {
        // Place near the -X end of the tunnel, centered in YZ
        let tb = TunnelBounds { size: tunnel_size };
        let start = tunnel_pos + Vec3::new(-tb.size.x * 0.5 + 6.0, 0.0, 0.0);

        let sub_root = commands
            .spawn((
                Transform::from_translation(start),
                GlobalTransform::default(),
                Visibility::default(),
                Submarine,
                Velocity::default(),
                AngularVelocity::default(),
                SubPhysics(small_skiff_spec()),
                crate::hud_instruments::HudInstrumentState::default(),
                // Initialize persistent physics state; fill is set in simulate on first tick
                super::submarine::SubStateComp(levels::SubState {
                    position: levels::Vec3f::new(start.x, start.y, start.z),
                    velocity: levels::Vec3f::ZERO,
                    orientation: Quat::IDENTITY,
                    ang_mom: levels::Vec3f::ZERO,
                    ballast_fill: Vec::new(),
                }),
                Name::new("SubmarineRoot"),
            ))
            .id();

        // Hull (prolate spheroid) as child
        let sub_radius = 0.6;
        let sub_scale = Vec3::new(2.2, 0.8, 0.8); // prolate along +X
        let hull_mesh = meshes.add(Mesh::from(Sphere::new(sub_radius)));
        let hull_material = materials.add(StandardMaterial {
            base_color: Color::from(Srgba::new(0.75, 0.8, 0.85, 1.0)),
            perceptual_roughness: 0.4,
            metallic: 0.1,
            ..Default::default()
        });
        let hull = commands
            .spawn((
                Mesh3d(hull_mesh),
                MeshMaterial3d(hull_material),
                Transform::from_scale(sub_scale),
                GlobalTransform::default(),
                Name::new("SubmarineHull"),
            ))
            .id();
        commands.entity(hull).insert(ChildOf(sub_root));

        // Rudder as child (triangular prism thickness)
        let rudder_mesh = make_rudder_prism_mesh(1.0, 1.2, 0.12);
        let rudder_mesh = meshes.add(rudder_mesh);
        let rudder_material = materials.add(StandardMaterial {
            base_color: Color::from(Srgba::new(0.9, 0.1, 0.1, 1.0)),
            cull_mode: Some(Face::Back),
            ..Default::default()
        });
        let rudder_local = Transform::from_translation(Vec3::new(-1.6, 0.0, 0.0));
        let rudder = commands
            .spawn((
                Mesh3d(rudder_mesh),
                MeshMaterial3d(rudder_material),
                rudder_local,
                GlobalTransform::default(),
                Rudder,
                Name::new("Rudder"),
            ))
            .id();
        commands.entity(rudder).insert(ChildOf(sub_root));

        // Forward floodlight as a child (spotlight)
        let light_pos = Vec3::new(0.01, 0.3, 0.0);
        let light_transform =
            Transform::from_translation(light_pos).looking_at(light_pos + Vec3::X, Vec3::Y);
        commands
            .spawn((
                SpotLight {
                    color: Color::srgb(1.0, 1.0, 1.0),
                    intensity: 1_200_000_000.0, // brighter for longer throw
                    range: 600.0,
                    inner_angle: 0.04,
                    outer_angle: 0.08,
                    shadows_enabled: true,
                    ..Default::default()
                },
                light_transform,
                Name::new("Sub Floodlight"),
            ))
            .insert(ChildOf(sub_root));

        let tail_point_light_pos = Vec3::new(-1.1, 0.27, 0.0);
        let tail_root = commands
            .spawn((
                Transform::from_translation(tail_point_light_pos),
                Name::from("Sub Tail-light Root"),
                ChildOf(sub_root),
            ))
            .id();
        // Replace tail-light with blinking LightBulb
        let _tail_bulb = commands.spawn((
            LightBulb {
                color: Color::srgb(1.0, 0.1, 0.1),
                strength: 0.0,
            },
            BlinkingLight {
                period: 0.9,
                on_fraction: 0.4,
                on_intensity: 2.8,
                off_intensity: 0.0,
            },
            super::light_bulb::LightShadowOverride(false),
            Transform::IDENTITY,
            GlobalTransform::default(),
            Name::from("Sub Tail LightBulb"),
            ChildOf(tail_root),
        ));

        // Unified game camera (single camera). Initialize near the bow; mode = FirstPerson
        let fp_world = start + Vec3::new(1.0, 0.0, 0.0);
        let fp_t = Transform::from_translation(fp_world).looking_at(fp_world + Vec3::X, Vec3::Y);
        commands.spawn((
            Camera3d {
                depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::TEXTURE_BINDING)
                    .into(),
                ..Default::default()
            },
            Camera {
                hdr: true,
                is_active: true,
                ..Default::default()
            },
            bevy::core_pipeline::bloom::Bloom {
                intensity: 0.02,
                low_frequency_boost: 0.7,
                low_frequency_boost_curvature: 0.95,
                high_pass_frequency: 1.0,
                prefilter: BloomPrefilter {
                    threshold: 0.6,
                    threshold_softness: 0.2,
                },
                composite_mode: bevy::core_pipeline::bloom::BloomCompositeMode::Additive,
                max_mip_dimension: 512,
                scale: Vec2::ONE,
            },
            bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
            bevy::pbr::DistanceFog {
                color: Color::srgb(0.04, 0.11, 0.12),
                falloff: bevy::pbr::FogFalloff::Exponential { density: 0.10 },
                ..Default::default()
            },
            Msaa::Off,
            fp_t,
            GlobalTransform::default(),
            GameCamera,
            CamMode::FirstPerson,
            FollowCam {
                distance: 8.0,
                height: 2.0,
                stiffness: 8.0,
            },
            FollowCamState {
                last_dir: Vec3::NEG_X,
            },
            FreeFlyState {
                yaw: 0.0,
                pitch: 0.0,
                speed: 8.0,
            },
            Name::new("Game Camera"),
        ));

        let _ = tunnel_entity; // ensure it exists (unused var otherwise)
    }
}
