use ash::{Entry, vk};
use ash::version::{EntryV1_0, InstanceV1_0, DeviceV1_0};
use ash::extensions::ext::DebugReport;
use ash::extensions::khr::{Swapchain, Surface};
use ash::vk::Handle;
use ash::vk_make_version;
use std::ffi::{CString, CStr};
use std::os::raw::{c_char, c_void};

use specs::{Builder, Component, NullStorage, System, Read, ReadStorage, WriteStorage, DispatcherBuilder};
use specs_derive::{Component};

use byteorder::{NativeEndian, ByteOrder};

use crate::fy_math::{Vec4, Mat4, TransformComponent};

pub struct RenderContext {
    instance: ash::Instance,
    phys_device: vk::PhysicalDevice,
    device: ash::Device,
    surface: vk::SurfaceKHR,
    mem_allocator: vk_mem::Allocator,
    graphics_queue: vk::Queue,
    swapchain_ext: Swapchain,
    swapchain: vk::SwapchainKHR,
    sc_image_ready_sem: vk::Semaphore,
    render_finished_sem: vk::Semaphore,
    graphics_command_buffer: vk::CommandBuffer,
    sub_command_pools: std::vec::Vec<vk::CommandPool>,
    sub_command_buffers: std::vec::Vec<vk::CommandBuffer>,
    framebuffers: std::vec::Vec<vk::Framebuffer>,
    render_pass: vk::RenderPass,
    graphics_pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    render_area: vk::Rect2D,
    thread_pool: std::sync::Arc<rayon::ThreadPool>
}

#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct RenderComponent;

const PUSH_CONSTANT_SIZE: u32 = std::mem::size_of::<Mat4>() as u32;

unsafe extern "system" fn vulkan_debug_callback(
    _: vk::DebugReportFlagsEXT,
    _: vk::DebugReportObjectTypeEXT,
    _: u64,
    _: usize,
    _: i32,
    _: *const c_char,
    p_message: *const c_char,
    _: *mut c_void
) -> u32 {
    println!("{:?}", CStr::from_ptr(p_message));
    vk::FALSE
}

