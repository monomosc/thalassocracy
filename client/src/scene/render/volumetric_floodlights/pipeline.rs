use bevy::asset::AssetServer;
use bevy::pbr::{
    DirectionalLightShadowMap, DistanceFog, FogFalloff, FogMeta, GpuFog, LightMeta,
    ViewFogUniformOffset,
};
use bevy::prelude::*;
use bevy::render::render_resource::{BindGroupLayoutDescriptor, ShaderType};
use bevy::render::texture::TextureCache;
use bevy::render::{
    mesh::{Mesh, MeshVertexBufferLayoutRef, RenderMesh},
    render_asset::RenderAssets,
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource,
        BindingType, BlendComponent, BlendFactor, BlendOperation, BlendState, Buffer,
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState,
        Extent3d, Face, FilterMode, FragmentState, MultisampleState, PipelineCache, PrimitiveState,
        RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, Shader,
        ShaderStages, SpecializedRenderPipeline, SpecializedRenderPipelines, StencilState, Texture,
        TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
        TextureViewDescriptor, TextureViewDimension, VertexState,
    },
    renderer::RenderDevice,
    view::{ExtractedView, Msaa, ViewDepthTexture, ViewTarget},
};
use bytemuck::{Pod, Zeroable};

use super::{
    ExtractedVolumetricDebugSettings, ExtractedVolumetricSettings, RenderVolumetricLightingMode,
    VolumetricLightingMode, CONE_VOLUME_SHADER_PATH,
};

#[derive(Resource)]
pub(super) struct ConeVolumePipeline {
    pub(super) shader: Handle<Shader>,
    resources: Option<ConePipelineResources>,
}

impl FromWorld for ConeVolumePipeline {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self {
            shader: asset_server.load(CONE_VOLUME_SHADER_PATH),
            resources: None,
        }
    }
}

struct ConePipelineResources {
    global_layout: BindGroupLayout,
    view_layout: BindGroupLayout,
    cone_layout: BindGroupLayout,
    fog_layout: BindGroupLayout,
    fallback_shadow_texture: Texture,
    fallback_shadow_sampler: Sampler,
}

impl ConeVolumePipeline {
    pub(super) fn ensure_initialized(&mut self, device: &RenderDevice) {
        if self.resources.is_some() {
            return;
        }

        let shadow_texture = device.create_texture(&TextureDescriptor {
            label: Some("cone_volume_fallback_shadow"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let shadow_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("cone_volume_shadow_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            compare: Some(CompareFunction::LessEqual),
            ..Default::default()
        });

        let global_layout = device.create_bind_group_layout(
            Some("cone_volume_global_bgl"),
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Depth,
                        view_dimension: TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        );
        let view_layout = device.create_bind_group_layout(
            Some("cone_volume_view_bgl"),
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Depth,
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        );

        let cone_layout = device.create_bind_group_layout(
            Some("cone_volume_cone_bgl"),
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        );
        let fog_layout = device.create_bind_group_layout(
            Some("gpu_fog_layout"),
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(GpuFog::min_size()),
                },
                count: None,
            }],
        );

