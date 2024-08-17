#![allow(unused)]
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::client::{globals::GlobalList, Connection, QueueHandle},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::{LayerShell, LayerShellHandler},
    shm::{Shm, ShmHandler},
};

pub struct App {
    pub registry_state: RegistryState,
    pub compositor_state: CompositorState,
    pub output_state: OutputState,
    pub shm_state: Shm,
    pub layer_state: LayerShell,

    pub running: bool,
    pub size: (u32, u32),
    pub changed_size: bool,
    pub first_configure: bool,
}

impl App {
    pub fn new(
        global_list: &GlobalList,
        queue_handle: &QueueHandle<Self>,
        size: (u32, u32),
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            registry_state: RegistryState::new(global_list),
            compositor_state: CompositorState::bind(global_list, queue_handle)?,
            output_state: OutputState::new(global_list, queue_handle),
            shm_state: Shm::bind(global_list, queue_handle)?,
            layer_state: LayerShell::bind(global_list, queue_handle)?,
            running: true,
            size,
            changed_size: false,
            first_configure: true,
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
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        new_transform: smithay_client_toolkit::reexports::client::protocol::wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        time: u32,
    ) {
        println!("WAYLAND@Compositor: Frame requested!");
    }

    fn surface_enter(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        output: &smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        conn: &Connection,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>,
        surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
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
        self.size = configure.new_size;
        self.changed_size = true;

        if self.first_configure {
            // Draw frame here!
            self.first_configure = false;
        }
    }
}
