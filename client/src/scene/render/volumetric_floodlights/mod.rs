use crate::render_settings::{RenderSettings, VolumetricConeShaderDebugSettings};
use bevy::asset::load_internal_asset;
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::ecs::query::QueryItem;
use bevy::pbr::SpotLight;
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use bevy::render::{
    camera::ExtractedCamera,
    mesh::{
        allocator::MeshAllocator, Mesh, Mesh3d, MeshVertexBufferLayoutRef, RenderMesh,
        RenderMeshBufferInfo,
    },
    render_asset::RenderAssets,
    render_graph::{
        NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
    },
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource,
        BindingType, BlendComponent, BlendFactor, BlendOperation, BlendState, Buffer,
        BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState,
        Extent3d, FilterMode, FragmentState, IndexFormat, LoadOp, MultisampleState, Operations,
        PipelineCache, PrimitiveState, RenderPassDepthStencilAttachment, RenderPassDescriptor,
        RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, Shader,
        ShaderStages, SpecializedRenderPipeline, SpecializedRenderPipelines, StencilState, StoreOp,
        Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
        TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexState,
    },
    renderer::{RenderContext, RenderDevice},
    view::{ExtractedView, Msaa, ViewDepthTexture, ViewTarget, ViewVisibility},
    Extract, ExtractSchedule, Render, RenderApp, RenderSet,
};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
pub mod debug_material;
pub use debug_material::{VolumetricConeDebugMaterial, VOLUMETRIC_CONE_DEBUG_SHADER_HANDLE};

pub mod volumetric_cone_material;

mod legacy;
use self::legacy::VolumetricCone;