impl RenderContext {
    pub fn new(window: &sdl2::video::Window, window_size_x: u32, window_size_y: u32, thread_pool: std::sync::Arc<rayon::ThreadPool>, num_threads: usize) -> RenderContext {
        let sdl_vk_exts = window.vulkan_instance_extensions().unwrap();
        let entry = Entry::new().unwrap();

        let instance = {
            let app_name = CString::new("Pong2").unwrap();
            let layer_names = [CString::new("VK_LAYER_LUNARG_standard_validation").unwrap()];
            let layer_names_raw: Vec<*const i8> = layer_names.iter().map(|name| name.as_ptr()).collect();
            let mut extensions_names = vec![DebugReport::name().as_ptr()];

            for ext in sdl_vk_exts.iter() {
                extensions_names.push(ext.as_ptr() as *const i8);
            }

            let appinfo = vk::ApplicationInfo::builder()
                .application_name(&app_name)
                .application_version(0)
                .engine_name(&app_name)
                .engine_version(0)
                .api_version(vk_make_version!(1,0,0));
            let inst_create_info = vk::InstanceCreateInfo::builder()
                .application_info(&appinfo)
                .enabled_layer_names(&layer_names_raw)
                .enabled_extension_names(&extensions_names);

            unsafe { entry.create_instance(&inst_create_info, None).unwrap() }
        };

        //Create a debugging callback function for error handling
        let debug_info = vk::DebugReportCallbackCreateInfoEXT::builder()
            .flags(vk::DebugReportFlagsEXT::ERROR | vk::DebugReportFlagsEXT::WARNING | vk::DebugReportFlagsEXT::PERFORMANCE_WARNING)
            .pfn_callback(Some(vulkan_debug_callback));

        let debug_report_loader = DebugReport::new(&entry, &instance);
        let debug_call_back = unsafe { debug_report_loader.create_debug_report_callback(&debug_info, None).unwrap() };

        //Print out information about available Vulkan devices
        let pdevices = unsafe { instance.enumerate_physical_devices().unwrap() };
        println!("Available devices:");
        for pdev in pdevices.iter() {
            let properties = unsafe { instance.get_physical_device_properties(*pdev) };
            let name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) };
            println!("{:?}", name);
            println!("{:?}", properties.limits.point_size_range);
        }

        let physical_device = pdevices[0];

        let inst_handle = instance.handle().as_raw() as usize;
        let surface_ext = Surface::new(&entry, &instance);
        let surface: vk::SurfaceKHR = vk::Handle::from_raw(window.vulkan_create_surface(inst_handle).unwrap());
        let _surface_caps = unsafe { surface_ext.get_physical_device_surface_capabilities(physical_device, surface).unwrap() };
        let surface_formats = unsafe { surface_ext.get_physical_device_surface_formats(physical_device, surface).unwrap() };
        let _surface_present_modes = unsafe { surface_ext.get_physical_device_surface_present_modes(physical_device, surface).unwrap() };

        let queue_props = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let mut graphics_queue_family_index = std::u32::MAX;

        for (i, queue) in queue_props.iter().enumerate() {
            let supports_present = unsafe { surface_ext.get_physical_device_surface_support(physical_device, i as u32, surface) };
            if queue.queue_flags.contains(vk::QueueFlags::GRAPHICS) && supports_present {
                graphics_queue_family_index = i as u32;
                break;
            }
        }

        assert!(graphics_queue_family_index != std::u32::MAX, "No graphics queue family found!");

        let priorities = [1.0];

        //Device queues must be specified when creating the device
        let queue_infos = vec![vk::DeviceQueueCreateInfo::builder()
                            .queue_family_index(graphics_queue_family_index)
                            .queue_priorities(&priorities)
                            .build()];

        let device_extensions = [Swapchain::name().as_ptr()];
        let device_create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&device_extensions);
        let device = unsafe { instance.create_device(physical_device, &device_create_info, None).unwrap() };

        let allocator = {
            let create_info = vk_mem::AllocatorCreateInfo {
                physical_device,
                device: device.clone(),
                instance: instance.clone(),
                ..Default::default()
            };
            vk_mem::Allocator::new(&create_info).unwrap()
        };

        let graphics_queue = unsafe { device.get_device_queue(graphics_queue_family_index, 0) };

        let command_pool = {
            let create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(graphics_queue_family_index);
            unsafe { device.create_command_pool(&create_info, None).unwrap() }
        };

        let sub_command_pools = {
            let mut ret = Vec::new();
            let pool_create = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(graphics_queue_family_index);

            for _ in 0..num_threads {
                let pool = unsafe { device.create_command_pool(&pool_create, None).unwrap() };
                ret.push(pool);
            }

            ret
        };

        let swapchain_ext = Swapchain::new(&instance, &device);

        let swapchain = {
            let create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(2)
                .image_format(surface_formats[0].format)           //This method picks the first available format and color space
                .image_color_space(surface_formats[0].color_space) 
                .image_extent(vk::Extent2D::builder().width(window_size_x).height(window_size_y).build())
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(vk::PresentModeKHR::MAILBOX) //FIFO is guaranteed to be available
                .clipped(true);
            unsafe { swapchain_ext.create_swapchain(&create_info, None).unwrap() }
        };

        let render_pass = {

            //An attachment description describes the layout of the rendering attachment
            let attachment = [vk::AttachmentDescription::builder()
                .format(surface_formats[0].format) //Use the same format as the swapchain images
                .samples(vk::SampleCountFlags::TYPE_1) //No multisampling
                .load_op(vk::AttachmentLoadOp::CLEAR) //Clear this image when the render pass begins (clear value is specified later)
                .store_op(vk::AttachmentStoreOp::STORE) //Store this image at the end of rendering to present
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE) //No depth/stencil is used, so these can be dont care
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED) //This app doesn't read from the attachment, so this specifies the data is unknown
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR) //This layout is what the image will be moved to once the render pass ends
                .build()];

            //Attachment references describe the layout that each attachment should be in when the subpass begins
            let attach_refs = [vk::AttachmentReference::builder()
                .attachment(0)
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .build()];

            //Each renderpass is a collection of subpasses. This app only uses one pass to render
            let subpasses = [vk::SubpassDescription::builder()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&attach_refs)
                .build()];

            //Subpass dependencies specify memory dependencies that must happen during subpass transitions
            //Image layout transitions normally automatically occur before the subpass begins using the layouts in
            //AttachmentDescription's and AttachmentReference's
            //For presentation, however, the swapchain image is usually not acquired yet, so this dependency
            //moves the layout transition to COLOR_ATTACHMENT to before the actual COLOR_ATTACHMENT_OUTPUT actually occurs
            let present_dependency = vk::SubpassDependency::builder()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT) 
                .src_access_mask(vk::AccessFlags::empty()) //Nothing needs to be waited on for the image to transition
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT) //Writes occur in the COLOR_ATTACHMENT_OUTPUT stage
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE) //Must transition image before writing to it
                .build();

            let dependencies = [present_dependency];

            //Build the render pass
            let create_info = vk::RenderPassCreateInfo::builder()
                .attachments(&attachment)
                .subpasses(&subpasses)
                .dependencies(&dependencies)
                .build();
            unsafe { device.create_render_pass(&create_info, None).unwrap() }
        };

        //Get handles to the actual swapchain images
        let swapchain_images = unsafe { swapchain_ext.get_swapchain_images(swapchain).unwrap() };

        //Create image views and framebuffers
        let mut swapchain_image_views = Vec::new();
        let mut framebuffers = Vec::new();

        for (i, image) in swapchain_images.iter().enumerate() {
            //Image views describe access on a subset of an image resource (i.e. a few mipmap layers)
            //As the swapchain images should not use mipmapping and aren't array images, the image view should cover the entire image
            let create_info = vk::ImageViewCreateInfo::builder()
                .image(*image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(surface_formats[0].format)
                .components(vk::ComponentMapping::builder().r(vk::ComponentSwizzle::IDENTITY).g(vk::ComponentSwizzle::IDENTITY).b(vk::ComponentSwizzle::IDENTITY).a(vk::ComponentSwizzle::IDENTITY).build())
                .subresource_range(vk::ImageSubresourceRange::builder()
                                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                                    .base_mip_level(0)
                                    .level_count(1)
                                    .base_array_layer(0)
                                    .layer_count(1)
                                    .build());
            let iv = unsafe { device.create_image_view(&create_info, None).unwrap() };
            swapchain_image_views.push(iv);

            //Framebuffers specify a particular image view to use as an attachment. These will be used with the render pass created above
            let attachments = [swapchain_image_views[i]];

            let create_info = vk::FramebufferCreateInfo::builder()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(window_size_x)
                .height(window_size_y)
                .layers(1)
                .build();

            let fb = unsafe { device.create_framebuffer(&create_info, None).unwrap() };

            framebuffers.push(fb);
        }

        //A pipeline layout is a collection of all of the descriptor set layouts and push constants that will be used in a single pipeline
        let pipeline_layout = {
            let push_constant_range = [vk::PushConstantRange::builder()
                .stage_flags(vk::ShaderStageFlags::VERTEX)
                .offset(0)
                .size(PUSH_CONSTANT_SIZE)
                .build()];
            let create_info = vk::PipelineLayoutCreateInfo::builder()
                .push_constant_ranges(&push_constant_range)
                .build();
            unsafe { device.create_pipeline_layout(&create_info, None).unwrap() }
        };

        //Create a graphics pipeline around the vertex and fragment shaders
        let graphics_pipeline = {
            let f_spv = include_bytes!("../frag.spv");
            let v_spv = include_bytes!("../vert.spv");

            let mut f_code = vec![0; f_spv.len() / 4];
            let mut v_code = vec![0; v_spv.len() / 4];

            NativeEndian::read_u32_into(v_spv, v_code.as_mut_slice());
            NativeEndian::read_u32_into(f_spv, f_code.as_mut_slice());

            let create_info = vk::ShaderModuleCreateInfo::builder()
                .code(f_code.as_slice())
                .build();
            let f_mod = unsafe { device.create_shader_module(&create_info, None).unwrap() };

            let create_info = vk::ShaderModuleCreateInfo::builder()
                .code(v_code.as_slice())
                .build();
            let v_mod = unsafe { device.create_shader_module(&create_info, None).unwrap() };

            let entrypoint = CString::new("main").unwrap();
            let v_stage = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(v_mod)
                .name(&entrypoint)
                .build();
            let f_stage = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(f_mod)
                .name(&entrypoint)
                .build();

            //Points are read from a storage buffer, so no vertex input is necessary
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::builder().build();

            //Verticies will be drawn as points
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
                .topology(vk::PrimitiveTopology::POINT_LIST)
                .primitive_restart_enable(false)
                .build();

            //Standard fill for rasterization
            let raster_state = vk::PipelineRasterizationStateCreateInfo::builder()
                .depth_clamp_enable(false)
                .rasterizer_discard_enable(false)
                .polygon_mode(vk::PolygonMode::FILL)
                .cull_mode(vk::CullModeFlags::NONE)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .depth_bias_enable(false)
                .line_width(1.0)
                .build();

            //Viewport and scissor cover the entire screen
            let viewport = [vk::Viewport::builder()
                .x(0.0)
                .y(0.0)
                .width(window_size_x as f32)
                .height(window_size_y as f32)
                .min_depth(0.0)
                .max_depth(1.0)
                .build()];
            let scissor = [vk::Rect2D::builder()
                .offset(vk::Offset2D::builder().x(0).y(0).build())
                .extent(vk::Extent2D::builder().width(window_size_x).height(window_size_y).build())
                .build()];

            let view_state = vk::PipelineViewportStateCreateInfo::builder()
                .viewports(&viewport)
                .scissors(&scissor)
                .build();

            //No multisampling
            let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1)
                .sample_shading_enable(false)
                .alpha_to_coverage_enable(false)
                .alpha_to_one_enable(false)
                .build();

            //No blending
            let blend_attachment = [vk::PipelineColorBlendAttachmentState::builder()
                .blend_enable(false)
                .color_write_mask(vk::ColorComponentFlags::all())
                .build()];

            let blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
                .logic_op_enable(false)
                .attachments(&blend_attachment)
                .build();

            let stages = [v_stage, f_stage];

            let create_info = [vk::GraphicsPipelineCreateInfo::builder()
                .stages(&stages)
                .vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly)
                .viewport_state(&view_state)
                .rasterization_state(&raster_state)
                .multisample_state(&multisample_state)
                .color_blend_state(&blend_state)
                .render_pass(render_pass)
                .subpass(0)
                .layout(pipeline_layout)
                .build()];
            let pipelines = unsafe { device.create_graphics_pipelines(vk::PipelineCache::null(), &create_info, None).unwrap() };
            unsafe {
                device.destroy_shader_module(v_mod, None);
                device.destroy_shader_module(f_mod, None);
            }
            pipelines[0]
        };

        let graphics_command_buffer = {
            let alloc_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);

            let buffers = unsafe { device.allocate_command_buffers(&alloc_info).unwrap() };
            buffers[0]
        };

        let sub_command_buffers = {
            let mut ret = Vec::new();

            for thread_idx in 0..num_threads {
                let alloc_info = vk::CommandBufferAllocateInfo::builder()
                    .command_pool(sub_command_pools[thread_idx])
                    .level(vk::CommandBufferLevel::SECONDARY)
                    .command_buffer_count(1);

                let buffers = unsafe { device.allocate_command_buffers(&alloc_info).unwrap() };
                ret.push(buffers[0]);
            }
            ret
        };

        let (sc_image_ready_sem, render_finished_sem) = {
            let create_info = vk::SemaphoreCreateInfo::builder().build();
            unsafe { (device.create_semaphore(&create_info, None).unwrap(), device.create_semaphore(&create_info, None).unwrap()) }
        };

        let render_area = vk::Rect2D::builder()
            .offset(vk::Offset2D::builder().x(0).y(0).build())
            .extent(vk::Extent2D::builder().width(window_size_x).height(window_size_y).build())
            .build();

        RenderContext {
            instance,
            phys_device: physical_device,
            device,
            surface,
            mem_allocator: allocator,
            graphics_queue,
            swapchain_ext,
            swapchain,
            sc_image_ready_sem,
            render_finished_sem,
            graphics_command_buffer,
            sub_command_buffers,
            sub_command_pools,
            framebuffers,
            render_pass,
            graphics_pipeline,
            render_area,
            pipeline_layout,
            thread_pool
        }
    }
}

