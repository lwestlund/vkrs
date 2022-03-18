use super::queue_family_indices::QueueFamilyIndices;
use super::swapchain;
use super::uniform_buffer_object::UniformBufferObject;
use super::validation;
use super::vertex::Vertex;
use super::vulkan;

use ash::vk;
use glam::{const_vec2, const_vec3, Mat4};
use std::{
    mem::{align_of, size_of},
    time::Instant,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

const VERSION_MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");
const VERSION_MINOR: &str = env!("CARGO_PKG_VERSION_MINOR");
const VERSION_PATCH: &str = env!("CARGO_PKG_VERSION_PATCH");

const MAX_FRAMES_IN_FLIGHT: u32 = 2;

const VERTICES: [Vertex; 4] = [
    Vertex {
        pos: const_vec2!([-0.5, -0.5]),
        color: const_vec3!([1.0, 0.0, 0.0]),
    },
    Vertex {
        pos: const_vec2!([0.5, -0.5]),
        color: const_vec3!([0.0, 1.0, 0.0]),
    },
    Vertex {
        pos: const_vec2!([0.5, 0.5]),
        color: const_vec3!([0.0, 0.0, 1.0]),
    },
    Vertex {
        pos: const_vec2!([-0.5, 0.5]),
        color: const_vec3!([1.0, 1.0, 1.0]),
    },
];

const INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

pub struct App {
    start_instant: Instant,
    _entry: ash::Entry,
    instance: ash::Instance,
    debug_utils_loader: ash::extensions::ext::DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    surface_fn: ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    queue_family_indices: QueueFamilyIndices,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    swapchain: ash::extensions::khr::Swapchain,
    swapchain_khr: vk::SwapchainKHR,
    swapchain_image_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    descriptor_set_layout: vk::DescriptorSetLayout,
    graphics_pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    swapchain_framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    transient_command_pool: vk::CommandPool,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    vertex_buffer: vk::Buffer,
    vertex_buffer_memory: vk::DeviceMemory,
    index_buffer: vk::Buffer,
    index_buffer_memory: vk::DeviceMemory,
    uniform_buffers: Vec<vk::Buffer>,
    uniform_buffer_memories: Vec<vk::DeviceMemory>,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: Vec<vk::DescriptorSet>,
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
        let (physical_device, queue_family_indices) =
            vulkan::select_physical_device(&instance, &surface_fn, surface);
        let (device, graphics_queue, present_queue) =
            vulkan::create_logical_device_with_graphics_and_present_queue(
                &instance,
                &queue_family_indices,
                physical_device,
            );
        let (swapchain, swapchain_khr, swapchain_image_format, swapchain_extent, swapchain_images) =
            swapchain::create_swapchain_and_images(
                &instance,
                physical_device,
                &device,
                &surface_fn,
                surface,
                &queue_family_indices,
                &window.inner_size(),
                None,
            );
        let swapchain_image_views =
            swapchain::create_image_views(&device, &swapchain_images, swapchain_image_format);

        let render_pass = vulkan::create_render_pass(&device, swapchain_image_format);
        let descriptor_set_layout = vulkan::create_descriptor_set_layout(&device);
        let (graphics_pipeline, pipeline_layout) = vulkan::create_graphics_pipeline(
            &device,
            swapchain_extent,
            render_pass,
            descriptor_set_layout,
        );

        let swapchain_framebuffers = vulkan::create_framebuffers(
            &device,
            &swapchain_image_views,
            render_pass,
            swapchain_extent,
        );

        let command_pool = vulkan::create_command_pool(
            &device,
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            &queue_family_indices,
        );
        let transient_command_pool = vulkan::create_command_pool(
            &device,
            vk::CommandPoolCreateFlags::TRANSIENT,
            &queue_family_indices,
        );
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let (vertex_buffer, vertex_buffer_memory) = vulkan::create_vertex_buffer(
            &device,
            memory_properties,
            graphics_queue,
            transient_command_pool,
            &VERTICES,
        );
        // TODO(lovew): Instead of allocating a separate buffer for vertex indices we should have
        // allocated only a single buffer and simply used an offset into it to store vertex indices
        // in the same memory after the vertices themselves.
        let (index_buffer, index_buffer_memory) = vulkan::create_index_buffer(
            &device,
            memory_properties,
            graphics_queue,
            transient_command_pool,
            &INDICES,
        );

        let (uniform_buffers, uniform_buffer_memories) =
            vulkan::create_uniform_buffers(&device, memory_properties, swapchain_images.len() as _);

        let descriptor_pool = vulkan::create_descriptor_pool(&device, swapchain_images.len() as _);
        let descriptor_sets = vulkan::create_descriptor_sets(
            &device,
            descriptor_pool,
            descriptor_set_layout,
            &uniform_buffers,
        );

        let command_buffers =
            vulkan::create_command_buffers(&device, command_pool, MAX_FRAMES_IN_FLIGHT);

        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) =
            vulkan::create_sync_objects(&device, MAX_FRAMES_IN_FLIGHT);

        Self {
            start_instant: Instant::now(),
            _entry: entry,
            instance,
            debug_utils_loader,
            surface_fn,
            surface,
            debug_messenger,
            physical_device,
            queue_family_indices,
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
            descriptor_set_layout,
            graphics_pipeline,
            pipeline_layout,
            swapchain_framebuffers,
            command_pool,
            transient_command_pool,
            memory_properties,
            vertex_buffer,
            vertex_buffer_memory,
            index_buffer,
            index_buffer_memory,
            uniform_buffers,
            uniform_buffer_memories,
            descriptor_pool,
            descriptor_sets,
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
                &self.queue_family_indices,
                window_size,
                Some(self.swapchain_khr),
            );
        let swapchain_image_views =
            swapchain::create_image_views(&self.device, &swapchain_images, swapchain_image_format);
        let render_pass = vulkan::create_render_pass(&self.device, swapchain_image_format);
        let (graphics_pipeline, pipeline_layout) = vulkan::create_graphics_pipeline(
            &self.device,
            swapchain_extent,
            render_pass,
            self.descriptor_set_layout,
        );
        let swapchain_framebuffers = vulkan::create_framebuffers(
            &self.device,
            &swapchain_image_views,
            render_pass,
            swapchain_extent,
        );
        let (uniform_buffers, uniform_buffer_memories) = vulkan::create_uniform_buffers(
            &self.device,
            self.memory_properties,
            swapchain_images.len() as _,
        );
        let descriptor_pool =
            vulkan::create_descriptor_pool(&self.device, swapchain_images.len() as _);
        let descriptor_sets = vulkan::create_descriptor_sets(
            &self.device,
            descriptor_pool,
            self.descriptor_set_layout,
            &uniform_buffers,
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
        self.uniform_buffers = uniform_buffers;
        self.uniform_buffer_memories = uniform_buffer_memories;
        self.descriptor_pool = descriptor_pool;
        self.descriptor_sets = descriptor_sets;
    }

    fn update_uniform_buffer(&self, image_index: u32) {
        let elapsed = self.start_instant.elapsed().as_secs_f32();

        let aspect_ratio = self.swapchain_extent.width as f32 / self.swapchain_extent.height as f32;
        let model = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_4 * elapsed);
        let view = Mat4::look_at_rh(
            const_vec3!([2.0, 2.0, 2.0]),
            const_vec3!([0.0, 0.0, 0.0]),
            const_vec3!([0.0, 0.0, 1.0]),
        );
        let mut proj = Mat4::perspective_rh(f32::to_radians(45.0), aspect_ratio, 0.1, 10.0);
        proj.y_axis.y *= -1.0;
        let ubo = UniformBufferObject { model, view, proj };
        let ubos = [ubo];

        // TODO(lovew): Look at replacing this with "push constants".
        let buffer_memory = self.uniform_buffer_memories[image_index as usize];
        let size = size_of::<UniformBufferObject>() as vk::DeviceSize;
        unsafe {
            let data_ptr = self
                .device
                .map_memory(buffer_memory, 0, size, vk::MemoryMapFlags::empty())
                .unwrap();
            let mut align = ash::util::Align::new(data_ptr, align_of::<f32>() as _, size);
            align.copy_from_slice(&ubos);
            self.device.unmap_memory(buffer_memory);
        }
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

        self.update_uniform_buffer(image_index);

        // Reset the fence first when we know there will be work
        // submitted so that it will get signaled again.
        unsafe { self.device.reset_fences(&fences).unwrap() };

        let command_buffer = self.command_buffers[self.current_frame];
        let frame_buffer = self.swapchain_framebuffers[image_index as usize];
        let descriptor_set = self.descriptor_sets[image_index as usize];
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
            self.vertex_buffer,
            self.index_buffer,
            INDICES.len() as _,
            self.pipeline_layout,
            descriptor_set,
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
            self.uniform_buffers.iter().for_each(|b| {
                self.device.destroy_buffer(*b, None);
            });
            self.uniform_buffer_memories.iter().for_each(|m| {
                self.device.free_memory(*m, None);
            });
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
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
            self.device.destroy_buffer(self.index_buffer, None);
            self.device.free_memory(self.index_buffer_memory, None);
            self.device.destroy_buffer(self.vertex_buffer, None);
            self.device.free_memory(self.vertex_buffer_memory, None);
            self.image_available_semaphores.iter().for_each(|s| {
                self.device.destroy_semaphore(*s, None);
            });
            self.render_finished_semaphores.iter().for_each(|s| {
                self.device.destroy_semaphore(*s, None);
            });
            self.in_flight_fences.iter().for_each(|f| {
                self.device.destroy_fence(*f, None);
            });
            self.device
                .destroy_command_pool(self.transient_command_pool, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.cleanup_swapchain(DestroyOldSwapchain::Yes);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
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
