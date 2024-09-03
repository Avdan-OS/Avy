use std::sync::{Arc, RwLock};

use smithay_client_toolkit::{
    reexports::{
        client::{
            protocol::{wl_output::WlOutput, wl_surface::WlSurface},
            EventQueue,
        },
        protocols::wp::viewporter::client::wp_viewport::WpViewport,
    },
    shell::{wlr_layer, WaylandSurface},
};

use crate::{
    app::{AvyClient, RegisteredSurface},
    impl_as_any,
    util::Size,
};

use super::{AvySurface, InputHandler, KeyboardHandler, PointerHandler, TouchHandler};

pub struct AvyLayerParams<'a> {
    pub layer: wlr_layer::Layer,
    pub namespace: Option<&'a str>,
    pub output: Option<&'a WlOutput>,

    pub anchor: wlr_layer::Anchor,
    pub size: Size,
    pub margin: Option<(i32, i32, i32, i32)>,
    pub keyboard_interactivity: wlr_layer::KeyboardInteractivity,
}

pub struct AvyLayer {
    layer: wlr_layer::LayerSurface,
    viewport: WpViewport,
    size: Arc<RwLock<Size>>,
}

impl_as_any!(AvyLayer);

impl AvySurface for AvyLayer {
    fn wl_surface(&self) -> &WlSurface {
        self.layer.wl_surface()
    }

    fn viewport(&mut self) -> &mut WpViewport {
        &mut self.viewport
    }

    fn size(&self) -> &Arc<RwLock<Size>> {
        &self.size
    }
}

impl InputHandler for AvyLayer {}

impl AvyLayer {
    pub fn build<'a>(
        app: &'a mut AvyClient,
        event_queue: &mut EventQueue<AvyClient>,
        params: AvyLayerParams,
    ) -> RegisteredSurface<'a> {
        let qh = &event_queue.handle();

        // Setup layer surface.
        let wl_surface = app.compositor_state.create_surface(qh);
        let layer = app.layer_state.create_layer_surface(
            qh,
            wl_surface.clone(),
            params.layer,
            params.namespace,
            params.output,
        );

        layer.set_anchor(params.anchor);

        let (width, height) = params.size.logical_size();
        layer.set_size(width, height);

        layer.set_keyboard_interactivity(params.keyboard_interactivity);

        if let Some((top, right, bottom, left)) = params.margin {
            layer.set_margin(top, right, bottom, left);
        }

        // Use fractional scaling.
        app.fractional_scale.fractional_scaling(&wl_surface, qh);

        // Make a viewport for the surface.
        let viewport = app.viewporter.get_viewport(&wl_surface, qh);

        let registered_surface = app.register_surface(
            AvyLayer {
                layer: layer.clone(),
                viewport,
                size: Arc::new(RwLock::new(params.size)),
            },
            event_queue,
        );

        registered_surface
    }
}

#[allow(unused)]
impl KeyboardHandler for AvyLayer {
    fn enter(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        serial: u32,
        raw: &[u32],
        keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
    }

    fn leave(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
    }

    fn release_key(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        modifiers: smithay_client_toolkit::seat::keyboard::Modifiers,
        layout: u32,
    ) {
    }
}

#[allow(unused)]
impl TouchHandler for AvyLayer {
    fn down(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        serial: u32,
        time: u32,
        surface: WlSurface,
        id: i32,
        position: (f64, f64),
    ) {
        println!("Touch down: {position:?}")
    }

    fn up(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        serial: u32,
        time: u32,
        id: i32,
    ) {
    }

    fn motion(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        time: u32,
        id: i32,
        position: (f64, f64),
    ) {
        println!("Touch move: {position:?}")
    }

    fn shape(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        id: i32,
        major: f64,
        minor: f64,
    ) {
    }

    fn orientation(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        id: i32,
        orientation: f64,
    ) {
    }

    fn cancel(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
    ) {
    }
}

#[allow(unused)]
impl PointerHandler for AvyLayer {
    fn pointer_frame(
        &mut self,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<AvyClient>,
        pointer: &smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer,
        events: &[smithay_client_toolkit::seat::pointer::PointerEvent],
    ) {
    }
}
