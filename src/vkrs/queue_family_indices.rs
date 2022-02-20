use ash::vk;

pub struct QueueFamilyIndices {
    pub graphics_family: Option<u32>,
    pub present_family: Option<u32>,
}

impl QueueFamilyIndices {
    pub fn new() -> Self {
        let graphics_family = None;
        let present_family = None;
        Self {
            graphics_family,
            present_family,
        }
    }

    pub fn find_queue_families(
        instance: &ash::Instance,
        surface_fn: &ash::extensions::khr::Surface,
        surface: vk::SurfaceKHR,
        device: vk::PhysicalDevice,
    ) -> QueueFamilyIndices {
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(device) };
        let mut indices = Self::new();
        for (index, queue_family) in queue_families.iter().enumerate() {
            let index = index as u32;
            if queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                indices.graphics_family = Some(index);
            }
            let has_present_support = unsafe {
                surface_fn
                    .get_physical_device_surface_support(device, index, surface)
                    .unwrap()
            };
            if has_present_support && indices.present_family.is_none() {
                indices.present_family = Some(index)
            }

            if indices.is_complete() {
                break;
            }
        }

        indices
    }

    pub fn is_complete(&self) -> bool {
        self.graphics_family.is_some() && self.present_family.is_some()
    }
}
