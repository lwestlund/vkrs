use glam::Mat4;

#[derive(Clone, Copy)]
pub struct UniformBufferObject {
    pub model: Mat4,
    pub view: Mat4,
    pub proj: Mat4,
}
