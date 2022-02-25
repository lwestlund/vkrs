use super::extensions;
use super::queue_family_indices::QueueFamilyIndices;
use super::shader;
use super::swapchain;
use super::validation;

use ash::vk;

use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_void},
    path::PathBuf,
};

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

    let device_extension_support = extensions::check_device_extension_support(instance, device);
    if !device_extension_support {
        return 0;
    }

    // Can only get swapchain support details after we have verified device extension support for it.
    let swapchain_support_details = swapchain::SupportDetails::new(device, surface_fn, surface);
    if swapchain_support_details.formats.is_empty()
        || swapchain_support_details.present_modes.is_empty()
    {
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
    let device_queue_create_infos = {
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
            .collect::<Vec<_>>()
    };

    let required_validation_layers = validation::get_validation_layer_names_as_ptrs();
    let device_extensions = extensions::get_required_device_extensions();
    let device_extension_names = device_extensions
        .iter()
        .map(|ext| ext.as_ptr())
        .collect::<Vec<_>>();
    let device_features = vk::PhysicalDeviceFeatures::builder();
    let mut device_create_info = vk::DeviceCreateInfo::builder()
        .enabled_extension_names(&device_extension_names)
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

pub fn create_render_pass(
    device: &ash::Device,
    swapchain_image_format: vk::Format,
) -> vk::RenderPass {
    let color_attachment = vk::AttachmentDescription::builder()
        .format(swapchain_image_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .build();
    let color_attachments = [color_attachment];

    let color_attachment_ref = vk::AttachmentReference::builder()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .build();
    let color_attachment_refs = [color_attachment_ref];

    let subpass = vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_refs)
        .build();
    let subpasses = [subpass];

    let render_pass_info = vk::RenderPassCreateInfo::builder()
        .attachments(&color_attachments)
        .subpasses(&subpasses);

    unsafe { device.create_render_pass(&render_pass_info, None).unwrap() }
}

pub fn create_graphics_pipeline(
    device: &ash::Device,
    swapchain_extent: vk::Extent2D,
) -> vk::PipelineLayout {
    let out_dir = PathBuf::from("src/vkrs/shaders");
    let vertex_shader_code = shader::read_shader_file(&out_dir.join("shader.vert.spv"));
    let fragment_shader_code = shader::read_shader_file(&out_dir.join("shader.frag.spv"));

    let vertex_shader_module = shader::create_shader_module(device, &vertex_shader_code);
    let fragment_shader_module = shader::create_shader_module(device, &fragment_shader_code);

    let shader_entry_point = CString::new("main").unwrap();
    let vertex_shader_stage_info = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(vertex_shader_module)
        .name(&shader_entry_point)
        .build();
    let fragment_shader_stage_info = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::FRAGMENT)
        .module(fragment_shader_module)
        .name(&shader_entry_point)
        .build();

    let _shader_stages = [vertex_shader_stage_info, fragment_shader_stage_info];

    // Fixed function configuration.
    // Vertex input.
    let _vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        // .vertex_binding_descriptions(&[]) // Empty because vertices are hard coded in shader.
        // .vertex_attribute_descriptions(&[]) // Empty because vertices are hard coded in shader.
        .build();

    // Input assembly.
    let _input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false);

    // Viewports and scissors.
    let viewport = vk::Viewport::builder()
        .x(0.0)
        .y(0.0)
        .width(swapchain_extent.width as _)
        .height(swapchain_extent.height as _)
        .min_depth(0.0)
        .max_depth(1.0)
        .build();
    let viewports = [viewport];
    let scissor = vk::Rect2D::builder()
        .offset(vk::Offset2D { x: 0, y: 0 })
        .extent(swapchain_extent)
        .build();
    let scissors = [scissor];
    let _viewport_state = vk::PipelineViewportStateCreateInfo::builder()
        .viewports(&viewports)
        .scissors(&scissors);

    // Rasterizer.
    let _rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::CLOCKWISE)
        .depth_bias_enable(false);

    // Multisampling.
    let _multisampling = vk::PipelineMultisampleStateCreateInfo::builder()
        .sample_shading_enable(false)
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .min_sample_shading(1.0)
        .alpha_to_coverage_enable(false)
        .alpha_to_one_enable(false);

    // Color blending.
    let color_blend_attachment = vk::PipelineColorBlendAttachmentState::builder()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(false)
        .src_color_blend_factor(vk::BlendFactor::ONE)
        .dst_color_blend_factor(vk::BlendFactor::ZERO)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
        .alpha_blend_op(vk::BlendOp::ADD)
        .build();
    let color_blend_attachments = [color_blend_attachment];
    let _color_blending = vk::PipelineColorBlendStateCreateInfo::builder()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(&color_blend_attachments)
        .blend_constants([0.0, 0.0, 0.0, 0.0]);

    // Pipeline layout.
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder().build();
    let pipeline_layout = unsafe {
        device
            .create_pipeline_layout(&pipeline_layout_info, None)
            .unwrap()
    };

    unsafe {
        device.destroy_shader_module(vertex_shader_module, None);
        device.destroy_shader_module(fragment_shader_module, None);
    }

    pipeline_layout
}
