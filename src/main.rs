mod vkrs;

use winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder};

const NAME: &str = "vkrs";
const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(NAME)
        .with_inner_size(LogicalSize::new(f64::from(WIDTH), f64::from(HEIGHT)))
        .build(&event_loop)
        .expect("Failed to create window.");

    let app = vkrs::App::new(NAME, &window);
    app.run(event_loop, window);
}
