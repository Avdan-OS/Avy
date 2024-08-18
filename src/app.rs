#![allow(unused)]
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_relative_pointer, delegate_seat, delegate_shm, delegate_touch,
    output::{OutputHandler, OutputState},
    reexports::{
        client::{
            globals::GlobalList,
            protocol::{
                wl_keyboard::WlKeyboard, wl_pointer::WlPointer, wl_surface::WlSurface,
                wl_touch::WlTouch,
            },
            Connection, QueueHandle,
        },
        protocols::wp::{
            relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1,
            viewporter::client::wp_viewport::WpViewport,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyboardData, KeyboardHandler},
        pointer::{PointerData, PointerHandler},
        relative_pointer::{RelativePointerHandler, RelativePointerState},
        touch::{TouchData, TouchHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::wlr_layer::{LayerShell, LayerShellHandler},
    shm::{Shm, ShmHandler},
};

use crate::{
    delegate_fractional_scale, delegate_viewporter,
    protocols::{
        fractional_scale::{FractionalScaleHandler, FractionalScaleManager, ScaleFactor},
        viewporter::{Viewport, Viewporter},
    },
};

pub struct App {
    pub registry_state: RegistryState,
    pub compositor_state: CompositorState,
    pub output_state: OutputState,
    pub shm_state: Shm,
    pub layer_state: LayerShell,
    pub fractional_scale: FractionalScaleManager,
    pub viewporter: Viewporter,
    pub seat_state: SeatState,
    pub relative_pointer_state: RelativePointerState,

    pub pointer: Option<WlPointer>,
    pub relative_pointer: Option<ZwpRelativePointerV1>,

    pub keyboard: Option<WlKeyboard>,
    pub touch: Option<WlTouch>,

    pub running: bool,
    pub logical_size: (u32, u32),
    pub changed_size: bool,
    pub first_configure: bool,
    pub viewport: Option<WpViewport>,
    pub scale_factor: Option<ScaleFactor>,
}

impl App {
    pub fn new(
        global_list: &GlobalList,
        queue_handle: &QueueHandle<Self>,
        logical_size: (u32, u32),
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            registry_state: RegistryState::new(global_list),
            compositor_state: CompositorState::bind(global_list, queue_handle)?,
            output_state: OutputState::new(global_list, queue_handle),
            shm_state: Shm::bind(global_list, queue_handle)?,
            layer_state: LayerShell::bind(global_list, queue_handle)?,
            fractional_scale: FractionalScaleManager::new(global_list, queue_handle)?,
            viewporter: Viewporter::new(global_list, queue_handle)?,
            seat_state: SeatState::new(global_list, queue_handle),
            relative_pointer_state: RelativePointerState::bind(global_list, queue_handle),

            pointer: None,
            relative_pointer: None,
            keyboard: None,
            touch: None,

            running: true,
            logical_size,
            changed_size: false,
            first_configure: true,
            scale_factor: None,
            viewport: None,
        })
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm_state
    }
}

delegate_shm!(App);

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState);
}

impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        surface: &WlSurface,
        new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        surface: &WlSurface,
        new_transform: smithay_client_toolkit::reexports::client::protocol::wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        surface: &WlSurface,
        time: u32,
    ) {
        println!("WAYLAND@Compositor: Frame requested!");
    }

    fn surface_enter(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        surface: &WlSurface,
        output: &smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        surface: &WlSurface,
        output: &smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        output: smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        output: smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        output: smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
    ) {
    }
}

delegate_compositor!(App);
delegate_output!(App);
delegate_registry!(App);

delegate_layer!(App);

impl LayerShellHandler for App {
    fn closed(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    ) {
    }

    fn configure(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
        configure: smithay_client_toolkit::shell::wlr_layer::LayerSurfaceConfigure,
        serial: u32,
    ) {
        println!("WAYLAND:LayerShell: {configure:?}");
        self.logical_size = configure.new_size;
        self.changed_size = true;

        if self.first_configure {
            // Draw frame here!
            self.first_configure = false;
        }
    }
}

