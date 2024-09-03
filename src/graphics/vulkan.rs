//!
//! Support for Vulkan using `vulkano` (for now).
//!

use std::{any::Any, borrow::BorrowMut, sync::Arc};

use skia_bindings::{GrDirectContext, SkSurface};
use skia_safe::{gpu::vk::GetProcOf, Color4f};
use smithay_client_toolkit::reexports::client::{protocol::wl_display::WlDisplay, Proxy};
use thiserror::Error;
use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo, QueueFlags,
    },
    image::{view::ImageView, Image, ImageUsage},
    instance::{Instance, InstanceCreateInfo, InstanceExtensions},
    swapchain::{Swapchain, SwapchainCreateInfo, SwapchainPresentInfo},
    sync::{self, GpuFuture},
    Handle, LoadingError, Validated, Version, VulkanError, VulkanLibrary, VulkanObject,
};

pub const MAX_VK_API_VERSION: Version = Version::major_minor(1, 3);

use crate::{
    impl_as_any,
    util::{AsAny, Size},
    wayland::surface::AvySurface,
};

use super::{GraphicsBackend, GraphicsSurface};

#[derive(Debug, Error)]
pub enum Error {
    #[error("An error has occurred whilst loading a Vulkan library: {0}")]
    Loading(#[from] LoadingError),

    #[error("A Vulkan error has occurred: {0}")]
    Validated(#[from] Validated<VulkanError>),

    #[error("A Vulkan error has occurred: {0}")]
    Vulkan(#[from] VulkanError),

    #[error("Your graphics device does not support B8G8R8A8 format.")]
    UnsupportedBGRA,

    #[error("An error occurred whilst creating a Skia context for Vulkan.")]
    SkiaCreationError,

    #[error("An error occurred whilst creating a Skia surface for Vulkan.")]
    SkiaSurfaceError,
}

impl_as_any!(Error);

pub struct Vulkan {
    instance: Arc<Instance>,
}

impl Vulkan {
    pub fn new(
        application_name: impl ToString,
        application_version: Version,
    ) -> Result<Self, Error> {
        let lib = VulkanLibrary::new().expect("[Vulkan] No Vulkan library found.");
        let instance = Instance::new(
            lib.clone(),
            InstanceCreateInfo {
                application_name: Some(application_name.to_string()),
                application_version,
                engine_name: Some(crate::ENGINE_NAME.to_string()),
                engine_version: crate::ENGINE_VERSION,
                max_api_version: Some(MAX_VK_API_VERSION),
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
        )?;

        Ok(Self { instance })
    }
}

impl GraphicsBackend for Vulkan {
    type Surface = VulkanSurface;
    type Error = Error;

    fn for_surface(
        &self,
        wl_display: &WlDisplay,
        surface: &(impl AvySurface + ?Sized),
    ) -> Result<Self::Surface, Self::Error> {
        let instance = self.instance.clone();

        // Create KHR Surface, which supports Wayland surfaces.
        let khr_surface = unsafe {
            vulkano::swapchain::Surface::from_wayland(
                instance.clone(),
                wl_display.id().as_ptr(),
                surface.wl_surface().id().as_ptr(),
                None,
            )
        }?;

        // Get our Vulkan Device
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
        )?;

        let queue = queues.next().unwrap();

        // Create our Swapchain.
        let capabilities =
            physical_device.surface_capabilities(&khr_surface, Default::default())?;

        let (image_format, _) = physical_device
            .surface_formats(&khr_surface, Default::default())
            .into_iter()
            .flatten()
            .find(|(format, _)| &vulkano::format::Format::B8G8R8A8_UNORM == format)
            .ok_or(Error::UnsupportedBGRA)?;

        let (width, height) = surface.size_ref().physical_size();
        let (width, height) = (width as u32, height as u32);

        let (swapchain, images) = Swapchain::new(
            device.clone(),
            khr_surface.clone(),
            SwapchainCreateInfo {
                min_image_count: capabilities.min_image_count + 1,
                image_format,
                image_extent: [width, height],
                image_usage: ImageUsage::COLOR_ATTACHMENT,
                composite_alpha: vulkano::swapchain::CompositeAlpha::PreMultiplied,
                ..Default::default()
            },
        )?;

        let image_views: Vec<_> = images
            .iter()
            .cloned()
            .map(ImageView::new_default)
            .collect::<Result<_, _>>()?;

        // Create Skia Backend
        let instance_for_get_proc = instance.clone();
        let get_proc = |of: GetProcOf| unsafe {
            let res = match of {
                skia_safe::gpu::vk::GetProcOf::Instance(raw_instance, name) => instance
                    .library()
                    .get_instance_proc_addr(ash::vk::Instance::from_raw(raw_instance as _), name),
                skia_safe::gpu::vk::GetProcOf::Device(device, name) => {
                    (instance_for_get_proc.fns().v1_0.get_device_proc_addr)(
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

        let gr_context = skia_safe::gpu::direct_contexts::make_vulkan(&backend_context, None)
            .ok_or(Error::SkiaCreationError)?;

        Ok(VulkanSurface {
            device: device.clone(),
            queue,
            swapchain,
            images,
            image_views,
            recreate_swapchain: false,
            previous_frame_end: Some(Box::new(sync::now(device))),
            gr_context,
        })
    }
}

pub struct VulkanSurface {
    recreate_swapchain: bool,
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    gr_context: skia_safe::RCHandle<GrDirectContext>,
    image_views: Vec<Arc<ImageView>>,
    images: Vec<Arc<Image>>,
    swapchain: Arc<Swapchain>,
    queue: Arc<Queue>,
    device: Arc<Device>,
}

///
/// SAFETY: Nobody except us can access the gr_context for this surface.
/// Everything else is Send-able
///
unsafe impl Send for VulkanSurface {}

impl GraphicsSurface for VulkanSurface {
    fn render(
        &mut self,
        size: &Size,
        callback: &mut dyn FnMut(&skia_safe::Canvas),
    ) -> Result<(), Box<dyn Any>> {
        size.handle_changes(|_| {
            self.recreate_swapchain = true;
        });

        if self.recreate_swapchain {
            self.recreate_swapchain(size)
                .map_err(Box::new)
                .map_err(AsAny::as_any)?;
        }

        let (image_index, suboptimal, acquire_fut) =
            match vulkano::swapchain::acquire_next_image(self.swapchain.clone(), None)
                .map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(vulkano::VulkanError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return Ok(());
                }
                Err(err) => return Err(Box::new(Error::from(err)).as_any()),
            };

        if suboptimal {
            // Recreate swapchain next frame.
            self.recreate_swapchain = true;
        }

        let image_view = self.image_views.get(image_index as usize).cloned().unwrap();
        let image = image_view.image();

        let mut skia = self
            .skia_surface(image, size)
            .map_err(Box::new)
            .map_err(AsAny::as_any)?;
        let canvas = skia.canvas();

        // Apply fractional scaling (if necessary).
        size.scale_canvas(canvas);

        canvas.clear(Color4f {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });

        callback(canvas);

        drop(skia);

        self.gr_context.flush_submit_and_sync_cpu();

        let fut = self
            .previous_frame_end
            .borrow_mut()
            .take()
            .unwrap()
            .join(acquire_fut)
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_index),
            )
            .then_signal_fence_and_flush();

        match fut.map_err(Validated::unwrap) {
            Ok(future) => {
                self.previous_frame_end = Some(future.boxed());
            }
            Err(VulkanError::OutOfDate) => {
                self.recreate_swapchain = true;
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
            Err(err) => {
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
                return Err(Box::new(Error::from(err)).as_any());
            }
        }

        Ok(())
    }
}

impl VulkanSurface {
    pub fn recreate_swapchain(&mut self, size: &Size) -> Result<(), Error> {
        let (width, height) = size.physical_size();
        let (width, height) = (width as u32, height as u32);

        let (new_swapchain, new_images) = self.swapchain.recreate(SwapchainCreateInfo {
            image_extent: [width, height],
            ..self.swapchain.create_info()
        })?;

        self.image_views = new_images
            .iter()
            .cloned()
            .map(ImageView::new_default)
            .collect::<Result<_, _>>()?;

        self.swapchain = new_swapchain;
        self.images = new_images;

        self.recreate_swapchain = false;

        Ok(())
    }

    pub fn skia_surface(
        &mut self,
        image: &Arc<Image>,
        size: &Size,
    ) -> Result<skia_safe::RCHandle<SkSurface>, Error> {
        let image_info = unsafe {
            skia_safe::gpu::vk::ImageInfo::new(
                image.handle().as_raw() as _,
                Default::default(),
                skia_bindings::VkImageTiling::OPTIMAL,
                skia_bindings::VkImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                skia_safe::gpu::vk::Format::B8G8R8A8_UNORM,
                1,
                None,
                None,
                None,
                None,
            )
        };

        let (width, height) = size.physical_size();
        let (width, height) = (width as i32, height as i32);
        let render_target =
            &skia_safe::gpu::backend_render_targets::make_vk((width, height), &image_info);

        skia_safe::gpu::surfaces::wrap_backend_render_target(
            &mut self.gr_context,
            render_target,
            skia_bindings::GrSurfaceOrigin::TopLeft,
            skia_safe::ColorType::BGRA8888,
            None,
            None,
        )
        .ok_or(Error::SkiaSurfaceError)
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
        .filter(|p| p.supported_extensions().contains(device_extensions))
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