#[allow(deprecated)]
pub const CONE_VOLUME_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x6de2_9d11_cdbd_4a46_ba4c_6f7a_ee9f_c3f2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricLightingMode {
    LegacyCones,
    RaymarchCones,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct VolumetricLightingState {
    pub mode: VolumetricLightingMode,
}

impl Default for VolumetricLightingState {
    fn default() -> Self {
        Self {
            mode: VolumetricLightingMode::RaymarchCones,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct RenderVolumetricLightingMode(pub VolumetricLightingMode);

impl Default for RenderVolumetricLightingMode {
    fn default() -> Self {
        Self(VolumetricLightingMode::RaymarchCones)
    }
}

#[derive(Resource, Clone, Copy, Debug)]
struct ExtractedVolumetricSettings {
    scatter_strength: f32,
}

impl Default for ExtractedVolumetricSettings {
    fn default() -> Self {
        Self {
            scatter_strength: 0.02,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
struct ExtractedVolumetricDebugSettings {
    debug_mode: u32,
}

#[derive(Resource, Default)]
struct ConeVolumePipeline {
    resources: Option<ConePipelineResources>,
}

struct ConePipelineResources {
    global_layout: BindGroupLayout,
    view_layout: BindGroupLayout,
    cone_layout: BindGroupLayout,
    fallback_shadow_texture: Texture,
    fallback_shadow_sampler: Sampler,
}

impl ConeVolumePipeline {
    fn ensure_initialized(&mut self, device: &RenderDevice) {
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

        self.resources = Some(ConePipelineResources {
            global_layout,
            view_layout,
            cone_layout,
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
struct ConeVolumePipelineKey {
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
            ],
            vertex: VertexState {
                shader: CONE_VOLUME_SHADER_HANDLE,
                shader_defs: vec![],
                entry_point: "vertex".into(),
                buffers: vec![vertex_layout],
            },
            fragment: Some(FragmentState {
                shader: CONE_VOLUME_SHADER_HANDLE,
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
pub struct RenderConeLight {
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
pub struct ExtractedConeLights {
    pub cones: Vec<RenderConeLight>,
}

#[derive(Component)]
struct ViewConeRenderData {
    pipeline_id: CachedRenderPipelineId,
    global: BindGroup,
    view: BindGroup,
    _view_uniform: Buffer,
    draws: Vec<ConeDraw>,
}

struct ConeDraw {
    bind_group: BindGroup,
    _uniform_buffer: Buffer,
    mesh: Handle<Mesh>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ConeVolumeViewUniform {
    inv_view_proj: Mat4,
    view_proj: Mat4,
    camera_position: Vec4,
    screen_size: Vec4,
    params: Vec4,
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

fn extract_volumetric_mode(mut commands: Commands, mode: Extract<Res<VolumetricLightingState>>) {
    commands.insert_resource(RenderVolumetricLightingMode(mode.mode));
}

fn extract_volumetric_settings(mut commands: Commands, settings: Extract<Res<RenderSettings>>) {
    debug_assert!(
        settings.volumetric_cone_intensity.is_finite(),
        "volumetric_cone_intensity is not finite"
    );
    commands.insert_resource(ExtractedVolumetricSettings {
        scatter_strength: settings.volumetric_cone_intensity,
    });
}

fn extract_volumetric_debug_settings(
    mut commands: Commands,
    settings: Extract<Res<VolumetricConeShaderDebugSettings>>,
) {
    commands.insert_resource(ExtractedVolumetricDebugSettings {
        debug_mode: settings.debug_mode,
    });
}

fn extract_cone_lights(
    mut commands: Commands,
    state: Extract<Res<VolumetricLightingState>>,
    lights: Extract<
        Query<(
            Entity,
            &SpotLight,
            &GlobalTransform,
            Option<&Children>,
            Option<&ViewVisibility>,
        )>,
    >,
    cones_query: Extract<
        Query<(Entity, &GlobalTransform, &Mesh3d, Option<&ViewVisibility>), With<VolumetricCone>>,
    >,
) {
    let mut cones = Vec::new();
    if matches!(state.mode, VolumetricLightingMode::RaymarchCones) {
        let mut cone_data: HashMap<Entity, (Handle<Mesh>, Mat4, bool)> = HashMap::default();
        for (entity, transform, mesh, visibility) in cones_query.iter() {
            let visible = visibility.map_or(true, |v| v.get());
            cone_data.insert(
                entity,
                (mesh.0.clone(), transform.compute_matrix(), visible),
            );
        }

        for (entity, light, transform, children, visibility) in lights.iter() {
            if let Some(view_visibility) = visibility {
                if !view_visibility.get() {
                    continue;
                }
            }

            debug_assert!(
                light.range.is_finite(),
                "SpotLight {:?} has non-finite range",
                entity
            );
            debug_assert!(
                light.intensity.is_finite(),
                "SpotLight {:?} has non-finite intensity",
                entity
            );
            if light.range <= 0.1 {
                continue;
            }

            let mut mesh_and_model = None;
            if let Some(children) = children {
                for child in children.iter() {
                    if let Some((mesh, model, cone_visible)) = cone_data.get(&child) {
                        mesh_and_model = Some((mesh.clone(), *model, *cone_visible));
                        break;
                    }
                }
            }
            let Some((mesh, model, cone_visible)) = mesh_and_model else {
                continue;
            };
            if !cone_visible {
                continue;
            };

            let world_transform = transform.compute_transform();
            let direction = (world_transform.rotation * Vec3::NEG_Z).normalize_or_zero();
            debug_assert!(
                direction.length_squared() > 0.0,
                "SpotLight {:?} produced zero direction",
                entity
            );
            debug_assert!(
                (direction.length_squared() - 1.0).abs() < 1e-3,
                "SpotLight {:?} direction not normalized: {:?}",
                entity,
                direction
            );

            let cos_inner = light.inner_angle.cos();
            let cos_outer = light.outer_angle.cos();
            debug_assert!(
                cos_inner >= cos_outer - 1e-3,
                "SpotLight {:?} inner angle >= outer angle violated: cos_inner={:?}, cos_outer={:?}",
                entity,
                cos_inner,
                cos_outer
            );

            cones.push(RenderConeLight {
                light_entity: entity,
                apex: world_transform.translation,
                direction,
                range: light.range,
                intensity: light.intensity,
                color: light.color.into(),
                cos_inner,
                cos_outer,
                mesh,
                model,
            });
        }
    }

    commands.insert_resource(ExtractedConeLights { cones });
}

fn prepare_view_cone_lights(
    mut commands: Commands,
    views: Query<(
        Entity,
        &ExtractedView,
        Option<&ViewDepthTexture>,
        Option<&Msaa>,
    )>,
    cones: Res<ExtractedConeLights>,
    mode: Res<RenderVolumetricLightingMode>,
    settings: Res<ExtractedVolumetricSettings>,
    debug: Res<ExtractedVolumetricDebugSettings>,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<ConeVolumePipeline>>,
    mut pipeline: ResMut<ConeVolumePipeline>,
    render_device: Res<RenderDevice>,
    mesh_assets: Res<RenderAssets<RenderMesh>>,
) {
    let raymarch = matches!(mode.0, VolumetricLightingMode::RaymarchCones);
    for (entity, view, depth_texture, msaa) in &views {
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
        let pipeline_ref = &*pipeline;
        let resources = pipeline_ref.resources();

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
        let pipeline_id = pipelines.specialize(&pipeline_cache, pipeline_ref, key);

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
        });
    }
}

#[derive(RenderLabel, Debug, Clone, Hash, PartialEq, Eq)]
struct FloodlightPassLabel;

#[derive(Default)]
struct FloodlightViewNode;

impl ViewNode for FloodlightViewNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        Option<&'static ViewDepthTexture>,
        Option<&'static ViewConeRenderData>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (camera, target, depth_texture, render_data): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let mode = world.resource::<RenderVolumetricLightingMode>();
        if mode.0 != VolumetricLightingMode::RaymarchCones {
            return Ok(());
        }

        let Some(render_data) = render_data else {
            return Ok(());
        };
        if render_data.draws.is_empty() {
            return Ok(());
        }

        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(render_data.pipeline_id) else {
            return Ok(());
        };

        let Some(depth_texture) = depth_texture else {
            return Ok(());
        };
        let depth_view = depth_texture.view();

        let mesh_allocator = world.resource::<MeshAllocator>();
        let mesh_assets = world.resource::<RenderAssets<RenderMesh>>();

        let mut color_attachment = target.get_color_attachment();
        color_attachment.ops = Operations {
            load: LoadOp::Load,
            store: StoreOp::Store,
        };

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("cone_volume_pass"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let Some(viewport) = camera.viewport.as_ref() {
            render_pass.set_camera_viewport(viewport);
        }

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &render_data.global, &[]);
        render_pass.set_bind_group(1, &render_data.view, &[]);

        for draw in &render_data.draws {
            let Some(render_mesh) = mesh_assets.get(&draw.mesh) else {
                continue;
            };
            let Some(vertex_slice) = mesh_allocator.mesh_vertex_slice(&draw.mesh.id()) else {
                continue;
            };

            render_pass.set_bind_group(2, &draw.bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));

            match &render_mesh.buffer_info {
                RenderMeshBufferInfo::Indexed {
                    index_format,
                    count,
                } => {
                    let Some(index_slice) = mesh_allocator.mesh_index_slice(&draw.mesh.id()) else {
                        continue;
                    };
                    let index_stride = match index_format {
                        IndexFormat::Uint16 => 2u64,
                        IndexFormat::Uint32 => 4u64,
                    };
                    let offset = index_slice.range.start as u64 * index_stride;
                    render_pass.set_index_buffer(
                        index_slice.buffer.slice(..),
                        offset,
                        *index_format,
                    );
                    render_pass.draw_indexed(
                        index_slice.range.start..(index_slice.range.start + count),
                        vertex_slice.range.start as i32,
                        0..1,
                    );
                }
                RenderMeshBufferInfo::NonIndexed => {
                    render_pass.draw(vertex_slice.range.clone(), 0..1);
                }
            }
        }

        Ok(())
    }
}

pub struct VolumetricFloodlightsPlugin;

impl Plugin for VolumetricFloodlightsPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            CONE_VOLUME_SHADER_HANDLE,
            "cone_volume_pass.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            volumetric_cone_material::VOLUMETRIC_CONE_SHADER_HANDLE,
            "volumetric_cone_material.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            VOLUMETRIC_CONE_DEBUG_SHADER_HANDLE,
            "cone_debug.wgsl",
            Shader::from_wgsl
        );

        legacy::register(app);

        app.init_resource::<VolumetricLightingState>()
            .add_systems(Update, toggle_volumetric_mode)
            .add_systems(Startup, spawn_mode_label)
            .add_systems(Update, update_mode_label);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<RenderVolumetricLightingMode>()
                .init_resource::<ConeVolumePipeline>()
                .init_resource::<SpecializedRenderPipelines<ConeVolumePipeline>>()
                .init_resource::<ExtractedConeLights>()
                .init_resource::<ExtractedVolumetricSettings>()
                .init_resource::<ExtractedVolumetricDebugSettings>()
                .add_systems(ExtractSchedule, extract_volumetric_mode)
                .add_systems(
                    ExtractSchedule,
                    extract_volumetric_settings.after(extract_volumetric_mode),
                )
                .add_systems(
                    ExtractSchedule,
                    extract_volumetric_debug_settings.after(extract_volumetric_settings),
                )
                .add_systems(
                    ExtractSchedule,
                    extract_cone_lights.after(extract_volumetric_debug_settings),
                )
                .add_systems(Render, prepare_view_cone_lights.in_set(RenderSet::Queue))
                .add_render_graph_node::<ViewNodeRunner<FloodlightViewNode>>(
                    Core3d,
                    FloodlightPassLabel,
                )
                .add_render_graph_edges(
                    Core3d,
                    (
                        Node3d::MainTransparentPass,
                        FloodlightPassLabel,
                        Node3d::EndMainPass,
                    ),
                );
        }
    }
}

fn toggle_volumetric_mode(
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

#[derive(Component)]
struct ModeLabel;

fn spawn_mode_label(mut commands: Commands) {
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

fn update_mode_label(
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
