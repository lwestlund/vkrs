use ash::vk;

use crate::vkrs::queue_family_indices::QueueFamilyIndices;

pub struct SupportDetails {
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
}

pub struct SwapchainProperties {
    pub surface_format: vk::SurfaceFormatKHR,
    pub present_mode: vk::PresentModeKHR,
    pub extent: vk::Extent2D,
}

impl SupportDetails {
    pub fn new(
        device: vk::PhysicalDevice,
        surface_fn: &ash::extensions::khr::Surface,
        surface: vk::SurfaceKHR,
    ) -> Self {
        let capabilities = unsafe {
            surface_fn
                .get_physical_device_surface_capabilities(device, surface)
                .unwrap()
        };

        let formats = unsafe {
            surface_fn
                .get_physical_device_surface_formats(device, surface)
                .unwrap()
        };

        let present_modes = unsafe {
            surface_fn
                .get_physical_device_surface_present_modes(device, surface)
                .unwrap()
        };

        Self {
            capabilities,
            formats,
            present_modes,
        }
    }

    pub fn get_ideal_swapchain_properties(
        &self,
        window_size: &winit::dpi::PhysicalSize<u32>,
    ) -> SwapchainProperties {
        let surface_format = Self::choose_swapchain_surface_format(&self.formats);
        let present_mode = Self::choose_swapchain_present_mode(&self.present_modes);
        let extent = Self::choose_swapchain_extent(self.capabilities, window_size);
        SwapchainProperties {
            surface_format,
            present_mode,
            extent,
        }
    }

    fn choose_swapchain_surface_format(
        available_formats: &[vk::SurfaceFormatKHR],
    ) -> vk::SurfaceFormatKHR {
        *available_formats
            .iter()
            .find(|format| {
                format.format == vk::Format::B8G8R8A8_SRGB
                    && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or(&available_formats[0])
    }

    fn choose_swapchain_present_mode(
        available_present_modes: &[vk::PresentModeKHR],
    ) -> vk::PresentModeKHR {
        if available_present_modes.contains(&vk::PresentModeKHR::MAILBOX) {
            return vk::PresentModeKHR::MAILBOX;
        }
        // FIFO is guaranteed to exist, so it is used as fallback.
        vk::PresentModeKHR::FIFO
    }

    fn choose_swapchain_extent(
        capabilities: vk::SurfaceCapabilitiesKHR,
        window_size: &winit::dpi::PhysicalSize<u32>,
    ) -> vk::Extent2D {
        if capabilities.current_extent.width != std::u32::MAX {
            return capabilities.current_extent;
        }

        let min_extent = capabilities.min_image_extent;
        let max_extent = capabilities.max_image_extent;
        let width = window_size.width.clamp(min_extent.width, max_extent.width);
        let height = window_size
            .height
            .clamp(min_extent.height, max_extent.height);
        vk::Extent2D { width, height }
    }
}

pub fn create_swapchain_and_images(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    surface_fn: &ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
    window_size: &winit::dpi::PhysicalSize<u32>,
) -> (
    ash::extensions::khr::Swapchain,
    vk::SwapchainKHR,
    vk::Format,
    vk::Extent2D,
    Vec<vk::Image>,
) {
    let swapchain_support_details = SupportDetails::new(physical_device, surface_fn, surface);
    let properties = swapchain_support_details.get_ideal_swapchain_properties(window_size);
    let image_count = {
        let mut preferred_num_images = swapchain_support_details.capabilities.min_image_count + 1;
        let max_num_images = swapchain_support_details.capabilities.max_image_count;
        let unlimited_max_num_images = max_num_images == 0;
        if !unlimited_max_num_images && preferred_num_images > max_num_images {
            preferred_num_images = max_num_images;
        }
        preferred_num_images
    };

    log::debug!(target: "vulkan",
                concat!("Creating swapchain:\n",
                        "\tFormat: {:?}\n",
                        "\tColor space: {:?}\n",
                        "\tPresent mode: {:?}\n",
                        "\tExtent: {:?}\n",
                        "\tImage count: {}"),
                properties.surface_format.format,
                properties.surface_format.color_space,
                properties.present_mode,
                properties.extent,
                image_count);

    let queue_indices =
        QueueFamilyIndices::find_queue_families(instance, surface_fn, surface, physical_device);
    let graphics_index = queue_indices.graphics_family.unwrap();
    let present_index = queue_indices.present_family.unwrap();
    let queue_family_indices = [graphics_index, present_index];

    let create_info = {
        let mut builder = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(properties.surface_format.format)
            .image_color_space(properties.surface_format.color_space)
            .image_extent(properties.extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT);

        builder = match graphics_index == present_index {
            true => builder.image_sharing_mode(vk::SharingMode::EXCLUSIVE),
            _ => builder
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&queue_family_indices),
        };

        builder
            .pre_transform(swapchain_support_details.capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(properties.present_mode)
            .clipped(true)
        // TODO(lovew): There is no old swapchain at the moment since we disallow
        // resizing of the window.
        // .old_swapchain(old_swapchain)
    };

    let swapchain_fn = ash::extensions::khr::Swapchain::new(instance, device);
    let swapchain = unsafe { swapchain_fn.create_swapchain(&create_info, None).unwrap() };
    let images = unsafe { swapchain_fn.get_swapchain_images(swapchain).unwrap() };
    (
        swapchain_fn,
        swapchain,
        properties.surface_format.format,
        properties.extent,
        images,
    )
}

pub fn create_image_views(
    device: &ash::Device,
    swapchain_images: &[vk::Image],
    swapchain_image_format: vk::Format,
) -> Vec<vk::ImageView> {
    swapchain_images
        .iter()
        .map(|image| {
            let create_info = vk::ImageViewCreateInfo::builder()
                .image(*image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(swapchain_image_format)
                .components(vk::ComponentMapping {
                    r: vk::ComponentSwizzle::IDENTITY,
                    g: vk::ComponentSwizzle::IDENTITY,
                    b: vk::ComponentSwizzle::IDENTITY,
                    a: vk::ComponentSwizzle::IDENTITY,
                })
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            unsafe { device.create_image_view(&create_info, None).unwrap() }
        })
        .collect::<Vec<_>>()
}
