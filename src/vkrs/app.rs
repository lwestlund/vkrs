use super::vulkan;

use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

const NAME: &str = "vkrs";
const VERSION_MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");
const VERSION_MINOR: &str = env!("CARGO_PKG_VERSION_MINOR");
const VERSION_PATCH: &str = env!("CARGO_PKG_VERSION_PATCH");
const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

pub struct App {
    vk_data: vulkan::VkData,
}

impl App {
    pub fn new() -> Self {
        env_logger::init();

        let version_major = VERSION_MAJOR.parse().unwrap();
        let version_minor = VERSION_MINOR.parse().unwrap();
        let version_patch = VERSION_PATCH.parse().unwrap();
        let vk_data = vulkan::init(NAME, version_major, version_minor, version_patch);

        Self { vk_data }
    }

    fn init_window(event_loop: &EventLoop<()>) -> winit::window::Window {
        WindowBuilder::new()
            .with_title(NAME)
            .with_inner_size(LogicalSize::new(f64::from(WIDTH), f64::from(HEIGHT)))
            .with_resizable(false)
            .build(event_loop)
            .expect("Failed to create window.")
    }

    pub fn run(self) {
        let event_loop = EventLoop::new();
        let window = Self::init_window(&event_loop);

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                } if window_id == window.id() => *control_flow = ControlFlow::Exit,
                Event::LoopDestroyed => vulkan::deinit(&self.vk_data),
                _ => (),
            }
        })
    }
}
