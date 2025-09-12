use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::pbr::Material;
use bevy::prelude::Shader;
use bevy::asset::Handle;

// Custom unlit material for volumetric spot light cones.
// Features:
// - Distance falloff along cone (via mesh V coordinate: apex v=1 → base v=0)
// - Configurable per-channel absorption (e.g., absorb more red)
// - View-angle brightening (simple Fresnel-like term)
// - 3D noise modulation (world-space) to break up smoothness
// - Edge smoothing near apex/base
#[derive(Asset, AsBindGroup, Debug, Clone, Reflect)]
pub struct VolumetricConeMaterial {
    // Base color/intensity (alpha is overall intensity multiplier)
    #[uniform(0)]
    pub color: LinearRgba,

    // params0: (absorb_r, absorb_g, absorb_b, view_fresnel_strength)
    #[uniform(1)]
    pub params0: Vec4,

    // params1: (noise_strength, noise_scale, falloff_pow, edge_soften)
    #[uniform(2)]
    pub params1: Vec4,

    // Flicker amplitudes for low/mid/high frequency sine terms (xyz), w unused
    #[uniform(3)]
    pub flicker_amps: Vec4,

    // Flicker frequencies (Hz) for low/mid/high (xyz), w unused
    #[uniform(4)]
    pub flicker_freqs: Vec4,

    // Flicker phase offsets (radians) for low/mid/high (xyz), w unused
    #[uniform(5)]
    pub flicker_phases: Vec4,

    // HDR/emissive boost multiplier (x component). yzw unused/reserved.
    #[uniform(6)]
    pub hdr_params: Vec4,
    pub fog_enabled: bool, // This controls the FOG define

    // Blending mode
    pub alpha_mode: AlphaMode,
}

impl Default for VolumetricConeMaterial {
    fn default() -> Self {
        Self {
            color: LinearRgba::new(0.18, 0.6, 0.9, 0.12),
            // absorb more red by default; smaller for B to keep blue for longer
            params0: Vec4::new(2.2, 1.0, 0.6, 0.6),
            // mild noise, higher scale (finer), soft falloff, gentle edge soften
            params1: Vec4::new(0.25, 8.0, 1.2, 0.08),
            // gentle flicker amplitudes
            flicker_amps: Vec4::new(0.05, 0.03, 0.1, 0.0),
            // low/mid/high frequencies in Hz
            flicker_freqs: Vec4::new(0.27, 1.3, 7.7, 0.0),
            // default phases 0; per-spot randomized at spawn
            flicker_phases: Vec4::ZERO,
            // emissive boost default 1.0
            hdr_params: Vec4::new(6.0, 0.0, 0.0, 0.0),
            alpha_mode: AlphaMode::Add,
            fog_enabled: true,
        }
    }
}

impl Material for VolumetricConeMaterial {
    fn fragment_shader() -> ShaderRef { VOLUMETRIC_CONE_SHADER_HANDLE.into() }

    fn alpha_mode(&self) -> AlphaMode { self.alpha_mode }

    fn specialize(
            _pipeline: &bevy::pbr::MaterialPipeline<Self>,
            descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
            _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
            _key: bevy::pbr::MaterialPipelineKey<Self>,
        ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
            descriptor.primitive.cull_mode = None;
            
            
            // let view_key = MeshPipelineViewLayoutKey::from(key.mesh_key);
            // let view_layout = pipeline.mesh_pipeline.view_layouts.get_view_layout(view_key);
            // descriptor.layout.insert(1, view_layout.clone());

            // descriptor.fragment.as_mut().unwrap().shader_defs.push(bevy::render::render_resource::ShaderDefVal::Bool("DISTANCE_FOG".into(), true));

            Ok(())
    }
}

// Public shader handle so the app can register it via load_internal_asset!
#[allow(deprecated)]
pub const VOLUMETRIC_CONE_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0x8a2e_5b11_c3d4_49c7_9b5e_3dd6_f4a1_55b1);
