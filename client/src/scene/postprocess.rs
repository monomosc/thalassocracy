use bevy::app::Plugin as BevyPlugin;
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, texture_depth_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::render_graph::{RenderGraphApp, ViewNodeRunner, NodeRunError, RenderGraphContext};
use bevy::render::renderer::RenderContext;
use bevy::render::view::{ViewTarget, ViewDepthTexture};
// no Extract import needed
use bevy::render::RenderApp;
use bevy::render::RenderSet;
use bevy::render::Render;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::asset::{load_internal_asset, Handle};
use bevy::prelude::Shader;
use bevy::ecs::query::QueryItem;

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};

// A simple screen-space water post-process that adds depth-tinted absorption,
// lightweight diffusion (scattering), and subtle refraction.

const WATER_POST_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0x2f8e_3a80_bf21_40aa_9a9d_1d2c_8840_12aa);

pub struct WaterPostProcessPlugin;

impl BevyPlugin for WaterPostProcessPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, WATER_POST_SHADER_HANDLE, "water_post.wgsl", Shader::from_wgsl);

        // Extract debug toggles into the render world
        app.add_plugins(ExtractResourcePlugin::<RenderVisToggles>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return; };

        render_app
            .init_resource::<SpecializedRenderPipelines<WaterPostPipeline>>()
            .add_systems(Render, prepare_water_post_pipelines.in_set(RenderSet::Prepare))
            .add_render_graph_node::<ViewNodeRunner<WaterPostNode>>(Core3d, Node3d::Fxaa)
            .add_render_graph_edges(Core3d, (Node3d::Tonemapping, Node3d::Fxaa, Node3d::EndMainPassPostProcessing));
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<WaterPostPipeline>();
        }
    }
}

// Extracted toggles/params from RenderSettings for use in the Render World
#[derive(Resource, Clone, Default)]
pub struct RenderVisToggles { pub water_post: bool, pub strength: f32, pub debug: bool }

impl ExtractResource for RenderVisToggles {
    type Source = crate::render_settings::RenderSettings;
    fn extract_resource(source: &Self::Source) -> Self {
        Self { water_post: source.water_post, strength: source.water_post_strength.max(0.0), debug: source.water_post_debug }
    }
}

#[derive(Resource)]
pub struct WaterPostPipeline {
    color_bind_group_layout: BindGroupLayout,
    depth_bind_group_layout: BindGroupLayout,
    params_bind_group_layout: BindGroupLayout,
    sampler: Sampler,
}

impl FromWorld for WaterPostPipeline {
    fn from_world(render_world: &mut World) -> Self {
        use bevy::render::renderer::RenderDevice;
        let device = render_world.resource::<RenderDevice>();
        let color_bind_group_layout = device.create_bind_group_layout(
            "water_post_color_bgl",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        );
        let depth_bind_group_layout = device.create_bind_group_layout(
            "water_post_depth_bgl",
            &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (texture_depth_2d(),)),
        );
        let params_bind_group_layout = device.create_bind_group_layout(
            "water_post_params_bgl",
            &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (uniform_buffer::<[f32; 4]>(false),)),
        );
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("water_post_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });
        Self { color_bind_group_layout, depth_bind_group_layout, params_bind_group_layout, sampler }
    }
}

#[derive(Component)]
pub struct CameraWaterPostPipeline {
    pub pipeline_id: CachedRenderPipelineId,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct WaterPostPipelineKey {
    pub format: TextureFormat,
    pub hdr: bool,
}

impl SpecializedRenderPipeline for WaterPostPipeline {
    type Key = WaterPostPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("water_post".into()),
            layout: vec![
                self.color_bind_group_layout.clone(),
                self.depth_bind_group_layout.clone(),
                self.params_bind_group_layout.clone(),
            ],
            vertex: bevy::core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state(),
            fragment: Some(FragmentState {
                shader: WATER_POST_SHADER_HANDLE,
                shader_defs: if key.hdr { vec!["HDR".into()] } else { vec![] },
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        }
    }
}

pub fn prepare_water_post_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<WaterPostPipeline>>,
    pipe: Res<WaterPostPipeline>,
    views: Query<(Entity, &bevy::render::view::ExtractedView)>,
) {
    for (entity, view) in &views {
        let fmt = if view.hdr { ViewTarget::TEXTURE_FORMAT_HDR } else { TextureFormat::bevy_default() };
        let id = pipelines.specialize(&pipeline_cache, &pipe, WaterPostPipelineKey { format: fmt, hdr: view.hdr });
        commands.entity(entity).insert(CameraWaterPostPipeline { pipeline_id: id });
    }
}

