pub mod app;
pub mod debugging;

use std::{
    borrow::BorrowMut,
    cell::{Ref, RefCell, RefMut},
    convert::identity,
    ffi::{c_void, CStr},
    marker::PhantomData,
    ops::ControlFlow,
    process::exit,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
    u64,
};

use app::App;
use ash::{
    ext::debug_utils,
    khr::{get_physical_device_properties2, surface, wayland_surface},
    vk::{
        self, ApplicationInfo, Handle, InstanceCreateInfo, QueueFlags, StructureType,
        WaylandSurfaceCreateInfoKHR, API_VERSION_1_1, API_VERSION_1_3,
    },
};
use debugging::vulkan_debug_callback;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_output, delegate_registry,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_display::WlDisplay, wl_surface::WlSurface},
            Connection, EventQueue, Proxy, QueueHandle,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    shell::{
        wlr_layer::{Anchor, Layer},
        xdg::XdgSurface,
        WaylandSurface,
    },
};

const INIT_WIDTH: u32 = 1920;
const INIT_HEIGHT: u32 = 60;

const LAYER_NAMES: [&CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];
const LAYER_NAMES_PTR: &[*const i8] = &[LAYER_NAMES[0].as_ptr()];
const DEVICE_EXTENSION_NAMES: [&CStr; 1] = [c"VK_KHR_swapchain"];
const DEVICE_EXTENSION_NAMES_PTR: &[*const i8] = &[DEVICE_EXTENSION_NAMES[0].as_ptr()];

struct SwapchainElement {
    command_buffer: ash::vk::CommandBuffer,
    image: ash::vk::Image,
    image_view: ash::vk::ImageView,
    framebuffer: ash::vk::Framebuffer,
    start_semaphore: ash::vk::Semaphore,
    end_semaphore: ash::vk::Semaphore,
    fence: ash::vk::Fence,
    last_fence: ash::vk::Fence,
}

impl SwapchainElement {
    unsafe fn new(
        device: &ash::Device,
        format: ash::vk::Format,
        render_pass: ash::vk::RenderPass,
        image: ash::vk::Image,
        command_buf: ash::vk::CommandBuffer,
        (width, height): (u32, u32),
    ) -> ash::prelude::VkResult<Self> {
        let image_view = device.create_image_view(
            &ash::vk::ImageViewCreateInfo::default()
                .view_type(ash::vk::ImageViewType::TYPE_2D)
                .components(ash::vk::ComponentMapping {
                    r: ash::vk::ComponentSwizzle::IDENTITY,
                    g: ash::vk::ComponentSwizzle::IDENTITY,
                    b: ash::vk::ComponentSwizzle::IDENTITY,
                    a: ash::vk::ComponentSwizzle::IDENTITY,
                })
                .subresource_range(
                    ash::vk::ImageSubresourceRange::default()
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1)
                        .aspect_mask(ash::vk::ImageAspectFlags::COLOR),
                )
                .image(image)
                .format(format),
            None,
        )?;

        let framebuffer = device.create_framebuffer(
            &ash::vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachment_count(1)
                .attachments(&[image_view])
                .width(width)
                .height(height)
                .layers(1),
            None,
        )?;

        let start_semaphore =
            device.create_semaphore(&ash::vk::SemaphoreCreateInfo::default(), None)?;
        let end_semaphore =
            device.create_semaphore(&ash::vk::SemaphoreCreateInfo::default(), None)?;

        let fence = device.create_fence(
            &ash::vk::FenceCreateInfo::default().flags(ash::vk::FenceCreateFlags::SIGNALED),
            None,
        )?;

        let last_fence = ash::vk::Fence::null();

        Ok(Self {
            command_buffer: command_buf,
            image,
            image_view,
            framebuffer,
            start_semaphore,
            end_semaphore,
            fence,
            last_fence,
        })
    }

    unsafe fn destroy(&mut self, device: &ash::Device, command_pool: ash::vk::CommandPool) {
        let Self {
            fence,
            end_semaphore,
            start_semaphore,
            framebuffer,
            image_view,
            command_buffer,
            ..
        } = *self;
        device.destroy_fence(self.fence, None);
        device.destroy_semaphore(end_semaphore, None);
        device.destroy_semaphore(start_semaphore, None);
        device.destroy_framebuffer(framebuffer, None);
        device.destroy_image_view(image_view, None);
        device.free_command_buffers(command_pool, &[command_buffer]);
    }
}

