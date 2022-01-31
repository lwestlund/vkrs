use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

const WINDOW_TITLE: &'static str = "vkrs";
const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

pub struct App {}

impl App {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run(&mut self) {
        // TODO(lovew): Could these two be member variables instead? They need static lifetime.
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(f64::from(WIDTH), f64::from(HEIGHT)))
            .with_resizable(false)
            .build(&event_loop)
            .expect("Failed to create window");

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                } if window_id == window.id() => *control_flow = ControlFlow::Exit,
                _ => (),
            }
        })
    }
}
