//!
//! Support for various graphics backends.
//!

use std::any::Any;

use smithay_client_toolkit::reexports::client::protocol::wl_display::WlDisplay;

use crate::{
    util::{AsAny, Size},
    wayland::surface::AvySurface,
};

pub mod vulkan;

pub trait GraphicsBackend {
    type Surface: GraphicsSurface;
    type Error: std::error::Error + AsAny;

    fn for_surface(
        &self,
        wl_display: &WlDisplay,
        wl_surface: &(impl AvySurface + ?Sized),
    ) -> Result<Self::Surface, Self::Error>;
}

pub trait GraphicsSurface: Send{
    fn render(
        &mut self,
        size: &Size,
        callback: &mut dyn FnMut(&skia_safe::Canvas),
    ) -> Result<(), Box<dyn Any>>;
}
