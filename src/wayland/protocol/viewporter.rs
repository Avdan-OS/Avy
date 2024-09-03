use smithay_client_toolkit::{
    globals::GlobalData,
    reexports::{
        client::{
            globals::{BindError, GlobalList},
            protocol::wl_surface::WlSurface,
            Dispatch, QueueHandle,
        },
        protocols::wp::viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
    },
};

pub struct Viewporter(WpViewporter);

impl Viewporter {
    pub fn new<State: Dispatch<WpViewporter, GlobalData> + 'static>(
        globals: &GlobalList,
        queue_handle: &QueueHandle<State>,
    ) -> Result<Self, BindError> {
        let wp_viewporter = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self(wp_viewporter))
    }

    pub fn get_viewport<State: Dispatch<WpViewport, Viewport> + 'static>(
        &self,
        surface: &WlSurface,
        qh: &QueueHandle<State>,
    ) -> WpViewport {
        self.0.get_viewport(
            surface,
            qh,
            Viewport {
                surface: surface.clone(),
            },
        )
    }
}

impl<State> Dispatch<WpViewporter, GlobalData, State> for Viewporter
where
    State: Dispatch<WpViewporter, GlobalData>,
{
    fn event(
        _: &mut State,
        _: &WpViewporter,
        _: <WpViewporter as smithay_client_toolkit::reexports::client::Proxy>::Event,
        _: &GlobalData,
        _: &smithay_client_toolkit::reexports::client::Connection,
        _: &QueueHandle<State>,
    ) {
        // No events.
    }
}

pub struct Viewport {
    #[allow(unused)]
    surface: WlSurface,
}

impl<State> Dispatch<WpViewport, Viewport, State> for Viewport
where
    State: Dispatch<WpViewport, Viewport>,
{
    fn event(
        _: &mut State,
        _: &WpViewport,
        _: <WpViewport as smithay_client_toolkit::reexports::client::Proxy>::Event,
        _: &Viewport,
        _: &smithay_client_toolkit::reexports::client::Connection,
        _: &QueueHandle<State>,
    ) {
        // No events...
    }
}

#[macro_export]
macro_rules! delegate_viewporter {
    ($(@<$( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+>)? $ty: ty) => {
        smithay_client_toolkit::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewporter::WpViewporter: smithay_client_toolkit::globals::GlobalData
        ] => $crate::wayland::protocol::viewporter::Viewporter);
        smithay_client_toolkit::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport: $crate::wayland::protocol::viewporter::Viewport
        ] => $crate::wayland::protocol::viewporter::Viewport);
    };
}
