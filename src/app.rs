#![allow(unused)]
use std::{
    collections::HashMap,
    marker::PhantomData,
    process::id,
    sync::{Arc, Mutex, RwLock},
};

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_relative_pointer, delegate_seat, delegate_shm, delegate_touch,
    output::{OutputHandler, OutputState},
    reexports::{
        client::{
            globals::GlobalList,
            protocol::{
                wl_display::WlDisplay, wl_keyboard::WlKeyboard, wl_pointer::WlPointer,
                wl_surface::WlSurface, wl_touch::WlTouch,
            },
            Connection, EventQueue, Proxy, QueueHandle,
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
    shell::{
        wlr_layer::{LayerShell, LayerShellHandler},
        WaylandSurface,
    },
    shm::{Shm, ShmHandler},
};
use wayland_backend::client::ObjectId;

use crate::{
    delegate_fractional_scale, delegate_viewporter,
    graphics::{GraphicsBackend, GraphicsSurface},
    util::Size,
    wayland::{
        protocol::{
            fractional_scale::{FractionalScaleHandler, FractionalScaleManager, ScaleFactor},
            viewporter::{Viewport, Viewporter},
        },
        surface::AvySurface,
    },
};

pub struct AvySurfaceHandle<G> {
    __: PhantomData<G>,
    size: Arc<RwLock<Size>>,
    backend: Arc<Mutex<dyn GraphicsSurface>>,
}

impl<G: GraphicsBackend> AvySurfaceHandle<G> {
    pub fn render(&self, mut callback: impl FnMut(&skia_safe::Canvas)) -> Result<(), G::Error>
    where
        G::Error: 'static,
    {
        self.backend
            .lock()
            .unwrap()
            .render(&self.size.read().unwrap(), &mut callback)
            .map_err(|err| *err.downcast::<G::Error>().unwrap())
    }
}

pub struct RegisteredSurface<'a>(&'a mut AvyClient, ObjectId);

impl<'a> RegisteredSurface<'a> {
    pub fn make_backend<G: GraphicsBackend>(
        self,
        backend: &G,
    ) -> Result<AvySurfaceHandle<G>, G::Error>
    where
        G::Surface: 'static,
    {
        let id = self.1;
        let surface = self.0.surfaces.get(&id).unwrap().as_ref();
        let backend = backend.for_surface(&self.0.wl_display, surface)?;

        let backend = Arc::new(Mutex::new(backend));
        self.0.surface_backends.insert(id.clone(), backend.clone());

        Ok(AvySurfaceHandle {
            __: PhantomData,
            size: surface.size().clone(),
            backend,
        })
    }
}
pub struct AvyClient {
    pub wl_display: WlDisplay,
    pub registry_state: RegistryState,
    pub compositor_state: CompositorState,
    pub output_state: OutputState,
    pub shm_state: Shm,
    pub layer_state: LayerShell,
    pub fractional_scale: FractionalScaleManager,
    pub viewporter: Viewporter,
    pub seat_state: SeatState,
    pub relative_pointer_state: RelativePointerState,

    pub surfaces: HashMap<ObjectId, Box<dyn AvySurface>>,
    pub surface_backends: HashMap<ObjectId, Arc<Mutex<dyn GraphicsSurface>>>,

    pub pointer: Option<WlPointer>,
    pub relative_pointer: Option<ZwpRelativePointerV1>,

    pub keyboard: Option<WlKeyboard>,
    pub keyboard_focus: Option<ObjectId>,

    pub touch: Option<WlTouch>,
    pub active_touches: HashMap<i32, ObjectId>,

    pub running: bool,
}

impl AvyClient {
    pub fn new(
        global_list: &GlobalList,
        queue_handle: &QueueHandle<Self>,
        logical_size: (u32, u32),
        wl_display: WlDisplay,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            wl_display,
            registry_state: RegistryState::new(global_list),
            compositor_state: CompositorState::bind(global_list, queue_handle)?,
            output_state: OutputState::new(global_list, queue_handle),
            shm_state: Shm::bind(global_list, queue_handle)?,
            layer_state: LayerShell::bind(global_list, queue_handle)?,
            fractional_scale: FractionalScaleManager::new(global_list, queue_handle)?,
            viewporter: Viewporter::new(global_list, queue_handle)?,
            seat_state: SeatState::new(global_list, queue_handle),
            relative_pointer_state: RelativePointerState::bind(global_list, queue_handle),

            surfaces: HashMap::new(),
            surface_backends: HashMap::new(),

            pointer: None,
            relative_pointer: None,
            keyboard: None,
            keyboard_focus: None,
            touch: None,
            active_touches: HashMap::new(),

            running: true,
        })
    }

    pub fn register_surface<S: AvySurface + 'static>(
        &mut self,
        surface: S,
        event_queue: &mut EventQueue<Self>,
    ) -> RegisteredSurface {
        let id = surface.wl_surface().id();

        self.surfaces.insert(id.clone(), Box::new(surface));

        {
            let surface = self
                .surfaces
                .get(&id)
                .unwrap()
                .as_any_ref()
                .downcast_ref::<S>()
                .unwrap();

            surface.wl_surface().commit();
        }

        event_queue.roundtrip(self).unwrap();

        RegisteredSurface(self, id)
    }
}

