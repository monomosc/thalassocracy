use bevy::prelude::*;
use bevy::math::primitives::Sphere;
use bevy::pbr::{MeshMaterial3d, NotShadowCaster, StandardMaterial};

#[derive(Resource, Default)]
struct LightBulbAssets {
    sphere_mesh: Handle<Mesh>,
}

#[derive(Component, Reflect, Clone, Copy)]
#[reflect(Component)]
#[require(Transform, GlobalTransform, PointLight)]
pub struct LightBulb {
    pub color: Color,
    /// Strength scales both emissive brightness and spotlight intensity.
    pub strength: f32,
}

impl Default for LightBulb {
    fn default() -> Self { Self { color: Color::srgb(1.0, 0.95, 0.85), strength: 1.0 } }
}

#[derive(Component)]
struct LightBulbVisual;

pub struct LightBulbPlugin;

impl Plugin for LightBulbPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightBulbAssets>()
            .register_type::<LightBulb>()
            .register_type::<BlinkingLight>()
            .register_type::<LightShadowOverride>()
            .add_systems(Startup, setup_assets)
            .add_systems(Update, (ensure_bulb_visual_and_setup, tick_blinking_lights, update_bulb_properties));
    }
}

fn setup_assets(mut meshes: ResMut<Assets<Mesh>>, mut assets: ResMut<LightBulbAssets>) {
    assets.sphere_mesh = meshes.add(Mesh::from(Sphere::new(0.06)));
}

/// Optional per-entity override for the underlying PointLight's `shadows_enabled`.
#[derive(Component, Reflect, Clone, Copy)]
#[reflect(Component)]
pub struct LightShadowOverride(pub bool);

#[derive(Component, Reflect, Clone, Copy)]
#[reflect(Component)]
pub struct BlinkingLight {
    /// total period in seconds (e.g. 1.0 = 1 Hz)
    pub period: f32,
    /// fraction of the period that the light is ON (0..1), e.g. 0.2 = 20% duty cycle
    pub on_fraction: f32,
    /// LightBulb.strength when ON
    pub on_intensity: f32,
    /// LightBulb.strength when OFF (usually 0.0)
    pub off_intensity: f32,
}

impl Default for BlinkingLight {
    fn default() -> Self { Self { period: 1.0, on_fraction: 0.2, on_intensity: 1.0, off_intensity: 0.0 } }
}

fn tick_blinking_lights(
    time: Res<Time>,
    mut q: Query<(&BlinkingLight, &mut LightBulb)>,
) {
    let t = time.elapsed_secs();
    for (blink, mut bulb) in &mut q {
        let period = blink.period.max(1e-3);
        let phase = (t % period) / period; // 0..1
        let on_frac = blink.on_fraction.clamp(0.0, 1.0);
        let target = if phase < on_frac { blink.on_intensity } else { blink.off_intensity };
        if (bulb.strength - target).abs() > f32::EPSILON {
            bulb.strength = target;
        }
    }
}

fn ensure_bulb_visual_and_setup(
    mut commands: Commands,
    assets: Res<LightBulbAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q_new: Query<(Entity, &LightBulb, Option<&Children>, &mut PointLight), Added<LightBulb>>,
    vis_q: Query<(), With<LightBulbVisual>>,
) {
    for (e, bulb, children_opt, mut point) in &mut q_new {
        // Configure point light defaults and tie to bulb values
        point.color = bulb.color;
        point.range = 14.0;
        point.shadows_enabled = false;
        // Intensity coupling
        point.intensity = bulb.strength.max(0.0) * 50_000.0;

        // Ensure a small emissive sphere visual exists as a child
        let has_visual = children_opt
            .map(|children| children.iter().any(|c| vis_q.contains(c)))
            .unwrap_or(false);
        if !has_visual {
            let mat = materials.add(StandardMaterial {
                base_color: bulb.color,
                emissive: LinearRgba::new(
                    bulb.color.to_linear().red,
                    bulb.color.to_linear().green,
                    bulb.color.to_linear().blue,
                    0.0,
                ) * bulb.strength.max(0.0) * 20.0,
                perceptual_roughness: 0.3,
                metallic: 0.2,
                ..Default::default()
            });
            let child = commands
                .spawn((
                    Mesh3d(assets.sphere_mesh.clone()),
                    MeshMaterial3d(mat),
                    Transform::IDENTITY,
                    GlobalTransform::default(),
                    LightBulbVisual,
                    NotShadowCaster,
                    Name::new("LightBulb Visual"),
                ))
                .id();
            commands.entity(child).insert(ChildOf(e));
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_bulb_properties(
    bulb_q: Query<(Entity, &LightBulb, Option<&Children>, Option<&LightShadowOverride>), Changed<LightBulb>>,
    mut point_q: Query<&mut PointLight>,
    mut mat_q: Query<&mut MeshMaterial3d<StandardMaterial>, With<LightBulbVisual>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, bulb, children, shadow_override) in &bulb_q {
        // Update point light on the same entity
        if let Ok(mut pl) = point_q.get_mut(e) {
            pl.color = bulb.color;
            pl.intensity = bulb.strength.max(0.0) * 50_000.0;
            if let Some(LightShadowOverride(enabled)) = shadow_override { pl.shadows_enabled = *enabled; }
        }
        // Update emissive of visual child
        if let Some(children) = children {
            for c in children.iter() {
                if let Ok(mh) = mat_q.get_mut(c) {
                    if let Some(m) = materials.get_mut(&mh.0) {
                        m.base_color = bulb.color;
                        let lin = bulb.color.to_linear();
                        m.emissive = LinearRgba::new(lin.red, lin.green, lin.blue, 0.0)
                            * bulb.strength.max(0.0) * 20.0;
                    }
                }
            }
        }
    }
}