delegate_fractional_scale!(App);

impl FractionalScaleHandler for App {
    fn scale_factor_changed(
        &mut self,
        connection: &smithay_client_toolkit::reexports::client::Connection,
        qh: &QueueHandle<Self>,
        surface: &WlSurface,
        factor: ScaleFactor,
    ) {
        println!("New scale factor: {factor:?}");
        self.scale_factor.replace(factor);
        self.changed_size = true;
        if let Some(viewport) = &self.viewport {
            // Change the source buffer.
            viewport.set_source(
                0.0,
                0.0,
                factor.scale(self.logical_size.0),
                factor.scale(self.logical_size.1),
            );
            viewport.set_destination(self.logical_size.0 as i32, self.logical_size.1 as i32);
        }
    }
}

delegate_viewporter!(App);

impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer,
        events: &[smithay_client_toolkit::seat::pointer::PointerEvent],
    ) {
        println!("Pointer frame events: {events:?}");
    }
}

delegate_pointer!(App);

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut smithay_client_toolkit::seat::SeatState {
        &mut self.seat_state
    }

    fn new_seat(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat,
    ) {
    }

    fn new_capability(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = seat.get_pointer(qh, PointerData::new(seat.clone()));
            if let Ok(rel_pointer) = self
                .relative_pointer_state
                .get_relative_pointer(&pointer, qh)
            {
                println!("Created relative pointer!");
                self.relative_pointer.replace(rel_pointer);
            }
        }

        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard
                .replace(seat.get_keyboard(qh, KeyboardData::new(seat.clone())));
        }

        if capability == Capability::Touch {
            self.touch
                .replace(seat.get_touch(qh, TouchData::new(seat.clone())));
        }
    }

    fn remove_capability(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            self.keyboard.take();
        }

        if capability == Capability::Pointer {
            self.pointer.take();
            self.relative_pointer.take();
        }

        if capability == Capability::Touch {
            self.touch.take();
        }
    }

    fn remove_seat(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat,
    ) {
        self.keyboard.take();
        self.pointer.take();
        self.relative_pointer.take();
    }
}

delegate_seat!(App);

impl RelativePointerHandler for App {
    fn relative_pointer_motion(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        relative_pointer: &smithay_client_toolkit::reexports::protocols::wp::relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1,
        pointer: &smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer,
        event: smithay_client_toolkit::seat::relative_pointer::RelativeMotionEvent,
    ) {
        println!("Relative pointer motion: {event:?}");
    }
}

delegate_relative_pointer!(App);

impl KeyboardHandler for App {
    fn enter(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        serial: u32,
        raw: &[u32],
        keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
        println!("Keyboard enter!");
    }

    fn leave(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        println!("Press key! {:?}", event.keysym);
    }

    fn release_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        println!("Release key! {:?}", event.keysym);
    }

    fn update_modifiers(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        modifiers: smithay_client_toolkit::seat::keyboard::Modifiers,
        layout: u32,
    ) {
    }
}
delegate_keyboard!(App);

impl TouchHandler for App {
    fn down(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        serial: u32,
        time: u32,
        surface: WlSurface,
        id: i32,
        position: (f64, f64),
    ) {
        println!("Touch Down: {id} => {position:?}");
    }

    fn up(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        serial: u32,
        time: u32,
        id: i32,
    ) {
        println!("Touch Up: {id}");
    }

    fn motion(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        time: u32,
        id: i32,
        position: (f64, f64),
    ) {
        println!("Touch Motion: {id} => {position:?}");
    }

    fn shape(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        id: i32,
        major: f64,
        minor: f64,
    ) {
    }

    fn orientation(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        id: i32,
        orientation: f64,
    ) {
    }

    fn cancel(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
    ) {
    }
}

delegate_touch!(App);
