use super::extensions;
use super::queue_family_indices::QueueFamilyIndices;
use super::shader;
use super::swapchain;
use super::uniform_buffer_object::UniformBufferObject;
use super::validation;
use super::vertex::Vertex;

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
) -> (u32, Option<QueueFamilyIndices>) {
    let device_featues = unsafe { instance.get_physical_device_features(device) };
    if device_featues.geometry_shader != 1 {
        return (0, None);
    }

    let indices = QueueFamilyIndices::find_queue_families(instance, surface_fn, surface, device);
    if !indices.is_complete() {
        return (0, None);
    }

    let device_extension_support = extensions::check_device_extension_support(instance, device);
    if !device_extension_support {
        return (0, None);
    }

    // Can only get swapchain support details after we have verified device extension support for it.
    let swapchain_support_details = swapchain::SupportDetails::new(device, surface_fn, surface);
    if swapchain_support_details.formats.is_empty()
        || swapchain_support_details.present_modes.is_empty()
    {
        return (0, None);
    }

    let mut score = 0;
    let device_properties = unsafe { instance.get_physical_device_properties(device) };
    if device_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
        score += 1000;
    } else if device_properties.device_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
        score += 100;
    }
    score += device_properties.limits.max_image_dimension2_d;

    (score, Some(indices))
}

pub fn select_physical_device(
    instance: &ash::Instance,
    surface_fn: &ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
) -> (vk::PhysicalDevice, QueueFamilyIndices) {
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
    let mut queue_family_indices = QueueFamilyIndices::new();
    for (idx, device) in devices.iter().enumerate() {
        let (score, indices) = rate_physical_device(instance, surface_fn, surface, *device);
        if score > max_score && indices.is_some() {
            best_device_idx = idx;
            max_score = score;
            queue_family_indices = indices.unwrap();
        }
    }

    if max_score > 0 && queue_family_indices.is_complete() {
        let best_device = devices[best_device_idx];
        let properties = unsafe { instance.get_physical_device_properties(best_device) };
        log::debug!(target: "vulkan",
                    "Selected device {:?} with score {}",
                    unsafe { CStr::from_ptr(properties.device_name.as_ptr()) },
                    max_score);
        return (best_device, queue_family_indices);
    }
    panic!("Failed to find a suitable device.");
}

pub fn create_logical_device_with_graphics_and_present_queue(
    instance: &ash::Instance,
    queue_family_indices: &QueueFamilyIndices,
    physical_device: vk::PhysicalDevice,
) -> (ash::Device, vk::Queue, vk::Queue) {
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

    let dependency = vk::SubpassDependency::builder()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0) // Reference to subpasses[0].
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
        .build();
    let dependencies = [dependency];

    let render_pass_info = vk::RenderPassCreateInfo::builder()
        .attachments(&color_attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);

    unsafe { device.create_render_pass(&render_pass_info, None).unwrap() }
}

pub fn create_graphics_pipeline(
    device: &ash::Device,
    swapchain_extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> (vk::Pipeline, vk::PipelineLayout) {
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

    let shader_stages = [vertex_shader_stage_info, fragment_shader_stage_info];

    // Fixed function configuration.
    // Vertex input.
    let vertex_binding_descriptions = [Vertex::get_binding_description()];
    let vertex_attribute_descriptions = Vertex::get_attribute_descriptions();
    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vertex_binding_descriptions)
        .vertex_attribute_descriptions(&vertex_attribute_descriptions)
        .build();

    // Input assembly.
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
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
    let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
        .viewports(&viewports)
        .scissors(&scissors);

    // Rasterizer.
    let rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::CLOCKWISE)
        .depth_bias_enable(false);

    // Multisampling.
    let multisampling = vk::PipelineMultisampleStateCreateInfo::builder()
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
    let color_blending = vk::PipelineColorBlendStateCreateInfo::builder()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(&color_blend_attachments)
        .blend_constants([0.0, 0.0, 0.0, 0.0]);

    // Pipeline layout.
    let set_layouts = [descriptor_set_layout];
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder().set_layouts(&set_layouts);
    let pipeline_layout = unsafe {
        device
            .create_pipeline_layout(&pipeline_layout_info, None)
            .unwrap()
    };

    let pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
        .stages(&shader_stages)
        .vertex_input_state(&vertex_input_info)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterizer)
        .multisample_state(&multisampling)
        .color_blend_state(&color_blending)
        .layout(pipeline_layout)
        .render_pass(render_pass)
        .subpass(0)
        .build();
    let pipeline_infos = [pipeline_info];
    let graphics_pipeline = unsafe {
        device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_infos, None)
            .unwrap()[0]
    };

    unsafe {
        device.destroy_shader_module(vertex_shader_module, None);
        device.destroy_shader_module(fragment_shader_module, None);
    }

    (graphics_pipeline, pipeline_layout)
}