struct AvySwapchain<'a> {
    instance: &'a ash::Instance,
    physical_device: &'a ash::vk::PhysicalDevice,
    device: &'a ash::Device,
    s_device: ash::khr::swapchain::Device,
    vk_surface_instance: &'a ash::khr::surface::Instance,
    surface: &'a ash::vk::SurfaceKHR,
    command_pool: &'a ash::vk::CommandPool,
    swapchain: ash::vk::SwapchainKHR,
    render_pass: ash::vk::RenderPass,
    elements: Vec<SwapchainElement>,
    current_frame: usize,
    size: (u32, u32),
}

impl<'a> AvySwapchain<'a> {
    unsafe fn new(
        instance: &'a ash::Instance,
        physical_device: &'a ash::vk::PhysicalDevice,
        device: &'a ash::Device,
        vk_surface_instance: &'a ash::khr::surface::Instance,
        surface: &'a ash::vk::SurfaceKHR,
        command_pool: &'a ash::vk::CommandPool,
        (width, height): (u32, u32),
    ) -> ash::prelude::VkResult<Self> {
        let capabilities = vk_surface_instance
            .get_physical_device_surface_capabilities(*physical_device, *surface)?;

        let surface_formats =
            vk_surface_instance.get_physical_device_surface_formats(*physical_device, *surface)?;

        let ash::vk::SurfaceFormatKHR {
            format,
            color_space,
        } = surface_formats
            .iter()
            .find(|format| format.format == ash::vk::Format::B8G8R8A8_UNORM)
            .unwrap_or(surface_formats.first().unwrap());

        let image_count = if (capabilities.min_image_count + 1) < capabilities.max_image_count {
            capabilities.min_image_count + 1
        } else {
            capabilities.min_image_count
        };

        let s_device = ash::khr::swapchain::Device::new(instance, device);
        let swapchain = s_device.create_swapchain(
            &ash::vk::SwapchainCreateInfoKHR::default()
                .surface(*surface)
                .min_image_count(image_count)
                .image_format(*format)
                .image_color_space(*color_space)
                .image_extent(ash::vk::Extent2D { width, height })
                .image_array_layers(1)
                .image_usage(ash::vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(ash::vk::SharingMode::EXCLUSIVE)
                .pre_transform(capabilities.current_transform)
                .composite_alpha(ash::vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED)
                .present_mode(ash::vk::PresentModeKHR::MAILBOX)
                .clipped(true),
            None,
        )?;

        let attachment = ash::vk::AttachmentDescription::default()
            .format(*format)
            .samples(ash::vk::SampleCountFlags::TYPE_1)
            .load_op(ash::vk::AttachmentLoadOp::CLEAR)
            .store_op(ash::vk::AttachmentStoreOp::STORE)
            .initial_layout(ash::vk::ImageLayout::UNDEFINED)
            .final_layout(ash::vk::ImageLayout::PRESENT_SRC_KHR);

        let attachment_refs = [ash::vk::AttachmentReference::default()
            .attachment(0)
            .layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

        let subpass = ash::vk::SubpassDescription::default()
            .pipeline_bind_point(ash::vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&attachment_refs);

        let render_pass = device.create_render_pass(
            &ash::vk::RenderPassCreateInfo::default()
                .flags(ash::vk::RenderPassCreateFlags::empty())
                .attachments(&[attachment])
                .subpasses(&[subpass]),
            None,
        )?;

        let images = s_device.get_swapchain_images(swapchain)?;

        // Images (image) <-1:1-> Element { image, image_view, frame_buffer, start_semaphore, end_semaphore, fence }
        let command_bufs = device.allocate_command_buffers(
            &ash::vk::CommandBufferAllocateInfo::default()
                .command_pool(*command_pool)
                .command_buffer_count(images.len() as u32)
                .level(ash::vk::CommandBufferLevel::PRIMARY),
        )?;

        let elements = images
            .into_iter()
            .zip(command_bufs)
            .map(|(image, command_buf)| {
                SwapchainElement::new(
                    device,
                    *format,
                    render_pass,
                    image,
                    command_buf,
                    (width, height),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            instance,
            physical_device,
            device,
            s_device,
            surface,
            command_pool,
            swapchain,
            render_pass,
            elements,
            current_frame: 0,
            size: (width, height),
            vk_surface_instance,
        })
    }

    unsafe fn destroy(&mut self) {
        self.elements
            .iter_mut()
            .for_each(|el| el.destroy(self.device, *self.command_pool));

        self.device.destroy_render_pass(self.render_pass, None);

        self.s_device.destroy_swapchain(self.swapchain, None);
    }

    unsafe fn recreate(&mut self) -> ash::prelude::VkResult<()> {
        self.device.device_wait_idle()?;
        self.destroy();
        *self = Self::new(
            self.instance,
            self.physical_device,
            self.device,
            self.vk_surface_instance,
            self.surface,
            self.command_pool,
            self.size,
        )?;

        Ok(())
    }

    unsafe fn resize(
        &mut self,
        new_size: (u32, u32),
        surface: &WlSurface,
    ) -> ash::prelude::VkResult<()> {
        self.size = new_size;
        self.recreate()?;
        surface.commit();

        Ok(())
    }

    fn current_element(&self) -> &SwapchainElement {
        self.elements.get(self.current_frame).unwrap()
    }

    fn current_element_mut(&mut self) -> &mut SwapchainElement {
        self.elements.get_mut(self.current_frame).unwrap()
    }

    unsafe fn next_element(&mut self) -> ash::prelude::VkResult<ControlFlow<(), (usize, usize)>> {
        let device = self.device;
        let s_device = &self.s_device;
        let current = self.current_element();

        device.wait_for_fences(&[current.fence], true, u64::MAX)?;

        let image_index = match s_device.acquire_next_image(
            self.swapchain,
            u64::MAX,
            current.start_semaphore,
            ash::vk::Fence::null(),
        ) {
            Ok((idx, false)) => idx as usize,
            Ok((_, true))
            | Err(ash::vk::Result::ERROR_OUT_OF_DATE_KHR | ash::vk::Result::SUBOPTIMAL_KHR) => {
                self.recreate()?;
                return Ok(ControlFlow::Break(()));
            }
            Err(err) => return Err(err),
        };

        let element = (&self.elements[image_index] as *const SwapchainElement
            as *mut SwapchainElement)
            .as_mut()
            .unwrap();

        if !element.last_fence.is_null() {
            device.wait_for_fences(&[element.last_fence], true, u64::MAX)?;
        }

        element.last_fence = current.fence;
        self.device.reset_fences(&[current.fence])?;

        Ok(ControlFlow::Continue((self.current_frame, image_index)))
    }

    unsafe fn fill_frame(
        &mut self,
        queue: ash::vk::Queue,
        clear_value: ash::vk::ClearValue,
        wl_surface: &WlSurface,
        app: &mut App,
    ) -> ash::prelude::VkResult<ControlFlow<(), ()>> {
        if app.changed_size {
            app.changed_size = false;
            self.resize(app.size, wl_surface)?;
            return Ok(ControlFlow::Break(()));
        }

        let (current, element) = match self.next_element()? {
            ControlFlow::Continue(el) => el,
            ControlFlow::Break(()) => return Ok(ControlFlow::Break(())),
        };

        let image_index = element as u32;

        let (current, element) = (&self.elements[current], &self.elements[element]);


        self.device.begin_command_buffer(
            element.command_buffer,
            &ash::vk::CommandBufferBeginInfo::default()
                .flags(ash::vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )?;

        self.device.cmd_begin_render_pass(
            element.command_buffer,
            &ash::vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(element.framebuffer)
                .render_area(
                    ash::vk::Rect2D::default()
                        .offset(ash::vk::Offset2D::default().x(0).y(0))
                        .extent(
                            ash::vk::Extent2D::default()
                                .width(self.size.0)
                                .height(self.size.1),
                        ),
                )
                .clear_values(&[clear_value]),
            ash::vk::SubpassContents::INLINE,
        );

        self.device.cmd_end_render_pass(element.command_buffer);

        self.device.end_command_buffer(element.command_buffer)?;

        self.device.queue_submit(
            queue,
            &[ash::vk::SubmitInfo::default()
                .wait_semaphores(&[current.start_semaphore])
                .wait_dst_stage_mask(&[ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .command_buffers(&[element.command_buffer])
                .signal_semaphores(&[current.end_semaphore])],
            current.fence,
        )?;

        match self.s_device.queue_present(
            queue,
            &ash::vk::PresentInfoKHR::default()
                .wait_semaphores(&[current.end_semaphore])
                .swapchains(&[self.swapchain])
                .image_indices(&[image_index]),
        ) {
            Ok(false) => (),
            Err(ash::vk::Result::ERROR_OUT_OF_DATE_KHR | ash::vk::Result::SUBOPTIMAL_KHR)
            | Ok(true) => {
                self.recreate()?;
                return Ok(ControlFlow::Break(()));
            }
            Err(err) => return Err(err),
        };

        self.current_frame = (self.current_frame + 1) % self.elements.len();

        Ok(ControlFlow::Continue(()))
    }
}

#[inline]
fn version_as_string(v: u32) -> String {
    let [maj, min, patch] = [
        ash::vk::api_version_major(v),
        ash::vk::api_version_minor(v),
        ash::vk::api_version_patch(v),
    ];

    format!("{maj}.{min}.{patch}")
}

unsafe fn as_mut_void<'b, 'o: 'b, T: 'o>(t: &'b mut T) -> &'b mut c_void {
    (t as *mut T as *mut c_void).as_mut().unwrap()
}

struct Debugger<'a> {
    instance: debug_utils::Instance,
    messenger: ash::vk::DebugUtilsMessengerEXT,
    __marker: PhantomData<&'a ()>,
}

impl<'a> Debugger<'a> {
    unsafe fn new<'entry: 'a>(
        entry: &'entry ash::Entry,
        instance: &'a ash::Instance,
    ) -> ash::prelude::VkResult<Self> {
        let instance = debug_utils::Instance::new(entry, instance);

        let messenger = instance
            .create_debug_utils_messenger(
                &vk::DebugUtilsMessengerCreateInfoEXT::default()
                    .message_severity(
                        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                            | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                            | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
                    )
                    .message_type(
                        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                    )
                    .pfn_user_callback(Some(vulkan_debug_callback)),
                None,
            )
            .unwrap();

        Ok(Self {
            instance,
            messenger,
            __marker: PhantomData,
        })
    }

    unsafe fn destroy(&mut self) {
        self.instance
            .destroy_debug_utils_messenger(self.messenger, None);
    }
}

unsafe fn best_physical_device(instance: &ash::Instance) -> Option<ash::vk::PhysicalDevice> {
    let devices = instance.enumerate_physical_devices().map_or_else(
        |err| {
            println!("Error whilst querying physical devices: {err:?}");
            None
        },
        Some,
    )?;

    devices
        .iter()
        .max_by_key(|dev| {
            let props = instance.get_physical_device_properties(**dev);
            match props.device_type {
                ash::vk::PhysicalDeviceType::OTHER => 1,
                ash::vk::PhysicalDeviceType::INTEGRATED_GPU => 4,
                ash::vk::PhysicalDeviceType::DISCRETE_GPU => 5,
                ash::vk::PhysicalDeviceType::VIRTUAL_GPU => 3,
                ash::vk::PhysicalDeviceType::CPU => 2,
                _ => 0,
            }
        })
        .copied()
}

unsafe fn init_vulkan_wayland<'a>(
    entry: &ash::Entry,
    instance: &ash::Instance,
    wl_display: &'a WlDisplay,
    wl_surface: &'a WlSurface,
) -> ash::prelude::VkResult<(
    ash::khr::surface::Instance,
    ash::khr::wayland_surface::Instance,
    ash::vk::SurfaceKHR,
)> {
    let vk_surface_instance = ash::khr::surface::Instance::new(entry, instance);
    let vk_wl_instance = ash::khr::wayland_surface::Instance::new(entry, instance);

    let create_info = ash::vk::WaylandSurfaceCreateInfoKHR::default()
        .display(wl_display.id().as_ptr() as *mut c_void)
        .surface(wl_surface.id().as_ptr() as *mut c_void);

    println!("\tCreating KHR surface from wayland!");
    println!("\t\twith wl_display: {wl_display:p}");
    println!("\t\twith wl_surface: {wl_surface:p}");

    vk_wl_instance
        .create_wayland_surface(&create_info, None)
        .map(|surface| (vk_surface_instance, vk_wl_instance, surface))
}

unsafe fn supports_all_layers(
    instance: &ash::Instance,
    physical_device: ash::vk::PhysicalDevice,
) -> bool {
    let layer_props = instance
        .enumerate_device_layer_properties(physical_device)
        .expect("Get layer properties of device");

    let supported_layer_names = layer_props
        .iter()
        .map(|layer| layer.layer_name_as_c_str().unwrap())
        .collect::<Vec<_>>();

    LAYER_NAMES
        .iter()
        .all(|required_layer| supported_layer_names.contains(required_layer))
}

unsafe fn create_device(
    entry: &ash::Entry,
    instance: &ash::Instance,
    vk_surface_instance: &ash::khr::surface::Instance,
    surface: ash::vk::SurfaceKHR,
    physical_device: ash::vk::PhysicalDevice,
) -> Option<(ash::Device, u32, ash::vk::Queue)> {
    // Get index of a surface-supporting queue family.
    let queue_family_i = instance
        .get_physical_device_queue_family_properties(physical_device)
        .iter()
        .enumerate()
        .filter_map(|(i, queue_family)| {
            let supports_surface = vk_surface_instance
                .get_physical_device_surface_support(physical_device, i as u32, surface)
                .unwrap();
            (supports_surface
                && queue_family
                    .queue_flags
                    .contains(ash::vk::QueueFlags::GRAPHICS))
            .then_some(i as u32)
        })
        .next()?;

    let device_queue_infos = [ash::vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_i)
        .queue_priorities(&[1.0])];

    let mut create_info = ash::vk::DeviceCreateInfo::default()
        .queue_create_infos(&device_queue_infos)
        .enabled_extension_names(DEVICE_EXTENSION_NAMES_PTR);

    #[allow(deprecated)]
    if supports_all_layers(instance, physical_device) {
        create_info = create_info.enabled_layer_names(LAYER_NAMES_PTR);
    }

    let device = instance
        .create_device(physical_device, &create_info, None)
        .expect("Create Vulkan device");

    let queue = device.get_device_queue(queue_family_i, 0);

    Some((device, queue_family_i, queue))
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
    let entry = unsafe { ash::Entry::load()? };
    println!("Creating Vulkan instance...");
    let instance = unsafe {
        entry.create_instance(
            &InstanceCreateInfo::default()
                .application_info(
                    &ApplicationInfo::default()
                        .api_version(API_VERSION_1_1)
                        .application_name(c"AvTest")
                        .engine_name(c"Avy (Skia)"),
                )
                .enabled_extension_names(&[
                    debug_utils::NAME.as_ptr(),
                    surface::NAME.as_ptr(),
                    wayland_surface::NAME.as_ptr(),
                ])
                .enabled_layer_names(&LAYER_NAMES.map(CStr::as_ptr)),
            None,
        )
    }?;
    let mut debugger = unsafe { Debugger::new(&entry, &instance) }?;

    println!("\tDone!");

    println!("Selecting best physical device...");
    let physical_device = unsafe { best_physical_device(&instance) }.unwrap_or_else(|| exit(1));
    println!("\tDone");

    println!("Integrating with Wayland...");
    let (khr_surface_instance, _, khr_surface) =
        unsafe { init_vulkan_wayland(&entry, &instance, &wl_display, &wl_surface) }?;
    println!("\tDone!");

    println!("Making device!");
    let (device, queue_family_i, queue) = unsafe {
        create_device(
            &entry,
            &instance,
            &khr_surface_instance,
            khr_surface,
            physical_device,
        )
    }
    .expect("Create vulkan device");
    println!("Created vulkan device!");

    let command_pool = unsafe {
        device.create_command_pool(
            &ash::vk::CommandPoolCreateInfo::default()
                .queue_family_index(queue_family_i)
                .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
            None,
        )
    }?;

    let mut swapchain = unsafe {
        AvySwapchain::new(
            &instance,
            &physical_device,
            &device,
            &khr_surface_instance,
            &khr_surface,
            &command_pool,
            (INIT_WIDTH, INIT_HEIGHT),
        )
    }?;

    let time = std::time::Instant::now();
    let mut frames = 0;

    loop {
        event_loop.dispatch(Duration::from_millis(16), &mut app)?;

        if time.elapsed() > Duration::from_secs(10) {
            break;
        }

        let action = unsafe {
            swapchain.fill_frame(
                queue,
                ash::vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [1.0, 0.0, 1.0, 1.0]
                    },
                },
                &wl_surface,
                &mut app,
            )
        }?;

        match action {
            ControlFlow::Continue(()) => (),
            ControlFlow::Break(()) => continue,
        }

        frames += 1;
    }

    println!(
        "Average FPS: {:.2}",
        frames as f64 / time.elapsed().as_secs_f64()
    );

    unsafe {
        device.device_wait_idle()?;
        swapchain.destroy();
        device.destroy_command_pool(command_pool, None);
        device.destroy_device(None);
        khr_surface_instance.destroy_surface(khr_surface, None);
        debugger.destroy();

        drop(wl_surface);
        drop(wl_display);
    }

    Ok(())
}