#[derive(Default)]
pub struct WaterPostNode {
    cached_color_bg: std::sync::Mutex<Option<(TextureViewId, BindGroup)>>,
    cached_depth_bg: std::sync::Mutex<Option<(TextureViewId, BindGroup)>>,
}

impl bevy::render::render_graph::ViewNode for WaterPostNode {
    type ViewQuery = (
        &'static ViewTarget,
        Option<&'static ViewDepthTexture>,
        &'static CameraWaterPostPipeline,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (target, depth_tex, pipeline): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // Toggle via extracted render settings
        let toggles = match world.get_resource::<RenderVisToggles>() { Some(t) => t, None => return Ok(()) };
        if !toggles.water_post { return Ok(()); }
        let pipeline_cache = world.resource::<PipelineCache>();
        let post_pipe = world.resource::<WaterPostPipeline>();

        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
            // Pipeline not ready yet (first frames)
            tracing::debug!("water_post: pipeline not ready, skipping frame");
            return Ok(());
        };

        // If depth texture isn't available yet, skip the pass this frame without swapping.
        let Some(depth_view) = depth_tex else {
            tracing::debug!("water_post: no depth view yet, skipping frame");
            return Ok(());
        };

        // Prepare color bind group
        let pp = target.post_process_write();
        let source = pp.source;
        let destination = pp.destination;

        let mut color_cache = self.cached_color_bg.lock().unwrap();
        let color_bg = match &mut *color_cache {
            Some((id, bg)) if *id == source.id() => bg,
            cache => {
                let bg = render_context.render_device().create_bind_group(
                    Some("water_post_color_bg"),
                    &post_pipe.color_bind_group_layout,
                    &BindGroupEntries::sequential((source, &post_pipe.sampler)),
                );
                let (_, bg) = cache.insert((source.id(), bg));
                bg
            }
        };

        // Prepare depth bind group if available
        let mut depth_cache = self.cached_depth_bg.lock().unwrap();
        let depth_bg = match &mut *depth_cache {
            Some((id, bg)) if *id == depth_view.view().id() => bg,
            cache => {
                let bg = render_context.render_device().create_bind_group(
                    Some("water_post_depth_bg"),
                    &post_pipe.depth_bind_group_layout,
                    &BindGroupEntries::single(depth_view.view()),
                );
                let (_, bg) = cache.insert((depth_view.view().id(), bg));
                bg
            }
        };

        // Create or update params bind group
        let params_data = [toggles.strength, if toggles.debug { 1.0 } else { 0.0 }, 0.0, 0.0];
        let device = render_context.render_device();
        let params_buffer = device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("water_post_params"),
            contents: bytemuck::cast_slice(&params_data),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let post_pipe = world.resource::<WaterPostPipeline>();
        let params_bg = device.create_bind_group(
            Some("water_post_params_bg"),
            &post_pipe.params_bind_group_layout,
            &BindGroupEntries::single(params_buffer.as_entire_binding()),
        );

        let pass_desc = RenderPassDescriptor {
            label: Some("water_post_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        };
        let mut pass = render_context.command_encoder().begin_render_pass(&pass_desc);
        tracing::debug!("water_post: executing fullscreen pass");
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, color_bg, &[]);
        pass.set_bind_group(1, depth_bg, &[]);
        pass.set_bind_group(2, &params_bg, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