pub fn create_framebuffers(
    device: &ash::Device,
    swapchain_image_views: &[vk::ImageView],
    render_pass: vk::RenderPass,
    swapchain_extent: vk::Extent2D,
) -> Vec<vk::Framebuffer> {
    swapchain_image_views
        .iter()
        .map(|view| [*view])
        .map(|attachments| {
            let framebuffer_info = vk::FramebufferCreateInfo::builder()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(swapchain_extent.width)
                .height(swapchain_extent.height)
                .layers(1);
            unsafe { device.create_framebuffer(&framebuffer_info, None).unwrap() }
        })
        .collect::<Vec<_>>()
}

pub fn create_command_pool(
    device: &ash::Device,
    command_pool_create_flags: vk::CommandPoolCreateFlags,
    queue_family_indices: &QueueFamilyIndices,
) -> vk::CommandPool {
    let pool_info = vk::CommandPoolCreateInfo::builder()
        .flags(command_pool_create_flags)
        .queue_family_index(queue_family_indices.graphics_family.unwrap());

    unsafe { device.create_command_pool(&pool_info, None).unwrap() }
}

fn find_memory_type(
    memory_requirements: vk::MemoryRequirements,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    required_properties: vk::MemoryPropertyFlags,
) -> u32 {
    for i in 0..memory_properties.memory_type_count {
        if memory_requirements.memory_type_bits & (1 << i) != 0
            && (memory_properties.memory_types[i as usize].property_flags & required_properties)
                == required_properties
        {
            return i;
        }
    }
    panic!("Failed to find a suitable memory type.")
}

fn create_buffer(
    device: &ash::Device,
    device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    size: vk::DeviceSize,
    usage_flags: vk::BufferUsageFlags,
    memory_property_flags: vk::MemoryPropertyFlags,
) -> (vk::Buffer, vk::DeviceMemory, vk::DeviceSize) {
    let buffer_info = vk::BufferCreateInfo::builder()
        .size(size)
        .usage(usage_flags)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = unsafe {
        device
            .create_buffer(&buffer_info, None)
            .expect("Failed to create buffer.")
    };

    let memory_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memory_type = find_memory_type(
        memory_requirements,
        device_memory_properties,
        memory_property_flags,
    );

    let alloc_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(memory_requirements.size)
        .memory_type_index(memory_type);
    let buffer_memory = unsafe {
        device
            .allocate_memory(&alloc_info, None)
            .expect("Failed to allocate buffer memory.")
    };

    unsafe {
        device
            .bind_buffer_memory(buffer, buffer_memory, 0)
            .expect("Failed to bind buffer memory.");
    }

    (buffer, buffer_memory, memory_requirements.size)
}

