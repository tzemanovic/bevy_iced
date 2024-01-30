use bevy_derive::{Deref, DerefMut};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Query;
use bevy_ecs::{
    system::{Commands, Res, Resource},
    world::World,
};
use bevy_render::render_graph::RenderLabel;
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::{
    render_graph::{Node, NodeRunError, RenderGraphContext},
    renderer::RenderContext,
    view::ExtractedWindows,
    Extract,
};
use bevy_utils::HashMap;
use iced_wgpu::wgpu::util::StagingBelt;
use iced_wgpu::wgpu::TextureFormat;
use iced_widget::graphics::Viewport;
use std::sync::Mutex;

use crate::{DidDraw, IcedRenderer, IcedRenderers, WindowViewport};

#[derive(Clone, Hash, Debug, Eq, PartialEq, RenderLabel)]
pub struct IcedPass;

#[cfg(target_arch = "wasm32")]
pub const TEXTURE_FMT: TextureFormat = TextureFormat::Rgba8UnormSrgb;
#[cfg(not(target_arch = "wasm32"))]
pub const TEXTURE_FMT: TextureFormat = TextureFormat::Bgra8UnormSrgb;

/// This resource is used to pass all the viewports attached to windows into
/// the `RenderApp` sub app.
#[derive(Debug, Deref, DerefMut, Clone, Resource)]
pub struct ExtractedIcedWindows(HashMap<Entity, ExtractedIcedWindow>);

#[derive(Debug, Clone)]
pub struct ExtractedIcedWindow {
    viewport: Viewport,
    did_draw: bool,
}

pub(crate) fn extract_iced_data(
    mut commands: Commands,
    windows: Extract<Query<(Entity, &WindowViewport, &DidDraw)>>,
    renderers: Extract<Res<IcedRenderers>>,
) {
    let extracted_windows = windows
        .iter()
        .map(|(window, WindowViewport(viewport), did_draw)| {
            (
                window,
                ExtractedIcedWindow {
                    viewport: viewport.clone(),
                    did_draw: did_draw.swap(false, std::sync::atomic::Ordering::Relaxed),
                },
            )
        })
        .collect();
    commands.insert_resource(ExtractedIcedWindows(extracted_windows));
    commands.insert_resource(renderers.clone());
}

pub struct IcedNode {
    staging_belt: Mutex<StagingBelt>,
}

impl IcedNode {
    pub fn new() -> Self {
        Self {
            staging_belt: Mutex::new(StagingBelt::new(5 * 1024)),
        }
    }
}

impl Node for IcedNode {
    fn update(&mut self, _world: &mut World) {
        self.staging_belt.lock().unwrap().recall();
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let windows = &world.get_resource::<ExtractedWindows>().unwrap().windows;
        let ExtractedIcedWindows(extracted_windows) =
            world.get_resource::<ExtractedIcedWindows>().unwrap();

        let staging_belt = &mut *self.staging_belt.lock().unwrap();

        // Render all windows with viewports
        for (window_entity, ExtractedIcedWindow { viewport, did_draw }) in extracted_windows {
            if !did_draw {
                continue;
            }

            let window = windows.get(window_entity).unwrap();
            let render_device = world.resource::<RenderDevice>().wgpu_device();
            let render_queue = world.resource::<RenderQueue>();

            let view = window.swap_chain_texture_view.as_ref().unwrap();

            // TODO: in iced App this is a debug overlay
            let overlay_text: &[String] = &[];

            let renderers = world.resource::<IcedRenderers>();
            let renderer = renderers.get(window_entity);
            match renderer {
                // Nothing to draw in this window if there's no renderer
                None => {
                    continue;
                }
                Some(request_or_use) =>
                // Renderer lock scope
                {
                    let IcedRenderer(renderer) = &mut *request_or_use.lock().unwrap();
                    let crate::Renderer::Wgpu(renderer) = renderer else {
                        panic!("Only wgpu renderer is supported");
                    };

                    renderer.with_primitives(|backend, primitives| {
                        backend.present(
                            render_device,
                            render_queue,
                            render_context.command_encoder(),
                            None,
                            TEXTURE_FMT,
                            view,
                            primitives,
                            viewport,
                            overlay_text,
                        );
                    });
                }
            }
        }

        staging_belt.finish();

        Ok(())
    }
}
