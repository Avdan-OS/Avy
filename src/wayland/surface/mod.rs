use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use smithay_client_toolkit::reexports::{
    client::{protocol::wl_surface::WlSurface, Connection, QueueHandle},
    protocols::wp::viewporter::client::wp_viewport::WpViewport,
};

use crate::{
    util::{AsAny, Size},
    AvyClient,
};

pub mod layer;

pub trait AvySurface: AsAny + InputHandler {
    fn wl_surface(&self) -> &WlSurface;

    fn size(&self) -> &Arc<RwLock<Size>>;

    fn size_ref(&self) -> RwLockReadGuard<'_, Size> {
        self.size().read().unwrap()
    }

    fn size_mut(&mut self) -> RwLockWriteGuard<'_, Size> {
        self.size().write().unwrap()
    }

    fn viewport(&mut self) -> &mut WpViewport;
}

pub trait InputHandler: KeyboardHandler + TouchHandler + PointerHandler {}

pub trait KeyboardHandler {
    #[allow(clippy::too_many_arguments)]
    fn enter(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        serial: u32,
        raw: &[u32],
        keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    );

    fn leave(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        serial: u32,
    );

    fn press_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    );

    fn release_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    );

    fn update_modifiers(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        modifiers: smithay_client_toolkit::seat::keyboard::Modifiers,
        layout: u32,
    );
}

pub trait TouchHandler {
    #[allow(clippy::too_many_arguments)]
    fn down(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        serial: u32,
        time: u32,
        surface: WlSurface,
        id: i32,
        position: (f64, f64),
    );

    fn up(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        serial: u32,
        time: u32,
        id: i32,
    );

    fn motion(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        time: u32,
        id: i32,
        position: (f64, f64),
    );

    fn shape(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        id: i32,
        major: f64,
        minor: f64,
    );

    fn orientation(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        id: i32,
        orientation: f64,
    );

    fn cancel(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
    );
}

pub trait PointerHandler {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<AvyClient>,
        pointer: &smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer,
        events: &[smithay_client_toolkit::seat::pointer::PointerEvent],
    );
}
