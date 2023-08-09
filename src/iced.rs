use iced_widget::renderer;
use iced_widget::style;

pub use style::theme;

pub use iced_core::alignment;
pub use iced_core::event;
pub use iced_core::gradient;
pub use iced_core::{
    color, Alignment, Background, BorderRadius, Color, ContentFit, Degrees, Gradient, Length,
    Padding, Pixels, Point, Radians, Rectangle, Size, Vector,
};
pub use iced_runtime::Command;

pub mod clipboard {
    //! Access the clipboard.
    pub use iced_runtime::clipboard::{read, write};
}

pub mod font {
    //! Load and use fonts.
    pub use iced_core::font::*;
    pub use iced_runtime::font::*;
}

pub mod keyboard {
    //! Listen and react to keyboard events.
    pub use iced_core::keyboard::{Event, KeyCode, Modifiers};
}

pub mod mouse {
    //! Listen and react to mouse events.
    pub use iced_core::mouse::{Button, Cursor, Event, Interaction, ScrollDelta};
}

pub mod overlay {
    //! Display interactive elements on top of other widgets.

    /// A generic [`Overlay`].
    ///
    /// This is an alias of an `iced_native` element with a default `Renderer`.
    ///
    /// [`Overlay`]: iced_native::Overlay
    pub type Element<'a, Message, Renderer = crate::Renderer> =
        iced_core::overlay::Element<'a, Message, Renderer>;

    pub use iced_widget::overlay::*;
}

pub mod touch {
    //! Listen and react to touch events.
    pub use iced_core::touch::{Event, Finger};
}

pub mod widget {
    //! Use the built-in widgets or create your own.
    pub use iced_widget::*;

    // We hide the re-exported modules by `iced_widget`
    mod core {}
    mod graphics {}
    mod native {}
    mod renderer {}
    mod style {}
    mod runtime {}
}

pub use event::Event;
pub use font::Font;
pub use theme::Theme;

/// The default renderer.
pub type Renderer<Theme = style::Theme> = renderer::Renderer<Theme>;

/// A generic widget.
///
/// This is an alias of an `iced_native` element with a default `Renderer`.
pub type Element<'a, Message, Renderer = crate::Renderer> =
    iced_core::Element<'a, Message, Renderer>;

pub use iced_core::renderer::Style;
pub use iced_wgpu::Settings;
