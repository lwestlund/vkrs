use super::extensions;
use super::validation;

use ash::vk;

use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_void},
};

struct QueueFamilyIndices {
    pub graphics_family: Option<u32>,
}

impl QueueFamilyIndices {
    pub fn new() -> Self {
        let graphics_family = None;
        Self { graphics_family }
    }

    pub fn find_queue_families(
        instance: &ash::Instance,
        device: vk::PhysicalDevice,
    ) -> QueueFamilyIndices {
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(device) };
        let mut indices = Self::new();
        for (idx, queue_family) in queue_families.iter().enumerate() {
            if queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                indices.graphics_family = Some(idx as u32);
            }

            if indices.is_complete() {
                break;
            }
        }

        indices
    }

    pub fn is_complete(&self) -> bool {
        self.graphics_family.is_some()
    }
}

pub struct VkData {
    _entry: ash::Entry,
    instance: ash::Instance,
    debug_utils_loader: ash::extensions::ext::DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    _physical_device: vk::PhysicalDevice,
}

fn create_instance(name: &str, version: u32, entry: &ash::Entry) -> ash::Instance {
    let name = CString::new(name).unwrap();

    let app_info = vk::ApplicationInfo::builder()
        .application_name(name.as_c_str())
        .application_version(version)
        .engine_name(name.as_c_str())
        .engine_version(version)
        .api_version(vk::API_VERSION_1_2)
        .build();

    let required_extensions = extensions::get_required_extensions();
    if let Err(missing_extensions) =
        extensions::check_required_extensions(entry, &required_extensions)
    {
        panic!("Missing extensions: {}", missing_extensions)
    }

    let validation_layer_names = validation::get_validation_layer_names_as_ptrs();
    let mut instance_create_info = vk::InstanceCreateInfo::builder()
        .application_info(&app_info)
        .enabled_extension_names(&required_extensions);

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

fn setup_debug_messenger(
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

fn rate_physical_device(instance: &ash::Instance, device: vk::PhysicalDevice) -> u32 {
    let device_featues = unsafe { instance.get_physical_device_features(device) };
    if device_featues.geometry_shader != 1 {
        return 0;
    }

    let indices = QueueFamilyIndices::find_queue_families(instance, device);
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

fn select_physical_device(instance: &ash::Instance) -> vk::PhysicalDevice {
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
        let score = rate_physical_device(instance, *device);
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

pub fn init(
    name: &'static str,
    version_major: u32,
    version_minor: u32,
    version_patch: u32,
) -> VkData {
    let entry = unsafe { ash::Entry::load().expect("Failed to load Vulkan.") };

    let version = vk::make_api_version(0, version_major, version_minor, version_patch);

    let instance = create_instance(name, version, &entry);
    let (debug_utils_loader, debug_messenger) = setup_debug_messenger(&entry, &instance);
    let physical_device = select_physical_device(&instance);
    VkData {
        _entry: entry,
        instance,
        debug_utils_loader,
        debug_messenger,
        _physical_device: physical_device,
    }
}

pub fn deinit(vk_data: &VkData) {
    unsafe {
        if validation::ENABLE_VALIDATION_LAYERS {
            vk_data
                .debug_utils_loader
                .destroy_debug_utils_messenger(vk_data.debug_messenger, None);
        }
        vk_data.instance.destroy_instance(None);
    }
    log::debug!(target: "vulkan", "Deinitialized");
}