fn copy_buffer(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    transfer_queue: vk::Queue,
    src: vk::Buffer,
    dst: vk::Buffer,
    size: vk::DeviceSize,
) {
    let command_buffers = create_command_buffers(&device, command_pool, 1);
    let command_buffer = command_buffers[0];

    let begin_info =
        vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe {
        device
            .begin_command_buffer(command_buffer, &begin_info)
            .unwrap();
    }

    let copy_region = vk::BufferCopy::builder()
        .src_offset(0)
        .dst_offset(0)
        .size(size)
        .build();
    let regions = [copy_region];
    unsafe { device.cmd_copy_buffer(command_buffer, src, dst, &regions) }

    unsafe {
        device.end_command_buffer(command_buffer).unwrap();
    }

    let submit_info = vk::SubmitInfo::builder()
        .command_buffers(&command_buffers)
        .build();
    let submit_infos = [submit_info];
    unsafe {
        device
            .queue_submit(transfer_queue, &submit_infos, vk::Fence::null())
            .unwrap();
        device.queue_wait_idle(transfer_queue).unwrap();
    }

    unsafe {
        device.free_command_buffers(command_pool, &command_buffers);
    }
}

fn create_device_local_buffer_with_data<A, T: Copy>(
    device: &ash::Device,
    device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    command_pool: vk::CommandPool,
    transfer_queue: vk::Queue,
    buffer_usage_flags: vk::BufferUsageFlags,
    data: &[T],
) -> (vk::Buffer, vk::DeviceMemory) {
    let buffer_size = (std::mem::size_of::<T>() * data.len()) as vk::DeviceSize;

    // TODO(lovew): Instead of creating a buffer here we could have implemented a memory allocator
    // that we would request memory from, and it would give us a chunk of memory that was bound to
    // a buffer and mapped to some host memory.
    let (staging_buffer, staging_buffer_memory, staging_memory_size) = create_buffer(
        device,
        device_memory_properties,
        buffer_size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    );

    unsafe {
        let data_ptr = device
            .map_memory(
                staging_buffer_memory,
                0,
                buffer_size,
                vk::MemoryMapFlags::empty(),
            )
            .expect("Failed to map staging buffer memory.");
        let mut align = ash::util::Align::new(
            data_ptr,
            std::mem::align_of::<A>() as _,
            staging_memory_size,
        );
        align.copy_from_slice(data);
        device.unmap_memory(staging_buffer_memory);
    }

    let (buffer, buffer_memory, _) = create_buffer(
        device,
        device_memory_properties,
        buffer_size,
        vk::BufferUsageFlags::TRANSFER_DST | buffer_usage_flags,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    );

    copy_buffer(
        device,
        command_pool,
        transfer_queue,
        staging_buffer,
        buffer,
        buffer_size,
    );

    unsafe {
        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_buffer_memory, None);
    }

    (buffer, buffer_memory)
}

pub fn create_vertex_buffer(
    device: &ash::Device,
    device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    transfer_queue: vk::Queue,
    command_pool: vk::CommandPool,
    vertices: &[Vertex],
) -> (vk::Buffer, vk::DeviceMemory) {
    create_device_local_buffer_with_data::<f32, _>(
        &device,
        device_memory_properties,
        command_pool,
        transfer_queue,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        vertices,
    )
}

pub fn create_index_buffer(
    device: &ash::Device,
    device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    transfer_queue: vk::Queue,
    command_pool: vk::CommandPool,
    indices: &[u16],
) -> (vk::Buffer, vk::DeviceMemory) {
    create_device_local_buffer_with_data::<u16, _>(
        &device,
        device_memory_properties,
        command_pool,
        transfer_queue,
        vk::BufferUsageFlags::INDEX_BUFFER,
        indices,
    )
}

pub fn create_command_buffers(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    num_command_buffers: u32,
) -> Vec<vk::CommandBuffer> {
    let alloc_info = vk::CommandBufferAllocateInfo::builder()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(num_command_buffers);

    unsafe { device.allocate_command_buffers(&alloc_info).unwrap() }
}

