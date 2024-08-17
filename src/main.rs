#![feature(coroutine_trait, coroutines)]
pub mod app;

use core::ffi;
use std::{
    borrow::BorrowMut,
    ffi::{c_void, CStr},
    marker::PhantomData,
    ops::{ControlFlow, Coroutine},
    pin::Pin,
    process::exit,
    sync::Arc,
    time::Duration,
};

use app::App;

use ash::vk::Handle;
use palette::{rgb::Rgb, FromColor, IntoColor, Mix, Srgb};
use skia_bindings::SkPaint;
use skia_safe::{gpu::vk::GetProcOf, Color, Color4f, Paint, Rect};
use smithay_client_toolkit::{
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_display::WlDisplay, wl_surface::WlSurface},
            Connection, Proxy,
        },
    },
    shell::{
        wlr_layer::{Anchor, Layer},
        WaylandSurface,
    },
};
use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, DeviceExtensions, QueueCreateInfo, QueueFlags,
    },
    image::{view::ImageView, ImageUsage},
    instance::{
        debug::{
            DebugUtilsMessageSeverity, DebugUtilsMessengerCallback, DebugUtilsMessengerCreateInfo,
            ValidationFeatureDisable, ValidationFeatureEnable,
        },
        Instance, InstanceCreateInfo, InstanceExtensions,
    },
    swapchain::{Swapchain, SwapchainCreateInfo, SwapchainPresentInfo},
    sync::{self, GpuFuture},
    Validated, Version, VulkanError, VulkanLibrary, VulkanObject,
};

const INIT_WIDTH: u32 = 1920;
const INIT_HEIGHT: u32 = 60;

type Lch = palette::Lch<palette::white_point::D65, f32>;

fn color_changer(
    colors: &[Lch],
    transition: usize,
) -> impl Coroutine<(), Yield = Lch, Return = ()> + '_ {
    let first = colors.iter().cloned().cycle();
    let second = colors.iter().cloned().cycle().skip(1);

    let pairs = first.zip(second);
    #[coroutine]
    move || {
        for (a, b) in pairs {
            for i in 0..transition {
                let ratio = i as f32 / transition as f32;
                yield a.mix(b, ratio);
            }
        }
    }
}

