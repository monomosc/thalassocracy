use bevy::prelude::*;
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use bevy_inspector_egui::InspectorOptions;

#[derive(Resource, Debug, Clone, Reflect, InspectorOptions)]
#[reflect(Resource)]
pub struct RenderSettings {
    pub volumetric_cones: bool,
    pub water_post: bool,
    pub water_post_strength: f32,
    pub water_post_debug: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            volumetric_cones: true,
            water_post: true,
            water_post_strength: 1.0,
            water_post_debug: false,
        }
    }
}

pub struct RenderSettingsPlugin;

impl Plugin for RenderSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderSettings>()
            .register_type::<RenderSettings>()
            .add_plugins(ResourceInspectorPlugin::<RenderSettings>::default());
    }
}