impl <'a> System<'a> for RenderContext {
    type SystemData = (ReadStorage<'a, RenderComponent>, ReadStorage<'a, TransformComponent>);

    fn run (&mut self, (render_storage, transform_storage): Self::SystemData) {
        use specs::ParJoin;
        use rayon::prelude::*;

        unsafe { self.device.device_wait_idle().unwrap() };

        let (fb_idx, _) = unsafe { self.swapchain_ext.acquire_next_image(self.swapchain, std::u64::MAX, self.sc_image_ready_sem, vk::Fence::null()).unwrap() };

        for sub_cmd_bfr in self.sub_command_buffers.iter() {
            let inheritance_info = vk::CommandBufferInheritanceInfo::builder()
                .render_pass(self.render_pass)
                .subpass(0)
                .framebuffer(self.framebuffers[fb_idx as usize]);

            let begin_info = vk::CommandBufferBeginInfo::builder()
                .inheritance_info(&inheritance_info)
                .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE);

            unsafe { self.device.begin_command_buffer(*sub_cmd_bfr, &begin_info).unwrap(); }
        }

        (&render_storage, &transform_storage).par_join().for_each(|(_, transform)| {
            let idx = match self.thread_pool.current_thread_index() {
                None => {
                    panic!("Rendering operations occured outside thread pool!");
                },
                Some(idx) => {
                    idx
                }
            };

            let position = &transform.position;
            let x = Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 0.0
            };
            let y = Vec4 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                w: 0.0
            };
            let z = Vec4 {
                x: 0.0, 
                y: 0.0,
                z: 1.0,
                w: 0.0
            };
            let w = Vec4 {
                x: position.x,
                y: position.y,
                z: 0.0,
                w: 1.0
            };
            let m = Mat4 {
                x,
                y,
                z,
                w
            };
            
