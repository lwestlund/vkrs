use super::swapchain;
use super::validation;
use super::vulkan;

use ash::vk;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

const VERSION_MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");
const VERSION_MINOR: &str = env!("CARGO_PKG_VERSION_MINOR");
const VERSION_PATCH: &str = env!("CARGO_PKG_VERSION_PATCH");

pub struct App {
    _entry: ash::Entry,
    instance: ash::Instance,
    debug_utils_loader: ash::extensions::ext::DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    surface_fn: ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
    _physical_device: vk::PhysicalDevice,
    device: ash::Device,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    swapchain_fn: ash::extensions::khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    _swapchain_image_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    _swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    graphics_pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    swapchain_framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,
    in_flight_fence: vk::Fence,
}

impl App {
    pub fn new(name: &'static str, window: &winit::window::Window) -> Self {
        let version_major = VERSION_MAJOR.parse().unwrap();
        let version_minor = VERSION_MINOR.parse().unwrap();
        let version_patch = VERSION_PATCH.parse().unwrap();

        let entry = unsafe { ash::Entry::load().expect("Failed to load Vulkan.") };

        let version = vk::make_api_version(0, version_major, version_minor, version_patch);

        let instance = vulkan::create_instance(name, version, &entry, window);
        let (debug_utils_loader, debug_messenger) =
            vulkan::setup_debug_messenger(&entry, &instance);
        let surface_fn = ash::extensions::khr::Surface::new(&entry, &instance);
        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, window, None)
                .expect("Failed to create surface")
        };
        let physical_device = vulkan::select_physical_device(&instance, &surface_fn, surface);
        let (device, graphics_queue, present_queue) =
            vulkan::create_logical_device_with_graphics_and_present_queue(
                &instance,
                &surface_fn,
                surface,
                physical_device,
            );
        let (swapchain_fn, swapchain, swapchain_image_format, swapchain_extent, swapchain_images) =
            swapchain::create_swapchain_and_images(
                &instance,
                physical_device,
                &device,
                &surface_fn,
                surface,
                &window.inner_size(),
            );
        let swapchain_image_views =
            swapchain::create_image_views(&device, &swapchain_images, swapchain_image_format);

        let render_pass = vulkan::create_render_pass(&device, swapchain_image_format);
        let (graphics_pipeline, pipeline_layout) =
            vulkan::create_graphics_pipeline(&device, swapchain_extent, render_pass);

        let swapchain_framebuffers = vulkan::create_framebuffers(
            &device,
            &swapchain_image_views,
            render_pass,
            swapchain_extent,
        );

        let command_pool =
            vulkan::create_command_pool(&device, &instance, &surface_fn, surface, physical_device);
        let command_buffers = vulkan::create_command_buffers(&device, command_pool);

        let (image_available_semaphore, render_finished_semaphore, in_flight_fence) =
            vulkan::create_sync_objects(&device);

        Self {
            _entry: entry,
            instance,
            debug_utils_loader,
            surface_fn,
            surface,
            debug_messenger,
            _physical_device: physical_device,
            device,
            graphics_queue,
            present_queue,
            swapchain_fn,
            swapchain,
            _swapchain_image_format: swapchain_image_format,
            swapchain_extent,
            _swapchain_images: swapchain_images,
            swapchain_image_views,
            render_pass,
            graphics_pipeline,
            pipeline_layout,
            swapchain_framebuffers,
            command_pool,
            command_buffers,
            image_available_semaphore,
            render_finished_semaphore,
            in_flight_fence,
        }
    }

    fn draw_frame(&self) {
        unsafe {
            let fences = [self.in_flight_fence];
            self.device
                .wait_for_fences(&fences, true, u64::MAX)
                .unwrap();
            self.device.reset_fences(&fences).unwrap();
        };

        let image_index = unsafe {
            self.swapchain_fn
                .acquire_next_image(
                    self.swapchain,
                    u64::MAX,
                    self.image_available_semaphore,
                    vk::Fence::null(),
                )
                .unwrap()
                .0
        };

        let command_buffer = self.command_buffers[0];
        let frame_buffer = self.swapchain_framebuffers[image_index as usize];
        unsafe {
            self.device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap()
        };
        vulkan::record_command_buffer(
            &self.device,
            command_buffer,
            self.render_pass,
            frame_buffer,
            self.swapchain_extent,
            self.graphics_pipeline,
        );

        let wait_semaphores = [self.image_available_semaphore];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = [command_buffer];
        let signal_semaphores = [self.render_finished_semaphore];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores)
            .build();
        let submit_infos = [submit_info];
        unsafe {
            self.device
                .queue_submit(self.graphics_queue, &submit_infos, self.in_flight_fence)
                .expect("Failed to submit draw command buffer.")
        };

        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        unsafe {
            self.swapchain_fn
                .queue_present(self.present_queue, &present_info)
                .unwrap()
        };
    }

    pub fn run(self, event_loop: EventLoop<()>, window: winit::window::Window) {
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::MainEventsCleared => self.draw_frame(),
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                } if window_id == window.id() => *control_flow = ControlFlow::Exit,
                Event::LoopDestroyed => {
                    unsafe { self.device.device_wait_idle().unwrap() };
                    self.destroy_vulkan()
                }
                _ => (),
            }
        })
    }

    fn destroy_vulkan(&self) {
        unsafe {
            self.device
                .destroy_semaphore(self.image_available_semaphore, None);
            self.device
                .destroy_semaphore(self.render_finished_semaphore, None);
            self.device.destroy_fence(self.in_flight_fence, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.swapchain_framebuffers.iter().for_each(|framebuffer| {
                self.device.destroy_framebuffer(*framebuffer, None);
            });
            self.device.destroy_pipeline(self.graphics_pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
            self.swapchain_image_views
                .iter()
                .for_each(|v| self.device.destroy_image_view(*v, None));
            self.swapchain_fn.destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_fn.destroy_surface(self.surface, None);
            if validation::ENABLE_VALIDATION_LAYERS {
                self.debug_utils_loader
                    .destroy_debug_utils_messenger(self.debug_messenger, None);
            }
            self.instance.destroy_instance(None);
        }
        log::debug!(target: "vkrs", "Deinitialized");
    }
}
