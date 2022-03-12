use glam::{Vec2, Vec3};
use memoffset::offset_of;
use std::mem::size_of;

use ash::vk;

#[derive(Clone, Copy)]
pub struct Vertex {
    pub pos: Vec2,
    pub color: Vec3,
}

impl Vertex {
    pub fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(size_of::<Vertex>() as _)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build()
    }

    pub fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        let position_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(offset_of!(Vertex, pos) as _)
            .build();
        let color_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(offset_of!(Vertex, color) as _)
            .build();
        [position_desc, color_desc]
    }
}