impl ShmHandler for AvyClient {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm_state
    }
}

delegate_shm!(AvyClient);

impl ProvidesRegistryState for AvyClient {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState);
}

impl CompositorHandler for AvyClient {
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

impl OutputHandler for AvyClient {
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

delegate_compositor!(AvyClient);
delegate_output!(AvyClient);
delegate_registry!(AvyClient);

delegate_layer!(AvyClient);

impl LayerShellHandler for AvyClient {
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
        let surface = self
            .surfaces
            .get_mut(&layer.wl_surface().id())
            .expect("Surface not registered!")
            .as_mut();

        surface.size_mut().resize(configure.new_size);

        // Update viewport.
        let size = surface.size_ref().clone();

        let (width, height) = size.logical_size();
        surface.viewport().set_destination(width as _, height as _);

        let (width, height) = size.physical_size();
        surface.viewport().set_source(0.0, 0.0, width, height);
    }
}

delegate_fractional_scale!(AvyClient);

impl FractionalScaleHandler for AvyClient {
    fn scale_factor_changed(
        &mut self,
        connection: &smithay_client_toolkit::reexports::client::Connection,
        qh: &QueueHandle<Self>,
        surface: &WlSurface,
        factor: ScaleFactor,
    ) {
        let surface = self.surfaces.get_mut(&surface.id()).unwrap().as_mut();

        surface.size_mut().rescale(factor);

        // Update viewport.
        let size = surface.size_ref().clone();

        let (width, height) = size.logical_size();
        surface.viewport().set_destination(width as _, height as _);

        let (width, height) = size.physical_size();
        surface.viewport().set_source(0.0, 0.0, width, height);
    }
}

delegate_viewporter!(AvyClient);

impl SeatHandler for AvyClient {
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

delegate_seat!(AvyClient);

impl PointerHandler for AvyClient {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer,
        events: &[smithay_client_toolkit::seat::pointer::PointerEvent],
    ) {
        // TODO: Check the performance of this section.
        for event in events.as_chunks::<1>().0 {
            if let Some(surface) = self.surfaces.get_mut(&event[0].surface.id()) {
                surface.pointer_frame(conn, qh, pointer, event);
            }
        }
    }
}

delegate_pointer!(AvyClient);

impl RelativePointerHandler for AvyClient {
    fn relative_pointer_motion(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        relative_pointer: &smithay_client_toolkit::reexports::protocols::wp::relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1,
        pointer: &smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer,
        event: smithay_client_toolkit::seat::relative_pointer::RelativeMotionEvent,
    ) {
        // TODO: Check if this is actually necessary...
        println!("Relative pointer motion: {event:?}");
    }
}

delegate_relative_pointer!(AvyClient);

impl KeyboardHandler for AvyClient {
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
        self.keyboard_focus.replace(surface.id());
        self.surfaces
            .get_mut(&surface.id())
            .unwrap()
            .enter(conn, qh, keyboard, surface, serial, raw, keysyms)
    }

    fn leave(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        serial: u32,
    ) {
        let id = surface.id();
        self.surfaces
            .get_mut(&id)
            .unwrap()
            .leave(conn, qh, keyboard, surface, serial);

        self.keyboard_focus.take();
    }

    fn press_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        if let Some(focus) = &self.keyboard_focus {
            self.surfaces
                .get_mut(focus)
                .unwrap()
                .press_key(conn, qh, keyboard, serial, event)
        }
    }

    fn release_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        if let Some(focus) = &self.keyboard_focus {
            self.surfaces
                .get_mut(focus)
                .unwrap()
                .release_key(conn, qh, keyboard, serial, event)
        }
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
        if let Some(focus) = &self.keyboard_focus {
            self.surfaces
                .get_mut(focus)
                .unwrap()
                .update_modifiers(conn, qh, keyboard, serial, modifiers, layout)
        }
    }
}
delegate_keyboard!(AvyClient);

impl TouchHandler for AvyClient {
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
        let surface_id = surface.id();
        self.surfaces
            .get_mut(&surface_id)
            .unwrap()
            .down(conn, qh, touch, serial, time, surface, id, position);

        self.active_touches.insert(id, surface_id);
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
        let surface = self.active_touches.remove(&id).unwrap();
        self.surfaces
            .get_mut(&surface)
            .unwrap()
            .up(conn, qh, touch, serial, time, id);
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
        self.surfaces
            .get_mut(self.active_touches.get(&id).unwrap())
            .unwrap()
            .motion(conn, qh, touch, time, id, position)
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
        self.surfaces
            .get_mut(self.active_touches.get(&id).unwrap())
            .unwrap()
            .shape(conn, qh, touch, id, major, minor)
    }

    fn orientation(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        id: i32,
        orientation: f64,
    ) {
        self.surfaces
            .get_mut(self.active_touches.get(&id).unwrap())
            .unwrap()
            .orientation(conn, qh, touch, id, orientation)
    }

    fn cancel(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
    ) {
        // BUG: This may cause unintended effects, but this
        //      can be fixed later.
        let surface = self.active_touches.values().next().unwrap();
        self.surfaces
            .get_mut(surface)
            .unwrap()
            .cancel(conn, qh, touch);

        self.active_touches.clear();
    }
}

delegate_touch!(AvyClient);