fn best_physical_device(
    instance: Arc<Instance>,
    surface: Arc<vulkano::swapchain::Surface>,
    device_extensions: &DeviceExtensions,
) -> (Arc<PhysicalDevice>, u32) {
    instance
        .enumerate_physical_devices()
        .expect("could not enumerate devices")
        .filter(|p| p.supported_extensions().contains(&device_extensions))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.contains(QueueFlags::GRAPHICS)
                        && p.surface_support(i as u32, &surface).unwrap_or(false)
                })
                .map(|q| (p, q as u32))
        })
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            _ => 4,
        })
        .expect("no device available")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init::<App>(&conn).unwrap();
    let qh = event_queue.handle();

    let mut app = App::new(&globals, &qh, (INIT_WIDTH, INIT_HEIGHT))?;

    let wl_display = conn.display();
    let wl_surface = app.compositor_state.create_surface(&qh);
    let layer = app.layer_state.create_layer_surface(
        &qh,
        wl_surface.clone(),
        Layer::Top,
        Some("simple_layer"),
        None,
    );

    layer.set_anchor(Anchor::BOTTOM);
    layer.set_size(INIT_WIDTH, INIT_HEIGHT);
    layer.commit();

    event_queue.roundtrip(&mut app).unwrap();
    wl_surface.commit();

    let mut event_loop = EventLoop::<App>::try_new()?;
    WaylandSource::new(conn, event_queue).insert(event_loop.handle())?;

    println!(
        "[WAYLAND] Connected to display, with surface {:?}",
        layer.wl_surface().id()
    );

    // Init Vulkan...
    let instance = {
        let lib = VulkanLibrary::new().expect("[Vulkan] No Vulkan library found.");
        Instance::new(
            lib.clone(),
            InstanceCreateInfo {
                application_name: Some("DEMO".to_string()),
                application_version: Version::major_minor(0, 1),
                engine_name: Some("Avy (Skia)".to_string()),
                engine_version: Version::major_minor(0, 1),
                max_api_version: Some(Version::major_minor(1, 3)),
                enabled_extensions: InstanceExtensions {
                    khr_surface: true,
                    khr_wayland_surface: true,
                    khr_get_surface_capabilities2: true,
                    khr_get_physical_device_properties2: true,
                    ext_debug_utils: false,
                    ..InstanceExtensions::empty()
                },
                // debug_utils_messengers: vec![DebugUtilsMessengerCreateInfo::user_callback(
                //     unsafe {
                //         DebugUtilsMessengerCallback::new(|sev, ty, data| {
                //             println!("[VULKAN] [{ty:?}] [{sev:?}] {}", data.message);

                //             data.objects.for_each(|obj| {
                //                 println!(
                //                     "\t with {:?} @ {:p} {:?}",
                //                     obj.object_type,
                //                     obj.object_handle as *const i8,
                //                     obj.object_name
                //                 )
                //             });
                //         })
                //     },
                // )],
                ..Default::default()
            },
        )
    }?;

    let khr_surface = unsafe {
        vulkano::swapchain::Surface::from_wayland(
            instance.clone(),
            wl_display.id().as_ptr(),
            wl_surface.id().as_ptr(),
            None,
        )
    }?;

    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        ..Default::default()
    };

    let (physical_device, queue_family_i) =
        best_physical_device(instance.clone(), khr_surface.clone(), &device_extensions);

    let (device, mut queues) = Device::new(
        physical_device.clone(),
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index: queue_family_i,
                ..Default::default()
            }],
            enabled_extensions: device_extensions,
            ..Default::default()
        },
    )
    .expect("failed to create device");

    let queue = queues.next().unwrap();

    let caps = physical_device
        .surface_capabilities(&khr_surface, Default::default())
        .expect("failed to get surface capabilities");

    let (width, height) = app.size;
    let (image_format, _) = physical_device
        .surface_formats(&khr_surface, Default::default())
        .into_iter()
        .flatten()
        .find(|(format, _)| &vulkano::format::Format::B8G8R8A8_UNORM == format)
        .expect("Need to support B8G8R8A8 format!");

    let (mut swapchain, mut images) = Swapchain::new(
        device.clone(),
        khr_surface.clone(),
        SwapchainCreateInfo {
            min_image_count: caps.min_image_count + 1,
            image_format,
            image_extent: [width, height],
            image_usage: ImageUsage::COLOR_ATTACHMENT,
            composite_alpha: vulkano::swapchain::CompositeAlpha::PreMultiplied,
            ..Default::default()
        },
    )
    .unwrap();

    let mut image_views: Vec<_> = images
        .iter()
        .cloned()
        .map(ImageView::new_default)
        .collect::<Result<_, _>>()?;

    let instance_c = instance.clone();
    let get_proc = |of: GetProcOf| unsafe {
        println!("{:?}", of.name());
        let res = match of {
            skia_safe::gpu::vk::GetProcOf::Instance(raw_instance, name) => instance
                .library()
                .get_instance_proc_addr(ash::vk::Instance::from_raw(raw_instance as _), name),
            skia_safe::gpu::vk::GetProcOf::Device(device, name) => {
                (instance_c.fns().v1_0.get_device_proc_addr)(
                    ash::vk::Device::from_raw(device as _),
                    name,
                )
            }
        };

        match res {
            Some(f) => f as _,
            None => core::ptr::null(),
        }
    };

    let backend_context = unsafe {
        skia_safe::gpu::vk::BackendContext::new(
            instance.handle().as_raw() as _,
            physical_device.handle().as_raw() as _,
            device.handle().as_raw() as _,
            (queue.handle().as_raw() as _, queue_family_i as _),
            &get_proc,
        )
    };

    let mut gr_context = skia_safe::gpu::direct_contexts::make_vulkan(&backend_context, None)
        .expect("Error creating Skia Vulkan context.");

    let time = std::time::Instant::now();
    let mut frames = 0;

    let (vk_format, color_type) = (
        skia_safe::gpu::vk::Format::B8G8R8A8_UNORM,
        skia_safe::ColorType::BGRA8888,
    );

    let alloc = skia_safe::gpu::vk::Alloc::default();

    let mut previous_frame_end = Some(sync::now(device.clone()).boxed());
    let mut recreate_swapchain = false;

    let fonts = skia_safe::FontMgr::new();
    fonts.family_names().for_each(|a| println!("Font: {a}"));
    let inter = fonts
        .match_family_style("Inter", skia_safe::FontStyle::bold())
        .expect("Inter bold");

    let inter_50pt = skia_safe::Font::from_typeface(inter, Some(50.0));

    let colors: [Lch; 4] = [
        Lch::from_color(Srgb::new(66, 62, 59).into_linear()),
        Lch::from_color(Srgb::new(255, 46, 0).into_linear()),
        Lch::from_color(Srgb::new(254, 168, 47).into_linear()),
        Lch::from_color(Srgb::new(84, 72, 200).into_linear()),
    ];

    let mut rainbow = color_changer(&colors, 60);
    let mut rainbow = Pin::new(&mut rainbow);

    loop {
        event_loop.dispatch(Duration::from_millis(16), &mut app)?;

        if time.elapsed() > Duration::from_secs(10) {
            break;
        }

        if recreate_swapchain {
            (swapchain, images) = {
                let (swapchain, new_images) = swapchain
                    .recreate(SwapchainCreateInfo {
                        image_extent: [width, height],
                        ..swapchain.create_info()
                    })
                    .map_err(|vke| format!("Error re-creating Vulkan swap chain: {vke}"))?;

                image_views = images
                    .iter()
                    .cloned()
                    .map(ImageView::new_default)
                    .collect::<Result<_, _>>()?;

                recreate_swapchain = false;

                (swapchain, new_images)
            }
        }

        let (image_index, suboptimal, acquire_fut) =
            match vulkano::swapchain::acquire_next_image(swapchain.clone(), None)
                .map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(vulkano::VulkanError::OutOfDate) => {
                    recreate_swapchain = true;
                    continue;
                }
                Err(_) => panic!("Failed to acquire next image!"),
            };

        if suboptimal {
            recreate_swapchain = true;
        }

        let extent = swapchain.image_extent();
        let [width, height]: [i32; 2] =
            extent.map(|a| a.try_into().expect("Invalid swapchain image height!"));

        let image_view = image_views.get(image_index as usize).cloned().unwrap();
        let image = image_view.image();

        let image_info = unsafe {
            skia_safe::gpu::vk::ImageInfo::new(
                image.handle().as_raw() as _,
                alloc,
                skia_bindings::VkImageTiling::OPTIMAL,
                skia_bindings::VkImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk_format,
                1,
                None,
                None,
                None,
                None,
            )
        };

        let render_target = &skia_safe::gpu::backend_render_targets::make_vk(
            (width as _, height as _),
            &image_info,
        );

        let mut skia_surface = skia_safe::gpu::surfaces::wrap_backend_render_target(
            &mut gr_context,
            render_target,
            skia_bindings::GrSurfaceOrigin::TopLeft,
            color_type,
            None,
            None,
        )
        .expect("Error creating Skia Vulkan surface.");

        let canvas = skia_surface.canvas();

        canvas.clear(Color4f {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });

        let std::ops::CoroutineState::Yielded(color) = rainbow.as_mut().resume(()) else {
            panic!("Why is it finished?");
        };

        let color: Rgb = color.into_color();
        let (r, g, b) = color.into_format::<u8>().into_components();

        let mut color = Paint::default();
        color.set_color(skia_safe::Color::from_rgb(r, g, b));
        color.set_anti_alias(true);

        canvas.draw_circle((500, 50), 23.0, &color);
        canvas.draw_text_align(
            "Welcome to AvdanOS",
            (1700, 50),
            &inter_50pt,
            &color,
            skia_bindings::SkTextUtils_Align::Right,
        );

        drop(skia_surface);
        // gr_context.flush_and_submit();
        gr_context.flush_submit_and_sync_cpu();
        println!("Drawing!");

        let fut = previous_frame_end
            .borrow_mut()
            .take()
            .unwrap()
            .join(acquire_fut)
            .then_swapchain_present(
                queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(swapchain.clone(), image_index),
            )
            .then_signal_fence_and_flush();

        match fut.map_err(Validated::unwrap) {
            Ok(future) => {
                previous_frame_end = Some(future.boxed());
            }
            Err(VulkanError::OutOfDate) => {
                recreate_swapchain = true;
                previous_frame_end = Some(sync::now(device.clone()).boxed());
            }
            Err(e) => {
                eprintln!("{e:?}");
                previous_frame_end = Some(sync::now(device.clone()).boxed());
            }
        }
        frames += 1;
    }

    println!(
        "Average FPS: {:.2}",
        frames as f64 / time.elapsed().as_secs_f64()
    );

    Ok(())
}