pub fn record_command_buffer(
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    render_pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
    swapchain_extent: vk::Extent2D,
    graphics_pipeline: vk::Pipeline,
    vertex_buffer: vk::Buffer,
    index_buffer: vk::Buffer,
    num_vertex_indices: u32,
) {
    // Begin the command buffer.
    let begin_info = vk::CommandBufferBeginInfo::builder().build();
    unsafe {
        device
            .begin_command_buffer(command_buffer, &begin_info)
            .unwrap()
    };

    let clear_values = [vk::ClearValue {
        color: vk::ClearColorValue {
            float32: [0.0, 0.0, 0.0, 1.0],
        },
    }];
    let render_pass_info = vk::RenderPassBeginInfo::builder()
        .render_pass(render_pass)
        .framebuffer(framebuffer)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain_extent,
        })
        .clear_values(&clear_values);
    unsafe {
        device.cmd_begin_render_pass(
            command_buffer,
            &render_pass_info,
            vk::SubpassContents::INLINE,
        );

        device.cmd_bind_pipeline(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            graphics_pipeline,
        );

        let vertex_buffers = [vertex_buffer];
        let offsets = [0];
        device.cmd_bind_vertex_buffers(command_buffer, 0, &vertex_buffers, &offsets);

        device.cmd_bind_index_buffer(command_buffer, index_buffer, 0, vk::IndexType::UINT16);

        device.cmd_draw_indexed(command_buffer, num_vertex_indices, 1, 0, 0, 0);

        device.cmd_end_render_pass(command_buffer);

        device.end_command_buffer(command_buffer).unwrap();
    }
}

pub fn create_sync_objects(
    device: &ash::Device,
    max_frames_in_flight: u32,
) -> (Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>) {
    let mut image_available_semaphores = Vec::new();
    let mut render_finished_semaphores = Vec::new();
    let mut in_flight_fences = Vec::new();

    for _ in 0..max_frames_in_flight {
        let image_available_semaphore = {
            let semaphore_info = vk::SemaphoreCreateInfo::builder();
            unsafe {
                device
                    .create_semaphore(&semaphore_info, None)
                    .expect("Failed to create semaphore.")
            }
        };
        image_available_semaphores.push(image_available_semaphore);

        let render_finished_semaphore = {
            let semaphore_info = vk::SemaphoreCreateInfo::builder();
            unsafe {
                device
                    .create_semaphore(&semaphore_info, None)
                    .expect("Failed to create semaphore.")
            }
        };
        render_finished_semaphores.push(render_finished_semaphore);

        let in_flight_fence = {
            let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);
            unsafe {
                device
                    .create_fence(&fence_info, None)
                    .expect("Failed to create fence.")
            }
        };
        in_flight_fences.push(in_flight_fence);
    }

    (
        image_available_semaphores,
        render_finished_semaphores,
        in_flight_fences,
    )
}

pub fn create_descriptor_set_layout(device: &ash::Device) -> vk::DescriptorSetLayout {
    let ubo_layout_binding = vk::DescriptorSetLayoutBinding::builder()
        .binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .build();
    let bindings = [ubo_layout_binding];
    let layout_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings);

    unsafe {
        device
            .create_descriptor_set_layout(&layout_info, None)
            .expect("Failed to create descriptor set layout.")
    }
}

pub fn create_uniform_buffers(
    device: &ash::Device,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    num_buffers: u32,
) -> (Vec<vk::Buffer>, Vec<vk::DeviceMemory>) {
    let buffer_size = std::mem::size_of::<UniformBufferObject>() as vk::DeviceSize;

    let mut buffers = Vec::new();
    let mut buffer_memories = Vec::new();

    for _ in 0..num_buffers {
        let (buffer, buffer_memory, _) = create_buffer(
            device,
            memory_properties,
            buffer_size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        buffers.push(buffer);
        buffer_memories.push(buffer_memory);
    }

    (buffers, buffer_memories)
}
