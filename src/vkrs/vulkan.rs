use super::extensions;
use super::validation;

use ash::vk;

use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_void},
};

struct QueueFamilyIndices {
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
            // TODO(lovew): Maybe change to prefer a queue supports both graphics and
            // presentation by simply and-ing the two if conditions.
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

pub fn create_instance(
    name: &str,
    version: u32,
    entry: &ash::Entry,
    window: &winit::window::Window,
) -> ash::Instance {
    let name = CString::new(name).unwrap();

    let app_info = vk::ApplicationInfo::builder()
        .application_name(name.as_c_str())
        .application_version(version)
        .engine_name(name.as_c_str())
        .engine_version(version)
        .api_version(vk::API_VERSION_1_2);

    let required_extensions = extensions::get_required_extensions(window);
    if let Err(missing_extensions) =
        extensions::check_required_extensions(entry, &required_extensions)
    {
        panic!("Missing extensions: {}", missing_extensions)
    }

    let validation_layer_names = validation::get_validation_layer_names_as_ptrs();
    let instance_extensions: Vec<*const c_char> =
        required_extensions.iter().map(|ext| ext.as_ptr()).collect();
    let mut instance_create_info = vk::InstanceCreateInfo::builder()
        .application_info(&app_info)
        .enabled_extension_names(&instance_extensions);

    // Used to debug create_instance and destroy_instance.
    let mut debug_utils_create_info = populate_debug_messenger_create_info();
    if validation::ENABLE_VALIDATION_LAYERS {
        if let Err(missing_layers) = validation::check_validation_layer_support(entry) {
            panic!("Missing validation layers: {}", missing_layers);
        }
        instance_create_info = instance_create_info
            .enabled_layer_names(&validation_layer_names)
            .push_next(&mut debug_utils_create_info);
    }

    unsafe {
        entry
            .create_instance(&instance_create_info, None)
            .expect("Failed to create Vulkan instance.")
    }
}

unsafe extern "system" fn debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    use vk::DebugUtilsMessageSeverityFlagsEXT as SeverityFlags;
    use vk::DebugUtilsMessageTypeFlagsEXT as TypeFlags;

    let message = CStr::from_ptr((*p_callback_data).p_message);
    let message_types = match message_types {
        TypeFlags::GENERAL => "[General]",
        TypeFlags::VALIDATION => "[Validation]",
        TypeFlags::PERFORMANCE => "[Performance]",
        _ => "[Unknown]",
    };
    match message_severity {
        SeverityFlags::VERBOSE => {
            log::debug!(target: "vulkan", "{}: {:?}", message_types, message);
        }
        SeverityFlags::INFO => {
            log::info!(target: "vulkan", "{}: {:?}", message_types, message)
        }
        SeverityFlags::WARNING => {
            log::warn!(target: "vulkan", "{}: {:?}", message_types, message)
        }
        SeverityFlags::ERROR => {
            log::error!(target: "vulkan", "{}: {:?}", message_types, message)
        }
        _ => {
            log::error!(target: "vulkan", "Unknown severity {}", message_severity.as_raw());
            log::error!(target: "vulkan", "{}: {:?}", message_types, message);
        }
    };
    vk::FALSE
}

fn populate_debug_messenger_create_info() -> vk::DebugUtilsMessengerCreateInfoEXT {
    use vk::DebugUtilsMessageSeverityFlagsEXT as SeverityFlags;
    use vk::DebugUtilsMessageTypeFlagsEXT as TypeFlags;

    vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(SeverityFlags::VERBOSE | SeverityFlags::WARNING | SeverityFlags::ERROR)
        .message_type(TypeFlags::GENERAL | TypeFlags::VALIDATION | TypeFlags::PERFORMANCE)
        .pfn_user_callback(Some(debug_callback))
        .build()
}

pub fn setup_debug_messenger(
    entry: &ash::Entry,
    instance: &ash::Instance,
) -> (ash::extensions::ext::DebugUtils, vk::DebugUtilsMessengerEXT) {
    let debug_utils_loader = ash::extensions::ext::DebugUtils::new(entry, instance);

    if !validation::ENABLE_VALIDATION_LAYERS {
        return (debug_utils_loader, vk::DebugUtilsMessengerEXT::null());
    }

    let create_info = populate_debug_messenger_create_info();

    let debug_messenger = unsafe {
        debug_utils_loader
            .create_debug_utils_messenger(&create_info, None)
            .expect("Failed to create debug messenger.")
    };
    (debug_utils_loader, debug_messenger)
}

