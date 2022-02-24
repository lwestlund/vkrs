use std::{fs::File, path::Path};

use ash::vk;

pub fn read_shader_file(path: &Path) -> Vec<u32> {
    let mut file = File::open(path).unwrap();
    ash::util::read_spv(&mut file).unwrap()
}

pub fn create_shader_module(device: &ash::Device, shader_code: &[u32]) -> vk::ShaderModule {
    let create_info = vk::ShaderModuleCreateInfo::builder().code(shader_code);
    unsafe { device.create_shader_module(&create_info, None).unwrap() }
}
