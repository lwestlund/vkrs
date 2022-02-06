use std::ffi::CStr;

#[cfg(debug_assertions)]
pub const ENABLE_VALIDATION_LAYERS: bool = true;

#[cfg(not(debug_assertions))]
pub const ENABLE_VALIDATION_LAYERS: bool = false;

const REQUIRED_VALIDATION_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation\0"];

pub fn get_validation_layer_names_as_ptrs() -> Vec<*const c_char> {
    if ENABLE_VALIDATION_LAYERS {
        REQUIRED_VALIDATION_LAYERS
            .iter()
            .map(|layer| {
                CStr::from_bytes_with_nul(layer.as_bytes())
                    .unwrap()
                    .as_ptr()
            })
            .collect()
    } else {
        Vec::new()
    }
}

pub fn check_validation_layer_support(entry: &ash::Entry) -> Result<(), String> {
    let available_validation_layers = entry
        .enumerate_instance_layer_properties()
        .expect("Failed to get validation layers");

    log::debug!(target: "vkrs", "available validation layers:");
    for validation_layer in &available_validation_layers {
        log::debug!(target: "vkrs", "\t{}", unsafe {
            CStr::from_ptr(validation_layer.layer_name.as_ptr())
                .to_str()
                .expect("Failed to convert string to UTF-8.")
        });
    }

    let mut missing_layers = String::new();
    for required_layer in get_validation_layer_names_as_ptrs().iter() {
        unsafe {
            let required_layer = CStr::from_ptr(*required_layer);
            let validation_layer_found = available_validation_layers.iter().find(|layer| {
                let available_layer = CStr::from_ptr(layer.layer_name.as_ptr());
                required_layer == available_layer
            });
            if validation_layer_found.is_none() {
                if !missing_layers.is_empty() {
                    missing_layers.push_str(", ");
                }
                missing_layers.push_str(
                    required_layer
                        .to_str()
                        .expect("Failed to convert layer name to UTF-8."),
                );
            }
        }
    }

    if missing_layers.is_empty() {
        Ok(())
    } else {
        Err(missing_layers)
    }
}
