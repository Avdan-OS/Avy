use smithay_client_toolkit::{
    globals::GlobalData,
    reexports::{
        client::{
            globals::{BindError, GlobalList},
            protocol::wl_surface::WlSurface,
            Dispatch, QueueHandle,
        },
        protocols::wp::fractional_scale::v1::client::{
            self as fractional_scale, wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
            wp_fractional_scale_v1::WpFractionalScaleV1,
        },
    },
};

///
/// Represents a valid fractional scale.
///
#[derive(Clone, Copy)]
pub struct ScaleFactor(u32);

impl std::fmt::Debug for ScaleFactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ScaleFactor")
            .field(&(self.0 as f64 / Self::DENOMINATOR))
            .finish()
    }
}

impl ScaleFactor {
    ///
    /// The denominator used for Wayland fractional scales.
    ///
    pub const DENOMINATOR: f64 = 120.0;

    pub fn as_f64(&self) -> f64 {
        self.0 as f64 / Self::DENOMINATOR
    }

    pub fn scale<T: Into<f64>>(&self, dim: T) -> f64 {
        (dim.into() * self.as_f64()).round() // Round half away from zero.
    }
}

#[derive(Debug)]
pub struct FractionalScaleManager {
    manager: fractional_scale::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
}

impl FractionalScaleManager {
    pub fn new<State: Dispatch<WpFractionalScaleManagerV1, GlobalData> + 'static>(
        globals: &GlobalList,
        queue_handle: &QueueHandle<State>,
    ) -> Result<Self, BindError> {
        let manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { manager })
    }

    pub fn fractional_scaling<State: Dispatch<WpFractionalScaleV1, FractionalScale> + 'static>(
        &self,
        surface: &WlSurface,
        queue_handle: &QueueHandle<State>,
    ) -> WpFractionalScaleV1 {
        let data = FractionalScale {
            surface: surface.clone(),
        };
        self.manager
            .get_fractional_scale(surface, queue_handle, data)
    }
}

pub struct FractionalScale {
    surface: WlSurface,
}

pub trait FractionalScaleHandler: Sized {
    fn scale_factor_changed(
        &mut self,
        connection: &smithay_client_toolkit::reexports::client::Connection,
        qh: &QueueHandle<Self>,
        surface: &WlSurface,
        factor: ScaleFactor,
    );
}

impl<State> Dispatch<WpFractionalScaleV1, FractionalScale, State> for FractionalScale
where
    State: Dispatch<WpFractionalScaleV1, FractionalScale> + FractionalScaleHandler,
{
    fn event(
        state: &mut State,
        _: &WpFractionalScaleV1,
        event: <WpFractionalScaleV1 as smithay_client_toolkit::reexports::client::Proxy>::Event,
        data: &FractionalScale,
        conn: &smithay_client_toolkit::reexports::client::Connection,
        qhandle: &QueueHandle<State>,
    ) {
        if let fractional_scale::wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            state.scale_factor_changed(conn, qhandle, &data.surface, ScaleFactor(scale));
            return;
        }

        unimplemented!()
    }
}

impl<State> Dispatch<WpFractionalScaleManagerV1, GlobalData, State> for FractionalScaleManager
where
    State: Dispatch<WpFractionalScaleManagerV1, GlobalData> + FractionalScaleHandler,
{
    fn event(
        _: &mut State,
        _: &WpFractionalScaleManagerV1,
        _: <WpFractionalScaleManagerV1 as smithay_client_toolkit::reexports::client::Proxy>::Event,
        _: &GlobalData,
        _: &smithay_client_toolkit::reexports::client::Connection,
        _: &QueueHandle<State>,
    ) {
        unimplemented!("No events for WpFractionalScaleManagerV1")
    }
}

#[macro_export]
macro_rules! delegate_fractional_scale {
    ($(@<$( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+>)? $ty: ty) => {
        smithay_client_toolkit::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1: smithay_client_toolkit::globals::GlobalData
        ] => $crate::wayland::protocol::fractional_scale::FractionalScaleManager);
        smithay_client_toolkit::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1: $crate::wayland::protocol::fractional_scale::FractionalScale
        ] => $crate::wayland::protocol::fractional_scale::FractionalScale);
    };
}
