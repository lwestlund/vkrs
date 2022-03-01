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

const MAX_FRAMES_IN_FLIGHT: u32 = 2;

pub struct App {
    _entry: ash::Entry,
    instance: ash::Instance,
    debug_utils_loader: ash::extensions::ext::DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    surface_fn: ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    swapchain: ash::extensions::khr::Swapchain,
    swapchain_khr: vk::SwapchainKHR,
    swapchain_image_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    graphics_pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    swapchain_framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,
}

#[derive(PartialEq)]
enum RecreateSwapchain {
    No,
    Yes,
}

#[derive(PartialEq)]
enum DestroyOldSwapchain {
    No,
    Yes,
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
        let (swapchain, swapchain_khr, swapchain_image_format, swapchain_extent, swapchain_images) =
            swapchain::create_swapchain_and_images(
                &instance,
                physical_device,
                &device,
                &surface_fn,
                surface,
                &window.inner_size(),
                None,
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
        let command_buffers =
            vulkan::create_command_buffers(&device, command_pool, MAX_FRAMES_IN_FLIGHT);

        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) =
            vulkan::create_sync_objects(&device, MAX_FRAMES_IN_FLIGHT);

        Self {
            _entry: entry,
            instance,
            debug_utils_loader,
            surface_fn,
            surface,
            debug_messenger,
            physical_device,
            device,
            graphics_queue,
            present_queue,
            swapchain,
            swapchain_khr,
            swapchain_image_format,
            swapchain_extent,
            swapchain_images,
            swapchain_image_views,
            render_pass,
            graphics_pipeline,
            pipeline_layout,
            swapchain_framebuffers,
            command_pool,
            command_buffers,
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            current_frame: 0,
        }
    }

    fn recreate_swapchain(&mut self, window_size: &winit::dpi::PhysicalSize<u32>) {
        unsafe { self.device.device_wait_idle().unwrap() };

        self.cleanup_swapchain(DestroyOldSwapchain::No);

        let (swapchain, swapchain_khr, swapchain_image_format, swapchain_extent, swapchain_images) =
            swapchain::create_swapchain_and_images(
                &self.instance,
                self.physical_device,
                &self.device,
                &self.surface_fn,
                self.surface,
                window_size,
                Some(self.swapchain_khr),
            );
        let swapchain_image_views =
            swapchain::create_image_views(&self.device, &swapchain_images, swapchain_image_format);
        let render_pass = vulkan::create_render_pass(&self.device, swapchain_image_format);
        let (graphics_pipeline, pipeline_layout) =
            vulkan::create_graphics_pipeline(&self.device, swapchain_extent, render_pass);
        let swapchain_framebuffers = vulkan::create_framebuffers(
            &self.device,
            &swapchain_image_views,
            render_pass,
            swapchain_extent,
        );

        self.swapchain = swapchain;
        self.swapchain_khr = swapchain_khr;
        self.swapchain_image_format = swapchain_image_format;
        self.swapchain_extent = swapchain_extent;
        self.swapchain_images = swapchain_images;
        self.swapchain_image_views = swapchain_image_views;
        self.render_pass = render_pass;
        self.graphics_pipeline = graphics_pipeline;
        self.pipeline_layout = pipeline_layout;
        self.swapchain_framebuffers = swapchain_framebuffers;
    }

    fn draw_frame(&mut self) -> RecreateSwapchain {
        let fences = [self.in_flight_fences[self.current_frame]];
        unsafe {
            self.device
                .wait_for_fences(&fences, true, u64::MAX)
                .unwrap();
        };

        let result = unsafe {
            self.swapchain.acquire_next_image(
                self.swapchain_khr,
                u64::MAX,
                self.image_available_semaphores[self.current_frame],
                vk::Fence::null(),
            )
        };
        let image_index = match result {
            Ok((image_index, _)) => image_index,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return RecreateSwapchain::Yes,
            Err(error) => panic!("Error acquiring next image: {:?}", error),
        };

        // Reset the fence first when we know there will be work
        // submitted so that it will get signaled again.
        unsafe { self.device.reset_fences(&fences).unwrap() };

        let command_buffer = self.command_buffers[self.current_frame];
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

        let wait_semaphores = [self.image_available_semaphores[self.current_frame]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = [command_buffer];
        let signal_semaphores = [self.render_finished_semaphores[self.current_frame]];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores)
            .build();
        let submit_infos = [submit_info];
        unsafe {
            self.device
                .queue_submit(
                    self.graphics_queue,
                    &submit_infos,
                    self.in_flight_fences[self.current_frame],
                )
                .expect("Failed to submit draw command buffer.")
        };

        let swapchains = [self.swapchain_khr];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        let result = unsafe {
            self.swapchain
                .queue_present(self.present_queue, &present_info)
        };
        match result {
            Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return RecreateSwapchain::Yes,
            Err(error) => panic!("Failed to present swapchain image: {:?}", error),
            _ => {}
        }

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT as usize;
        RecreateSwapchain::No
    }

    pub fn run(mut self, event_loop: EventLoop<()>, window: winit::window::Window) {
        let mut recreate_swapchain = RecreateSwapchain::No;
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::MainEventsCleared => {
                    if recreate_swapchain == RecreateSwapchain::Yes {
                        let inner_size = window.inner_size();
                        if inner_size.width == 0 || inner_size.height == 0 {
                            return;
                        }
                        self.recreate_swapchain(&inner_size);
                    }
                    recreate_swapchain = self.draw_frame()
                }
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                } if window_id == window.id() => *control_flow = ControlFlow::Exit,
                Event::WindowEvent {
                    event: WindowEvent::Resized(window_size),
                    ..
                } => self.recreate_swapchain(&window_size),
                Event::LoopDestroyed => {
                    unsafe { self.device.device_wait_idle().unwrap() };
                    self.destroy_vulkan()
                }
                _ => (),
            }
        })
    }

    fn cleanup_swapchain(&self, destroy_old_swapchain: DestroyOldSwapchain) {
        unsafe {
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
            if destroy_old_swapchain == DestroyOldSwapchain::Yes {
                self.swapchain.destroy_swapchain(self.swapchain_khr, None);
            }
        }
    }

    fn destroy_vulkan(&self) {
        unsafe {
            self.image_available_semaphores.iter().for_each(|s| {
                self.device.destroy_semaphore(*s, None);
            });
            self.render_finished_semaphores.iter().for_each(|s| {
                self.device.destroy_semaphore(*s, None);
            });
            self.in_flight_fences.iter().for_each(|f| {
                self.device.destroy_fence(*f, None);
            });
            self.device.destroy_command_pool(self.command_pool, None);
            self.cleanup_swapchain(DestroyOldSwapchain::Yes);
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
