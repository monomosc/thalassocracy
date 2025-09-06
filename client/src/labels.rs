use bevy::prelude::*;
use crate::debug_vis::{LabelNode};

#[derive(Resource, Clone)]
pub struct LabelFont(pub Handle<Font>);

#[derive(Component, Copy, Clone)]
pub struct TracksEntity(pub Entity);

pub struct LabelPlugin;

impl Plugin for LabelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_font)
            // Spawn labels once when both font and targets exist
            .add_systems(Update, attach_initial_labels)
            .add_systems(Update, update_label_positions);
    }
}

fn load_font(mut commands: Commands, assets: Res<AssetServer>) {
    // Expect a font in assets/fonts; if not found, the handle is still valid but text won't render.
    let font: Handle<Font> = assets.load("fonts/FiraSans-Bold.ttf");
    commands.insert_resource(LabelFont(font));
}

// Attach labels for the initial greybox entities
fn attach_initial_labels(
    mut commands: Commands,
    font: Option<Res<LabelFont>>,
    q_station: Query<Entity, With<crate::scene::StationRoom>>,
    q_tunnel: Query<Entity, With<crate::scene::Tunnel>>,
    q_chamber: Query<Entity, With<crate::scene::Chamber>>,
    q_dock: Query<Entity, With<crate::scene::DockPad>>,
    existing: Query<&TracksEntity>,
) {
    let Some(font) = font else { return; };
    // Helper to spawn a UI text label that tracks an entity
    let mut spawn_label = |target: Entity, text: &str, color: Color| {
        commands.spawn((
            // Absolute positioning; updated every frame
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..Default::default()
            },
            Text::new(text.to_string()),
            TextFont { font: font.0.clone(), font_size: 16.0, ..Default::default() },
            TextColor(color),
            TracksEntity(target),
            LabelNode,
            Name::new(format!("Label: {}", text)),
        ));
    };

    // Spawn exactly one label per category if it doesn't exist yet
    if let Some(e) = q_station.iter().next() {
        if !existing.iter().any(|t| t.0 == e) { spawn_label(e, "Station", Color::WHITE); }
    }
    if let Some(e) = q_tunnel.iter().next() {
        if !existing.iter().any(|t| t.0 == e) { spawn_label(e, "Tunnel", Color::srgb(1.0, 1.0, 0.0)); }
    }
    if let Some(e) = q_chamber.iter().next() {
        if !existing.iter().any(|t| t.0 == e) { spawn_label(e, "Chamber", Color::srgb(1.0, 0.27, 0.0)); }
    }
    if let Some(e) = q_dock.iter().next() {
        if !existing.iter().any(|t| t.0 == e) { spawn_label(e, "Dock", Color::srgb(0.0, 1.0, 1.0)); }
    }
}

fn update_label_positions(
    mut q_text: Query<(&mut Node, &TracksEntity), With<LabelNode>>,
    q_target: Query<&GlobalTransform>,
    q_camera: Query<(&Camera, &GlobalTransform)>,
) {
    let Some((camera, cam_transform)) = q_camera.iter().next() else { return; };
    let viewport = match camera.logical_viewport_size() {
        Some(v) => v,
        None => return,
    };

    for (mut node, tracks) in q_text.iter_mut() {
        let target = tracks.0;
        if let Ok(target_xform) = q_target.get(target) {
            let world_pos = target_xform.translation() + Vec3::Y * 2.0;
            if let Some(ndc) = camera.world_to_ndc(cam_transform, world_pos) {
                if ndc.z >= 0.0 {
                    // NDC (-1..1) -> screen space
                    let screen_pos = (ndc.truncate() + Vec2::ONE) / 2.0 * viewport;
                    node.left = Val::Px(screen_pos.x);
                    node.top = Val::Px(viewport.y - screen_pos.y); // UI origin is top-left
                }
            }
        }
    }
}

// no has_label_for helper necessary; we use a Query in-system
