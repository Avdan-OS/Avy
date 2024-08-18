#![allow(unused)]
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        client::{globals::GlobalList, protocol::wl_surface::WlSurface, Connection, QueueHandle},
        protocols::wp::viewporter::client::wp_viewport::WpViewport,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
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
