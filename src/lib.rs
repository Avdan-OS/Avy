#![feature(slice_as_chunks)]

pub mod app;
pub mod util;
pub mod wayland;
pub mod graphics;

pub use app::AvyClient;
use vulkano::Version;

pub const ENGINE_NAME: &str = "Avy (Skia)";
pub const ENGINE_VERSION: Version = Version::major_minor(0, 1);