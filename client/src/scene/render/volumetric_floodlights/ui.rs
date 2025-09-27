use bevy::prelude::*;

use super::{VolumetricLightingMode, VolumetricLightingState};

#[derive(Component)]
pub(super) struct ModeLabel;

pub(super) fn toggle_volumetric_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<VolumetricLightingState>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        state.mode = match state.mode {
            VolumetricLightingMode::LegacyCones => VolumetricLightingMode::RaymarchCones,
            VolumetricLightingMode::RaymarchCones => VolumetricLightingMode::LegacyCones,
        };
        println!(
            "Volumetric mode: {}",
            match state.mode {
                VolumetricLightingMode::LegacyCones => "Legacy",
                VolumetricLightingMode::RaymarchCones => "Raymarch",
            }
        );
    }
}

pub(super) fn spawn_mode_label(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(14.0),
            ..Default::default()
        },
        Text::new("Volumetrics: raymarch [V]"),
        TextFont {
            font_size: 14.0,
            ..Default::default()
        },
        TextColor(Color::WHITE),
        ModeLabel,
        Name::new("VolumetricMode"),
    ));
}

pub(super) fn update_mode_label(
    state: Res<VolumetricLightingState>,
    mut q: Query<&mut Text, With<ModeLabel>>,
) {
    if !state.is_changed() {
        return;
    }
    let text = match state.mode {
        VolumetricLightingMode::LegacyCones => "Volumetrics: legacy [V]",
        VolumetricLightingMode::RaymarchCones => "Volumetrics: raymarch [V]",
    };
    for mut t in &mut q {
        *t = Text::new(text);
    }
}