            unsafe {
                let ptr = &m as *const Mat4;
                let slice = std::slice::from_raw_parts(ptr as *const u8, PUSH_CONSTANT_SIZE as usize);
                self.device.cmd_bind_pipeline(self.sub_command_buffers[idx], vk::PipelineBindPoint::GRAPHICS, self.graphics_pipeline);
                self.device.cmd_push_constants(self.sub_command_buffers[idx], self.pipeline_layout, vk::ShaderStageFlags::VERTEX, 0, &slice);
                self.device.cmd_draw(self.sub_command_buffers[idx], 1, 1, 0, 0);
            }
        });

        for sub_cmd_bfr in self.sub_command_buffers.iter() {
            unsafe { self.device.end_command_buffer(*sub_cmd_bfr).unwrap(); }
        }
        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe { self.device.begin_command_buffer(self.graphics_command_buffer, &begin_info).unwrap() };
        let clear_value = vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0]};
        let clear_value = [vk::ClearValue { color: clear_value}];
        let rp_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(self.render_pass)
            .framebuffer(self.framebuffers[fb_idx as usize])
            .render_area(self.render_area)
            .clear_values(&clear_value)
            .build();

        unsafe {
            self.device.cmd_begin_render_pass(self.graphics_command_buffer, &rp_begin_info, vk::SubpassContents::SECONDARY_COMMAND_BUFFERS); 
            self.device.cmd_execute_commands(self.graphics_command_buffer, self.sub_command_buffers.as_slice());
            self.device.cmd_end_render_pass(self.graphics_command_buffer);
            self.device.end_command_buffer(self.graphics_command_buffer).unwrap();
        }

        let wait_semaphores = [self.sc_image_ready_sem];
        let dst_stage_mask = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let cmd_buffers = [self.graphics_command_buffer];
        let signal_semaphores = [self.render_finished_sem];

        let submit  = [vk::SubmitInfo::builder()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&dst_stage_mask)
            .command_buffers(&cmd_buffers)
            .signal_semaphores(&signal_semaphores)
            .build()];
        unsafe { self.device.queue_submit(self.graphics_queue, &submit, vk::Fence::null()).unwrap() };

        let wait_semaphores = [self.render_finished_sem];
        let swapchains = [self.swapchain];
        let image_indices = [fb_idx];

        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices)
            .build();
        unsafe { self.swapchain_ext.queue_present(self.graphics_queue, &present_info).unwrap() };
    }
}