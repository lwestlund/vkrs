use ash::{extensions::ext::DebugUtils, vk};

use std::ffi::CStr;

#[cfg(target_os = "linux")]
pub fn get_required_extensions(window: &winit::window::Window) -> Vec<&'static CStr> {
    use crate::vkrs::validation;

    let mut extensions = ash_window::enumerate_required_extensions(window)
        .expect("Failed to enumerate required extensions for window.");
    if validation::ENABLE_VALIDATION_LAYERS {
        unsafe {
            extensions.push(CStr::from_ptr(DebugUtils::name().as_ptr()));
        }
    }
    extensions
}

pub fn check_required_extensions(
    entry: &ash::Entry,
    required_extensions: &[&'static CStr],
) -> Result<(), String> {
    let available_extensions = entry
        .enumerate_instance_extension_properties()
        .expect("Failed to get available extensions.");

    log::debug!(target: "vkrs", "available extensions:");
    for ext in &available_extensions {
        log::debug!(target: "vkrs", "\t{}", unsafe {
            CStr::from_ptr(ext.extension_name.as_ptr())
                .to_str()
                .expect("Failed to convert string to UTF-8.")
        });
    }

    let mut missing_extensions = String::new();
    for required_extension in required_extensions.iter() {
        unsafe {
            let extension_found = available_extensions.iter().find(|ext| {
                let available_extension = CStr::from_ptr(ext.extension_name.as_ptr());
                *required_extension == available_extension
            });
            if extension_found.is_none() {
                if !missing_extensions.is_empty() {
                    missing_extensions.push_str(", ");
                }
                missing_extensions.push_str(
                    required_extension
                        .to_str()
                        .expect("Failed to convert extension name to UTF-8."),
                );
            }
        }
    }
    if missing_extensions.is_empty() {
        Ok(())
    } else {
        Err(missing_extensions)
    }
}

pub fn get_required_device_extensions() -> [&'static CStr; 1] {
    [ash::extensions::khr::Swapchain::name()]
}

pub fn check_device_extension_support(
    instance: &ash::Instance,
    device: vk::PhysicalDevice,
) -> bool {
    let required_extensions = get_required_device_extensions();

    let extension_properties = unsafe {
        instance
            .enumerate_device_extension_properties(device)
            .expect("Failed to enumerate device extension properties.")
    };

    for required_extension in required_extensions.iter() {
        let found = extension_properties.iter().any(|ext| {
            let extension_name = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };
            *required_extension == extension_name
        });

        if !found {
            return false;
        }
    }

    true
}