fn rate_physical_device(
    instance: &ash::Instance,
    surface_fn: &ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
    device: vk::PhysicalDevice,
) -> u32 {
    let device_featues = unsafe { instance.get_physical_device_features(device) };
    if device_featues.geometry_shader != 1 {
        return 0;
    }

    let indices = QueueFamilyIndices::find_queue_families(instance, surface_fn, surface, device);
    if !indices.is_complete() {
        return 0;
    }

    let mut score = 0;
    let device_properties = unsafe { instance.get_physical_device_properties(device) };
    if device_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
        score += 1000;
    } else if device_properties.device_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
        score += 100;
    }
    score += device_properties.limits.max_image_dimension2_d;

    score
}

pub fn select_physical_device(
    instance: &ash::Instance,
    surface_fn: &ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
) -> vk::PhysicalDevice {
    let devices = unsafe {
        instance
            .enumerate_physical_devices()
            .expect("Failed to enumerate physical devices.")
    };
    log::debug!(target: "vulkan", "Available devices:");
    devices.iter().for_each(|device| {
        log::debug!(
            target: "vulkan",
            "\t{:?}",
            unsafe {
                CStr::from_ptr(instance
                               .get_physical_device_properties(*device)
                               .device_name
                               .as_ptr()
                ) })
    });

    let mut best_device_idx = 0;
    let mut max_score = 0;
    for (idx, device) in devices.iter().enumerate() {
        let score = rate_physical_device(instance, surface_fn, surface, *device);
        if score > max_score {
            best_device_idx = idx;
            max_score = score;
        }
    }

    if max_score > 0 {
        let best_device = devices[best_device_idx];
        let properties = unsafe { instance.get_physical_device_properties(best_device) };
        log::debug!(target: "vulkan",
                    "Selected device {:?} with score {}",
                    unsafe { CStr::from_ptr(properties.device_name.as_ptr()) },
                    max_score);
        return best_device;
    }
    panic!("Failed to find a suitable device.");
}

pub fn create_logical_device_with_graphics_and_present_queue(
    instance: &ash::Instance,
    surface_fn: &ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
) -> (ash::Device, vk::Queue, vk::Queue) {
    let queue_family_indices =
        QueueFamilyIndices::find_queue_families(instance, surface_fn, surface, physical_device);
    let queue_priorities = [1.0f32];
    let graphics_family_index = queue_family_indices.graphics_family.unwrap();
    let present_family_index = queue_family_indices.present_family.unwrap();
    let device_queue_create_infos: Vec<vk::DeviceQueueCreateInfo> = {
        // We only need to give the unique queue families needed, and graphics and present
        // may be supported by the same queue, so we remove duplicates if any.
        let mut queue_family_indices = vec![graphics_family_index, present_family_index];
        queue_family_indices.dedup();

        // Create a vector of DeviceQueueCreateInfo for each queue.
        queue_family_indices
            .iter()
            .map(|index| {
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(*index)
                    .queue_priorities(&queue_priorities)
                    .build()
            })
            .collect()
    };

    let required_validation_layers = validation::get_validation_layer_names_as_ptrs();
    let device_features = vk::PhysicalDeviceFeatures::builder();
    let mut device_create_info = vk::DeviceCreateInfo::builder()
        .enabled_features(&device_features)
        .queue_create_infos(&device_queue_create_infos);
    if validation::ENABLE_VALIDATION_LAYERS {
        device_create_info = device_create_info.enabled_layer_names(&required_validation_layers);
    }

    // Create the logical device and required queues.
    let device = unsafe {
        instance
            .create_device(physical_device, &device_create_info, None)
            .expect("Failed to create logical device.")
    };
    let graphics_queue = unsafe { device.get_device_queue(graphics_family_index, 0) };
    let present_queue = unsafe { device.get_device_queue(present_family_index, 0) };
    (device, graphics_queue, present_queue)
}
