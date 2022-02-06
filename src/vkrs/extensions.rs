use ash::extensions::ext::DebugUtils;
use ash::extensions::khr::Surface;

use std::ffi::CStr;

#[cfg(target_os = "linux")]
use ash::extensions::khr::XlibSurface;

#[cfg(target_os = "linux")]
pub fn get_required_extensions() -> Vec<*const i8> {
    use crate::vkrs::validation;

    let mut extensions = vec![Surface::name().as_ptr(), XlibSurface::name().as_ptr()];
    if validation::ENABLE_VALIDATION_LAYERS {
        extensions.push(DebugUtils::name().as_ptr());
    }
    extensions
}

pub fn check_required_extensions(
    entry: &ash::Entry,
    required_extensions: &[*const i8],
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
            let required_extension = CStr::from_ptr(*required_extension);
            let extension_found = available_extensions.iter().find(|ext| {
                let available_extension = CStr::from_ptr(ext.extension_name.as_ptr());
                required_extension == available_extension
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
