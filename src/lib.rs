//! # Use Iced UI programs in your Bevy application
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_iced::iced::widget::text;
//! use bevy_iced::{IcedContext, IcedPlugin};
//!
//! #[derive(Event)]
//! pub enum UiMessage {}
//!
//! pub fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(IcedPlugin::default())
//!         .add_event::<UiMessage>()
//!         .add_systems(Update, ui_system)
//!         .run();
//! }
//!
//! fn ui_system(time: Res<Time>, mut ctx: IcedContext<UiMessage>) {
//!     ctx.display(text(format!(
//!         "Hello Iced! Running for {:.2} seconds.",
//!         time.elapsed_seconds()
//!     )));
//! }
//! ```

#![deny(unsafe_code)]
#![deny(missing_docs)]

use std::any::{Any, TypeId};

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;

use crate::render::{extract_iced_data, IcedNode};

use bevy_app::{App, Plugin, PreUpdate};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EventReader;
use bevy_ecs::prelude::{EventWriter, Query};
use bevy_ecs::query::With;
use bevy_ecs::system::{Commands, NonSendMut, Res, ResMut, Resource, SystemParam};
use bevy_input::touch::Touches;
use bevy_render::render_graph::RenderGraph;
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::{ExtractSchedule, RenderApp};
use bevy_utils::HashMap;
use bevy_window::{PrimaryWindow, Window, WindowClosed, WindowCreated, WindowResized};
use iced_core::mouse::Cursor;
use iced_core::Size;
use iced_runtime::user_interface::UserInterface;
use iced_widget::style::Theme;

/// Basic re-exports for all Iced-related stuff.
///
/// This module attempts to emulate the `iced` package's API
/// as much as possible.
pub mod iced;

mod conversions;
mod render;
mod systems;
mod utils;

use iced_wgpu::graphics::Viewport;
use systems::IcedEventQueue;

/// The default renderer.
pub type Renderer = iced_renderer::Renderer;

/// The main feature of `bevy_iced`.
/// Add this to your [`App`] by calling `app.add_plugin(bevy_iced::IcedPlugin::default())`.
#[derive(Debug, Default)]
pub struct IcedPlugin;

/// Iced settings and fonts. Note that changes to this resource will not
/// affected windows that are already opened.
#[derive(Debug, Default, Resource)]
pub struct IcedSetup {
    /// The settings that Iced should use.
    pub settings: iced::Settings,
    /// Font file contents
    pub fonts: Vec<&'static [u8]>,
}

impl Plugin for IcedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (
                systems::process_input,
                handle_window_created,
                handle_window_resized,
                handle_window_closed,
            ),
        )
        .insert_resource(IcedSetup::default())
        .insert_resource(IcedSettings::default())
        .insert_non_send_resource(IcedCache::default())
        .insert_resource(IcedEventQueue::default());
    }

    fn finish(&self, app: &mut App) {
        let renderers = IcedRenderers(HashMap::default());
        app.insert_resource(renderers).insert_resource(IcedState {
            clipboard: iced_core::clipboard::Null,
        });

        let render_app = app.sub_app_mut(RenderApp);
        render_app.add_systems(ExtractSchedule, extract_iced_data);
        setup_pipeline(&mut render_app.world_mut().get_resource_mut().unwrap());
    }
}

/// This component is attached to a window
#[derive(Component, Debug, Deref, DerefMut, Clone)]
pub struct WindowViewport(pub Viewport);

struct IcedRenderer(Renderer);

impl std::fmt::Debug for IcedRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IcedRenderer").finish()
    }
}

#[derive(Debug, Resource)]
struct IcedState {
    clipboard: iced_core::clipboard::Null,
}

#[derive(Resource, Clone, Debug, Deref, DerefMut)]
struct IcedRenderers(HashMap<Entity, Arc<Mutex<IcedRenderer>>>);

fn setup_pipeline(graph: &mut RenderGraph) {
    graph.add_node(render::IcedPass, IcedNode::new());

    graph.add_node_edge(bevy_render::graph::CameraDriverLabel, render::IcedPass);
}

#[derive(Default)]
struct IcedCache {
    cache: HashMap<TypeId, Option<iced_runtime::user_interface::Cache>>,
}

impl IcedCache {
    fn get<M: Any>(&mut self) -> &mut Option<iced_runtime::user_interface::Cache> {
        let id = TypeId::of::<M>();
        if !self.cache.contains_key(&id) {
            self.cache.insert(id, Some(Default::default()));
        }
        self.cache.get_mut(&id).unwrap()
    }
}

/// Settings used to independently customize Iced rendering.
#[derive(Clone, Resource)]
pub struct IcedSettings {
    /// The scale factor to use for rendering Iced elements.
    /// Setting this to `None` defaults to using the `Window`s scale factor.
    pub scale_factor: Option<f64>,
    /// The theme to use for rendering Iced elements.
    pub theme: iced_widget::style::Theme,
    /// The style to use for rendering Iced elements.
    pub style: iced::Style,
}

impl IcedSettings {
    /// Set the `scale_factor` used to render Iced elements.
    pub fn set_scale_factor(&mut self, factor: impl Into<Option<f64>>) {
        self.scale_factor = factor.into();
    }
}

impl Default for IcedSettings {
    fn default() -> Self {
        Self {
            scale_factor: None,
            theme: iced_widget::style::Theme::Dark,
            style: iced::Style {
                text_color: iced_core::Color::WHITE,
            },
        }
    }
}

