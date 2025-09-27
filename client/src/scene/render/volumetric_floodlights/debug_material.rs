use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, CompareFunction, DepthStencilState, ShaderRef};

#[derive(Asset, AsBindGroup, Debug, Clone, Reflect)]
pub struct VolumetricConeDebugMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    // params.x = intensity, params.y = edge_soft, params.z = along_pow
    #[uniform(1)]
    pub params: Vec4,
    pub alpha_mode: AlphaMode,
}

impl Default for VolumetricConeDebugMaterial {
    fn default() -> Self {
        Self {
            color: LinearRgba::new(0.10, 0.85, 1.0, 0.35),
            params: Vec4::new(12.0, 0.10, 1.2, 0.0),
            alpha_mode: AlphaMode::Add,
        }
    }
}

impl Material for VolumetricConeDebugMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(VOLUMETRIC_CONE_DEBUG_SHADER_PATH.into())
    }
    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Render double-sided so cones are visible from inside.
        descriptor.primitive.cull_mode = None;
        // Enable depth testing against the main depth buffer (read-only), so cones are clipped by scene geometry.
        descriptor.depth_stencil = Some(DepthStencilState {
            format: CORE_3D_DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: CompareFunction::LessEqual,
            stencil: Default::default(),
            bias: Default::default(),
        });
        Ok(())
    }
}

pub const VOLUMETRIC_CONE_DEBUG_SHADER_PATH: &str =
    "shaders/volumetric_floodlights/cone_debug.wgsl";
