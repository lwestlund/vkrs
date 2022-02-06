use super::extensions;
use super::validation;

use ash::vk;

use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_void},
};

pub struct VkData {
    _entry: ash::Entry,
    instance: ash::Instance,
    debug_utils_loader: ash::extensions::ext::DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT,
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
    VkData {
        _entry: entry,
        instance,
        debug_utils_loader,
        debug_messenger,
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
