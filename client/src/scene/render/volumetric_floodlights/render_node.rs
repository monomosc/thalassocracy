use bevy::ecs::query::QueryItem;
use bevy::pbr::{ViewFogUniformOffset, ViewShadowBindings};
use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    mesh::{allocator::MeshAllocator, RenderMesh, RenderMeshBufferInfo},
    render_asset::RenderAssets,
    render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode},
    render_resource::{
        IndexFormat, LoadOp, Operations, PipelineCache, RenderPassDepthStencilAttachment,
        RenderPassDescriptor, StoreOp,
    },
    renderer::RenderContext,
    view::{ViewDepthTexture, ViewTarget},
};

use super::{pipeline::ViewConeRenderData, RenderVolumetricLightingMode, VolumetricLightingMode};

#[derive(RenderLabel, Debug, Clone, Hash, PartialEq, Eq)]
pub(super) struct FloodlightPassLabel;

#[derive(Default)]
pub(super) struct FloodlightViewNode;

impl ViewNode for FloodlightViewNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        Option<&'static ViewDepthTexture>,
        Option<&'static ViewConeRenderData>,
        Option<&'static ViewFogUniformOffset>,
        &'static ViewShadowBindings,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (camera, target, depth_texture, render_data, fog_offset, _view_shadow_bindings): QueryItem<
            Self::ViewQuery,
        >,
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
        render_pass.set_bind_group(0, &render_data.global, &[]); //Global shadow atlas
        render_pass.set_bind_group(1, &render_data.view, &[]); //depth-stencil texture
        if let (Some(fog_bg), Some(fog_offset)) = (&render_data.fog, fog_offset) {
            render_pass.set_bind_group(3, fog_bg, &[fog_offset.offset]); // DistanceFog GPU uniform
        }
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
