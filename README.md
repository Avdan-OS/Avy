## Skia + Vulkan + Wlr Layer Fun.

* Uses the [wlr-layer-shell-unstable-v1](https://wayland.app/protocols/wlr-layer-shell-unstable-v1) extension protocol.
* Uses the omnipresent [Skia](https://skia.org) for graphics.
* Supports fractional scaling for HiDPI screens

https://github.com/user-attachments/assets/b0c58fef-a8c9-46af-9390-51aeb1b92524

* Heavily based on the work of [Amini Allight](https://gitlab.com/amini-allight/wayland-vulkan-example), thank you!
* Implements most of the boilerplate necessary to get off the ground and use the [VK_KHR_wayland_surface](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VK_KHR_wayland_surface.html) Vulkan extension.
* Did I mention it's written in Rust?!

Thanks to [`ash-rs`](https://github.com/ash-rs) and [`vulkano`](https://github.com/vulkano-rs/vulkano), and the people at [`Smithay`](https://github.com/Smithay/) for making Rust bindings to Vulkan, and Wayland, respectively.