// An atomic flag for updating the draw state.
#[derive(Component, Clone, Debug, Default, Deref, DerefMut)]
pub(crate) struct DidDraw(Arc<AtomicBool>);

/// The context for interacting with Iced. Add this as a parameter to your system.
/// ```ignore
/// fn ui_system(..., mut ctx: IcedContext<UiMessage>) {
///     let element = ...; // Build your element
///     ctx.display(element);
/// }
/// ```
///
/// `IcedContext<T>` requires an event system to be defined in the [`App`].
/// Do so by invoking `app.add_event::<T>()` when constructing your App.
#[derive(SystemParam)]
pub struct IcedContext<'w, 's, Message: bevy_ecs::event::Event> {
    renderers: ResMut<'w, IcedRenderers>,
    state: ResMut<'w, IcedState>,
    settings: Res<'w, IcedSettings>,
    primary_window: Query<'w, 's, Entity, (With<PrimaryWindow>, With<WindowViewport>)>,
    windows: Query<'w, 's, (&'static Window, &'static WindowViewport, &'static DidDraw)>,
    events: ResMut<'w, IcedEventQueue>,
    cache_map: NonSendMut<'w, IcedCache>,
    messages: EventWriter<'w, Message>,
    touches: Res<'w, Touches>,
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    setup: Res<'w, IcedSetup>,
}

impl<'w, 's, M: bevy_ecs::event::Event> IcedContext<'w, 's, M> {
    /// Display an [`Element`] in the given window.
    pub fn display_in_window<'a>(
        &'a mut self,
        element: impl Into<iced_core::Element<'a, M, Theme, Renderer>>,
        window_entity: Entity,
    ) {
        let (window, viewport, did_draw) = self.windows.get(window_entity).unwrap();
        let bounds = viewport.logical_size();

        let element = element.into();

        let cursor = {
            match window.cursor_position() {
                Some(position) => {
                    Cursor::Available(utils::process_cursor_position(position, bounds, window))
                }
                None => utils::process_touch_input(self)
                    .map(Cursor::Available)
                    .unwrap_or(Cursor::Unavailable),
            }
        };

        let mut messages = Vec::<M>::new();
        let cache_entry = self.cache_map.get::<M>();
        let cache = cache_entry.take().unwrap_or_default();

        if !self.renderers.contains_key(&window_entity) {
            self.renderers.insert(
                window_entity,
                Arc::new(Mutex::new(init_iced_renderer(
                    &self.device,
                    &self.queue,
                    &self.setup,
                ))),
            );
        }
        let renderer = self.renderers.get_mut(&window_entity).unwrap();
        // Renderer lock scope
        {
            let IcedRenderer(renderer) = &mut *renderer.lock().unwrap();
            let mut ui = UserInterface::build(element, bounds, cache, renderer);
            let (_, _event_statuses) = ui.update(
                self.events.as_slice(),
                cursor,
                renderer,
                &mut self.state.clipboard,
                &mut messages,
            );

            ui.draw(renderer, &self.settings.theme, &self.settings.style, cursor);
            *cache_entry = Some(ui.into_cache());
            did_draw.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        self.messages.send_batch(messages);
        self.events.clear();
    }

    /// Display an [`Element`] in the primary window.
    pub fn display<'a>(
        &'a mut self,
        element: impl Into<iced_core::Element<'a, M, Theme, Renderer>>,
    ) {
        if let Ok(window) = self.primary_window.get_single() {
            self.display_in_window(element, window)
        }
    }
}

fn init_iced_renderer(
    device: &RenderDevice,
    queue: &RenderQueue,
    setup: &IcedSetup,
) -> IcedRenderer {
    let mut backend = iced_wgpu::Backend::new(
        device.wgpu_device(),
        queue.as_ref(),
        setup.settings,
        crate::render::TEXTURE_FMT,
    );
    for font in &setup.fonts {
        iced_wgpu::graphics::backend::Text::load_font(
            &mut backend,
            std::borrow::Cow::Borrowed(*font),
        );
    }

    IcedRenderer(Renderer::Wgpu(iced_wgpu::Renderer::new(
        backend,
        setup.settings.default_font,
        setup.settings.default_text_size,
    )))
}

fn handle_window_created(
    mut commands: Commands,
    mut window: EventReader<WindowCreated>,
    created_windows: Query<&Window>,
    iced_settings: Res<IcedSettings>,
) {
    for WindowCreated { window } in window.read() {
        commands
            .entity(*window)
            .insert(DidDraw::default())
            .insert(get_window_viewport(
                created_windows.get(*window).unwrap(),
                &iced_settings,
            ));
    }
}

fn handle_window_resized(
    mut commands: Commands,
    mut window: EventReader<WindowResized>,
    created_windows: Query<&Window>,
    iced_settings: Res<IcedSettings>,
) {
    for WindowResized {
        window,
        width: _,
        height: _,
    } in window.read()
    {
        commands.entity(*window).insert(get_window_viewport(
            created_windows.get(*window).unwrap(),
            &iced_settings,
        ));
    }
}

fn handle_window_closed(
    mut window: EventReader<WindowClosed>,
    mut renderers: ResMut<IcedRenderers>,
) {
    for WindowClosed { window } in window.read() {
        renderers.remove(window);
    }
}

fn get_window_viewport(window: &Window, iced_settings: &IcedSettings) -> WindowViewport {
    let scale_factor = iced_settings
        .scale_factor
        .unwrap_or(window.scale_factor().into());
    let viewport = Viewport::with_physical_size(
        Size::new(window.physical_width(), window.physical_height()),
        scale_factor,
    );
    WindowViewport(viewport)
}
