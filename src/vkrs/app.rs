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
    _graphics_queue: vk::Queue,
    _present_queue: vk::Queue,
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
        Self {
            _entry: entry,
            instance,
            debug_utils_loader,
            surface_fn,
            surface,
            debug_messenger,
            _physical_device: physical_device,
            device,
            _graphics_queue: graphics_queue,
            _present_queue: present_queue,
        }
    }

    pub fn run(self, event_loop: EventLoop<()>, window: winit::window::Window) {
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                } if window_id == window.id() => *control_flow = ControlFlow::Exit,
                Event::LoopDestroyed => self.destroy_vulkan(),
                _ => (),
            }
        })
    }

    fn destroy_vulkan(&self) {
        unsafe {
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