        self.resources = Some(ConePipelineResources {
            global_layout,
            view_layout,
            cone_layout,
            fog_layout,
            fallback_shadow_texture: shadow_texture,
            fallback_shadow_sampler: shadow_sampler,
        });
    }

    fn resources(&self) -> &ConePipelineResources {
        self.resources
            .as_ref()
            .expect("ConeVolumePipeline::ensure_initialized must be called before use")
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub(super) struct ConeVolumePipelineKey {
    format: TextureFormat,
    sample_count: u32,
    vertex_layout: MeshVertexBufferLayoutRef,
}

impl SpecializedRenderPipeline for ConeVolumePipeline {
    type Key = ConeVolumePipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let vertex_layout = key
            .vertex_layout
            .0
            .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])
            .expect("Cone mesh missing POSITION attribute");

        let resources = self.resources();

        RenderPipelineDescriptor {
            label: Some("cone_volume_raymarch".into()),
            layout: vec![
                resources.global_layout.clone(),
                resources.view_layout.clone(),
                resources.cone_layout.clone(),
                resources.fog_layout.clone(),
            ],
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: "vertex".into(),
                buffers: vec![vertex_layout],
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                cull_mode: Some(Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: CompareFunction::GreaterEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: key.sample_count,
                ..Default::default()
            },
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(super) struct RenderConeLight {
    pub light_entity: Entity,
    pub apex: Vec3,
    pub direction: Vec3,
    pub range: f32,
    pub intensity: f32,
    pub color: LinearRgba,
    pub cos_inner: f32,
    pub cos_outer: f32,
    pub mesh: Handle<Mesh>,
    pub model: Mat4,
}

#[derive(Resource, Default, Clone)]
pub(super) struct ExtractedConeLights {
    pub cones: Vec<RenderConeLight>,
}

#[derive(Component)]
pub(super) struct ViewConeRenderData {
    pub(super) pipeline_id: CachedRenderPipelineId,
    pub(super) global: BindGroup,
    pub(super) view: BindGroup,
    pub(super) _view_uniform: Buffer,
    pub(super) draws: Vec<ConeDraw>,
    pub(super) fog: Option<BindGroup>,
}

pub(super) struct ConeDraw {
    pub bind_group: BindGroup,
    pub _uniform_buffer: Buffer,
    pub mesh: Handle<Mesh>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ConeVolumeViewUniform {
    inv_view_proj: Mat4,
    view_proj: Mat4,
    camera_position: Vec4,
    screen_size: Vec4,
    params: Vec4,
    tuning: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ConeVolumePerConeUniform {
    model: Mat4,
    apex: Vec4,
    direction_range: Vec4,
    color_intensity: Vec4,
    angles: Vec4,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_view_cone_lights(
    mut commands: Commands,
    views: Query<(
        Entity,
        &ExtractedView,
        Option<&ViewDepthTexture>,
        Option<&ViewFogUniformOffset>,
        Option<&Msaa>,
        Option<&ViewFogUniformOffset>,
    )>,
    fog_meta: Res<FogMeta>,
    cones: Res<ExtractedConeLights>,
    mode: Res<RenderVolumetricLightingMode>,
    settings: Res<ExtractedVolumetricSettings>,
    debug: Res<ExtractedVolumetricDebugSettings>,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<ConeVolumePipeline>>,
    mut pipeline: ResMut<ConeVolumePipeline>,
    render_device: Res<RenderDevice>,
    texture_cache: Res<TextureCache>,
    mesh_assets: Res<RenderAssets<RenderMesh>>,
) {
    let raymarch = matches!(mode.0, VolumetricLightingMode::RaymarchCones);
    for (entity, view, depth_texture, fog_uniform, msaa, fog_offset) in &views {
        let mut entity_commands = commands.entity(entity);
        if !raymarch || cones.cones.is_empty() {
            entity_commands.remove::<ViewConeRenderData>();
            continue;
        }

        let Some(depth_texture) = depth_texture else {
            entity_commands.remove::<ViewConeRenderData>();
            continue;
        };

        let Some(first_cone) = cones.cones.first() else {
            entity_commands.remove::<ViewConeRenderData>();
            continue;
        };
        let Some(render_mesh) = mesh_assets.get(&first_cone.mesh) else {
            entity_commands.remove::<ViewConeRenderData>();
            continue;
        };

        pipeline.ensure_initialized(&render_device);
        let resources = pipeline.resources();

        let format = if view.hdr {
            ViewTarget::TEXTURE_FORMAT_HDR
        } else {
            TextureFormat::bevy_default()
        };
        let sample_count = msaa.map(|m| m.samples()).unwrap_or(1);
        let key = ConeVolumePipelineKey {
            format,
            sample_count,
            vertex_layout: render_mesh.layout.clone(),
        };
        let pipeline_id = pipelines.specialize(&pipeline_cache, &pipeline, key);

        let world_from_view = view.world_from_view.compute_matrix();
        let view_from_world = world_from_view.inverse();
        let clip_from_world = view
            .clip_from_world
            .unwrap_or(view.clip_from_view * view_from_world);
        let inv_view_proj = clip_from_world.inverse();
        let camera_position = view.world_from_view.translation();
        let viewport = view.viewport;
        let screen_width = viewport.z.max(1) as f32;
        let screen_height = viewport.w.max(1) as f32;
        debug_assert!(screen_width.is_finite() && screen_width > 0.0);
        debug_assert!(screen_height.is_finite() && screen_height > 0.0);
        let inv_screen_width = if screen_width > 0.0 {
            1.0 / screen_width
        } else {
            0.0
        };
        let inv_screen_height = if screen_height > 0.0 {
            1.0 / screen_height
        } else {
            0.0
        };
        let view_uniform = ConeVolumeViewUniform {
            inv_view_proj,
            view_proj: clip_from_world,
            camera_position: Vec4::new(
                camera_position.x,
                camera_position.y,
                camera_position.z,
                1.0,
            ),
            screen_size: Vec4::new(
                screen_width,
                screen_height,
                inv_screen_width,
                inv_screen_height,
            ),
            params: Vec4::new(settings.scatter_strength, debug.debug_mode as f32, 0.0, 0.0),
            tuning: Vec4::new(
                settings.distance_falloff,
                settings.angular_softness,
                settings.extinction,
                0.0,
            ),
        };
        let view_uniform_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("cone_volume_view_uniform"),
            contents: bytemuck::bytes_of(&view_uniform),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let shadow_view = resources
            .fallback_shadow_texture
            .create_view(&TextureViewDescriptor {
                dimension: Some(TextureViewDimension::D2Array),
                ..Default::default()
            });

        let global_bind_group = render_device.create_bind_group(
            Some("cone_volume_global_bg"),
            &resources.global_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&shadow_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&resources.fallback_shadow_sampler),
                },
            ],
        );

        let view_bind_group = render_device.create_bind_group(
            Some("cone_volume_view_bg"),
            &resources.view_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: view_uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(depth_texture.view()),
                },
            ],
        );
        let fog_bind_group = fog_offset.map(|_| {
            render_device.create_bind_group(
                Some("cone_fog_bind_group"),
                &resources.fog_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: fog_meta.gpu_fogs.binding().unwrap(),
                }],
            )
        });

        let mut draws = Vec::new();
        for cone in &cones.cones {
            let Some(_render_mesh) = mesh_assets.get(&cone.mesh) else {
                continue;
            };

            debug_assert!(
                cone.range.is_finite() && cone.range > 0.0,
                "Cone range invalid: {:?}",
                cone.range
            );
            debug_assert!(
                cone.intensity.is_finite() && cone.intensity >= 0.0,
                "Cone intensity invalid: {:?}",
                cone.intensity
            );
            debug_assert!(
                (cone.direction.length_squared() - 1.0).abs() < 1e-3,
                "Cone direction not normalized: {:?}",
                cone.direction
            );
            debug_assert!(
                cone.cos_inner >= cone.cos_outer - 1e-3,
                "Cone cos_inner < cos_outer: {:?} < {:?}",
                cone.cos_inner,
                cone.cos_outer
            );

            let cone_uniform = ConeVolumePerConeUniform {
                model: cone.model,
                apex: Vec4::new(cone.apex.x, cone.apex.y, cone.apex.z, 1.0),
                direction_range: Vec4::new(
                    cone.direction.x,
                    cone.direction.y,
                    cone.direction.z,
                    cone.range,
                ),
                color_intensity: Vec4::new(
                    cone.color.red,
                    cone.color.green,
                    cone.color.blue,
                    cone.intensity,
                ),
                angles: Vec4::new(cone.cos_inner, cone.cos_outer, 0.0, 0.0),
            };

            let uniform_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("cone_volume_cone_uniform"),
                contents: bytemuck::bytes_of(&cone_uniform),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

            let cone_bind_group = render_device.create_bind_group(
                Some("cone_volume_cone_bg"),
                &resources.cone_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            );

            draws.push(ConeDraw {
                bind_group: cone_bind_group,
                _uniform_buffer: uniform_buffer,
                mesh: cone.mesh.clone(),
            });
        }

        if draws.is_empty() {
            entity_commands.remove::<ViewConeRenderData>();
            continue;
        }

        entity_commands.insert(ViewConeRenderData {
            pipeline_id,
            global: global_bind_group,
            view: view_bind_group,
            _view_uniform: view_uniform_buffer,
            draws,
            fog: fog_bind_group,
        });
    }
}
