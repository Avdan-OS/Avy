use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::wayland::protocol::fractional_scale::ScaleFactor;

#[derive(Debug, Clone)]
pub struct Size {
    logical: (u32, u32),
    scale_factor: Option<ScaleFactor>,
    has_changed: Arc<AtomicBool>,
}

impl Size {
    pub fn new(logical_size: (u32, u32)) -> Self {
        Self {
            logical: logical_size,
            scale_factor: None,
            has_changed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn logical_size(&self) -> (u32, u32) {
        self.logical
    }

    ///
    /// Get the scaled size, with respect
    /// to the scale factor set by the compositor.
    ///
    /// (Returns integers, though in float format)
    ///
    pub fn physical_size(&self) -> (f64, f64) {
        let (width, height) = self.logical;
        if let Some(scale) = &self.scale_factor {
            (scale.scale(width), scale.scale(height))
        } else {
            (width as _, height as _)
        }
    }

    pub fn resize(&mut self, logical_size: (u32, u32)) {
        self.logical = logical_size;
        self.has_changed.store(true, Ordering::Relaxed);
    }

    pub fn rescale(&mut self, scale: ScaleFactor) {
        self.scale_factor.replace(scale);
        self.has_changed.store(true, Ordering::Relaxed);
    }

    pub fn handle_changes(&self, mut handler: impl FnMut(&Self)) {
        handler(self);
        self.has_changed.store(false, Ordering::Relaxed);
    }

    ///
    /// Apply scaling transform (if applicable) to Skia canvas.
    ///
    pub fn scale_canvas(&self, canvas: &skia_safe::Canvas) {
        if let Some(scale) = &self.scale_factor {
            let factor = scale.as_f64() as f32;
            canvas.scale((factor, factor));
        }
    }
}
