#![allow(dead_code)]
#[macro_use]
extern crate ash;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(windows)]
extern crate user32;
#[cfg(windows)]
extern crate winapi;
extern crate winit;
//#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
//extern crate glsl_to_spirv;
extern crate gfx_hal as hal;
extern crate notify;
use ash::extensions::{DebugReport, Surface, Swapchain, Win32Surface, XlibSurface};
use ash::util::Align;
pub use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0, V1_0};
use ash::vk;
use ash::Device;
use ash::Entry;
use ash::Instance;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::cell::{RefCell, RefMut};
use std::default::Default;
use std::ffi::{CStr, CString};
use std::fs::{read_dir, File};
use std::io::Read;
use std::marker::PhantomData;
use std::mem;
use std::mem::align_of;
use std::ops::DerefMut;
use std::ops::Drop;
use std::path::Path;
use std::ptr;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, SystemTime};

// Simple offset_of macro akin to C++ offsetof
#[macro_export]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = mem::uninitialized();
            (&b.$field as *const _ as isize) - (&b as *const _ as isize)
        }
    }};
}

#[derive(Clone, Debug, Copy)]
pub struct Vertex {
    pub pos: [f32; 4],
    pub uv: [f32; 2],
}

#[derive(Clone, Debug, Copy)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

pub struct Quad {
    base: ExampleBase,
    uniform_time_buffer: Buffer<f32>,
    renderpass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    pipeline_layout: vk::PipelineLayout,
    descriptor_sets: Vec<vk::DescriptorSet>,
    vertex_input_buffer: vk::Buffer,
    index_buffer: vk::Buffer,
    pipeline: GraphicsPipeline,
}
impl Drop for Quad {
    fn drop(&mut self) {
        // base.device.device_wait_idle().unwrap();

        // for pipeline in graphics_pipelines {
        //     base.device.destroy_pipeline(pipeline, None);
        // }
        // base.device.destroy_pipeline_layout(pipeline_layout, None);
        // base.device.destroy_shader_module(shader_module, None);
        // base.device.free_memory(index_buffer_memory, None);
        // base.device.destroy_buffer(index_buffer, None);
        // // base.device.free_memory(uniform_color_buffer_memory, None);
        // // base.device.destroy_buffer(uniform_color_buffer, None);
        // base.device.free_memory(vertex_input_buffer_memory, None);
        // base.device.destroy_buffer(vertex_input_buffer, None);
        // for &descriptor_set_layout in desc_set_layouts.iter() {
        //     base.device
        //         .destroy_descriptor_set_layout(descriptor_set_layout, None);
        // }
        // base.device.destroy_descriptor_pool(descriptor_pool, None);
        // for framebuffer in framebuffers {
        //     base.device.destroy_framebuffer(framebuffer, None);
        // }
        // base.device.destroy_render_pass(renderpass, None);
    }
}

fn load_shader(path: &Path, device: &Device<V1_0>) -> vk::ShaderModule {
    unsafe {
        println!("{}", path.display());
        let spv_file = File::open(path).expect("Could not find vert.spv.");

        let spv_bytes: Vec<u8> = spv_file.bytes().filter_map(|byte| byte.ok()).collect();
        println!("{}", spv_bytes.len());

        let shader_info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::ShaderModuleCreateInfo,
            p_next: ptr::null(),
            flags: Default::default(),
            code_size: spv_bytes.len(),
            p_code: spv_bytes.as_ptr() as *const u32,
        };
        device
            .create_shader_module(&shader_info, None)
            .expect("Fragment shader module error")
    }
}
pub struct GraphicsPipeline {
    renderpass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    viewports: Vec<vk::Viewport>,
    scissors: Vec<vk::Rect2D>,
    desc_set_layouts: Vec<vk::DescriptorSetLayout>,
}
impl GraphicsPipeline {
    pub fn create(
        &self,
        device: &Device<V1_0>,
        vertex_info: (&str, &Path),
        fragment_info: (&str, &Path),
    ) -> vk::Pipeline {
        let vertex_entry_name = CString::new(vertex_info.0).expect("vertex name");
        let frag_entry_name = CString::new(fragment_info.0).expect("frag name");
        let shader_module_vertex = load_shader(vertex_info.1, device);
        let shader_module = load_shader(fragment_info.1, &device);
        let shader_stage_create_infos = [
            vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                module: shader_module_vertex,
                p_name: vertex_entry_name.as_ptr(),
                p_specialization_info: ptr::null(),
                stage: vk::SHADER_STAGE_VERTEX_BIT,
            },
            vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                module: shader_module,
                p_name: frag_entry_name.as_ptr(),
                p_specialization_info: ptr::null(),
                stage: vk::SHADER_STAGE_FRAGMENT_BIT,
            },
        ];
        let viewport_state_info = vk::PipelineViewportStateCreateInfo {
            s_type: vk::StructureType::PipelineViewportStateCreateInfo,
            p_next: ptr::null(),
            flags: Default::default(),
            scissor_count: self.scissors.len() as u32,
            p_scissors: self.scissors.as_ptr(),
            viewport_count: self.viewports.len() as u32,
            p_viewports: self.viewports.as_ptr(),
        };
        let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
            s_type: vk::StructureType::PipelineRasterizationStateCreateInfo,
            p_next: ptr::null(),
            flags: Default::default(),
            cull_mode: vk::CULL_MODE_NONE,
            depth_bias_clamp: 0.0,
            depth_bias_constant_factor: 0.0,
            depth_bias_enable: 0,
            depth_bias_slope_factor: 0.0,
            depth_clamp_enable: 0,
            front_face: vk::FrontFace::CounterClockwise,
            line_width: 1.0,
            polygon_mode: vk::PolygonMode::Fill,
            rasterizer_discard_enable: 0,
        };
        let multisample_state_info = vk::PipelineMultisampleStateCreateInfo {
            s_type: vk::StructureType::PipelineMultisampleStateCreateInfo,
            flags: Default::default(),
            p_next: ptr::null(),
            rasterization_samples: vk::SAMPLE_COUNT_1_BIT,
            sample_shading_enable: 0,
            min_sample_shading: 0.0,
            p_sample_mask: ptr::null(),
            alpha_to_one_enable: 0,
            alpha_to_coverage_enable: 0,
        };
        let noop_stencil_state = vk::StencilOpState {
            fail_op: vk::StencilOp::Keep,
            pass_op: vk::StencilOp::Keep,
            depth_fail_op: vk::StencilOp::Keep,
            compare_op: vk::CompareOp::Always,
            compare_mask: 0,
            write_mask: 0,
            reference: 0,
        };
        let depth_state_info = vk::PipelineDepthStencilStateCreateInfo {
            s_type: vk::StructureType::PipelineDepthStencilStateCreateInfo,
            p_next: ptr::null(),
            flags: Default::default(),
            depth_test_enable: 1,
            depth_write_enable: 1,
            depth_compare_op: vk::CompareOp::LessOrEqual,
            depth_bounds_test_enable: 0,
            stencil_test_enable: 0,
            front: noop_stencil_state.clone(),
            back: noop_stencil_state.clone(),
            max_depth_bounds: 1.0,
            min_depth_bounds: 0.0,
        };
        let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState {
            blend_enable: 0,
            src_color_blend_factor: vk::BlendFactor::SrcColor,
            dst_color_blend_factor: vk::BlendFactor::OneMinusDstColor,
            color_blend_op: vk::BlendOp::Add,
            src_alpha_blend_factor: vk::BlendFactor::Zero,
            dst_alpha_blend_factor: vk::BlendFactor::Zero,
            alpha_blend_op: vk::BlendOp::Add,
            color_write_mask: vk::ColorComponentFlags::all(),
        }];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
            s_type: vk::StructureType::PipelineColorBlendStateCreateInfo,
            p_next: ptr::null(),
            flags: Default::default(),
            logic_op_enable: 0,
            logic_op: vk::LogicOp::Clear,
            attachment_count: color_blend_attachment_states.len() as u32,
            p_attachments: color_blend_attachment_states.as_ptr(),
            blend_constants: [0.0, 0.0, 0.0, 0.0],
        };
        let dynamic_state = [vk::DynamicState::Viewport, vk::DynamicState::Scissor];
        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo {
            s_type: vk::StructureType::PipelineDynamicStateCreateInfo,
            p_next: ptr::null(),
            flags: Default::default(),
            dynamic_state_count: dynamic_state.len() as u32,
            p_dynamic_states: dynamic_state.as_ptr(),
        };
        let shader_stage_create_infos = [
            vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                module: shader_module_vertex,
                p_name: vertex_entry_name.as_ptr(),
                p_specialization_info: ptr::null(),
                stage: vk::SHADER_STAGE_VERTEX_BIT,
            },
            vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PipelineShaderStageCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                module: shader_module,
                p_name: frag_entry_name.as_ptr(),
                p_specialization_info: ptr::null(),
                stage: vk::SHADER_STAGE_FRAGMENT_BIT,
            },
        ];
        let vertex_input_binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::Vertex,
        }];
        let vertex_input_attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32g32b32a32Sfloat,
                offset: offset_of!(Vertex, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32g32Sfloat,
                offset: offset_of!(Vertex, uv) as u32,
            },
        ];
        let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo {
            s_type: vk::StructureType::PipelineVertexInputStateCreateInfo,
            p_next: ptr::null(),
            flags: Default::default(),
            vertex_attribute_description_count: vertex_input_attribute_descriptions.len() as u32,
            p_vertex_attribute_descriptions: vertex_input_attribute_descriptions.as_ptr(),
            vertex_binding_description_count: vertex_input_binding_descriptions.len() as u32,
            p_vertex_binding_descriptions: vertex_input_binding_descriptions.as_ptr(),
        };
        let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo {
            s_type: vk::StructureType::PipelineInputAssemblyStateCreateInfo,
            flags: Default::default(),
            p_next: ptr::null(),
            primitive_restart_enable: 0,
            topology: vk::PrimitiveTopology::TriangleList,
        };
        let graphic_pipeline_info = vk::GraphicsPipelineCreateInfo {
            s_type: vk::StructureType::GraphicsPipelineCreateInfo,
            p_next: ptr::null(),
            flags: vk::PipelineCreateFlags::empty(),
            stage_count: shader_stage_create_infos.len() as u32,
            p_stages: shader_stage_create_infos.as_ptr(),
            p_vertex_input_state: &vertex_input_state_info,
            p_input_assembly_state: &vertex_input_assembly_state_info,
            p_tessellation_state: ptr::null(),
            p_viewport_state: &viewport_state_info,
            p_rasterization_state: &rasterization_info,
            p_multisample_state: &multisample_state_info,
            p_depth_stencil_state: &depth_state_info,
            p_color_blend_state: &color_blend_state,
            p_dynamic_state: &dynamic_state_info,
            layout: self.pipeline_layout,
            render_pass: self.renderpass,
            subpass: 0,
            base_pipeline_handle: vk::Pipeline::null(),
            base_pipeline_index: 0,
        };
        unsafe {
            let graphics_pipelines = device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &[graphic_pipeline_info],
                    None,
                )
                .expect("pipeline");

            graphics_pipelines[0]
        }
    }
}
impl Quad {
    pub fn new() -> Quad {
        unsafe {
            let base = ExampleBase::new(1920, 1080);
            let renderpass_attachments = [
                vk::AttachmentDescription {
                    format: base.surface_format.format,
                    flags: vk::AttachmentDescriptionFlags::empty(),
                    samples: vk::SAMPLE_COUNT_1_BIT,
                    load_op: vk::AttachmentLoadOp::Clear,
                    store_op: vk::AttachmentStoreOp::Store,
                    stencil_load_op: vk::AttachmentLoadOp::DontCare,
                    stencil_store_op: vk::AttachmentStoreOp::DontCare,
                    initial_layout: vk::ImageLayout::Undefined,
                    final_layout: vk::ImageLayout::PresentSrcKhr,
                },
                vk::AttachmentDescription {
                    format: vk::Format::D16Unorm,
                    flags: vk::AttachmentDescriptionFlags::empty(),
                    samples: vk::SAMPLE_COUNT_1_BIT,
                    load_op: vk::AttachmentLoadOp::Clear,
                    store_op: vk::AttachmentStoreOp::DontCare,
                    stencil_load_op: vk::AttachmentLoadOp::DontCare,
                    stencil_store_op: vk::AttachmentStoreOp::DontCare,
                    initial_layout: vk::ImageLayout::DepthStencilAttachmentOptimal,
                    final_layout: vk::ImageLayout::DepthStencilAttachmentOptimal,
                },
            ];
            let color_attachment_ref = vk::AttachmentReference {
                attachment: 0,
                layout: vk::ImageLayout::ColorAttachmentOptimal,
            };
            let depth_attachment_ref = vk::AttachmentReference {
                attachment: 1,
                layout: vk::ImageLayout::DepthStencilAttachmentOptimal,
            };
            let dependency = vk::SubpassDependency {
                dependency_flags: Default::default(),
                src_subpass: vk::VK_SUBPASS_EXTERNAL,
                dst_subpass: Default::default(),
                src_stage_mask: vk::PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
                src_access_mask: Default::default(),
                dst_access_mask: vk::ACCESS_COLOR_ATTACHMENT_READ_BIT
                    | vk::ACCESS_COLOR_ATTACHMENT_WRITE_BIT,
                dst_stage_mask: vk::PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
            };
            let subpass = vk::SubpassDescription {
                color_attachment_count: 1,
                p_color_attachments: &color_attachment_ref,
                p_depth_stencil_attachment: &depth_attachment_ref,
                flags: Default::default(),
                pipeline_bind_point: vk::PipelineBindPoint::Graphics,
                input_attachment_count: 0,
                p_input_attachments: ptr::null(),
                p_resolve_attachments: ptr::null(),
                preserve_attachment_count: 0,
                p_preserve_attachments: ptr::null(),
            };
            let renderpass_create_info = vk::RenderPassCreateInfo {
                s_type: vk::StructureType::RenderPassCreateInfo,
                flags: Default::default(),
                p_next: ptr::null(),
                attachment_count: renderpass_attachments.len() as u32,
                p_attachments: renderpass_attachments.as_ptr(),
                subpass_count: 1,
                p_subpasses: &subpass,
                dependency_count: 1,
                p_dependencies: &dependency,
            };
            let renderpass = base
                .device
                .create_render_pass(&renderpass_create_info, None)
                .unwrap();
            let framebuffers: Vec<vk::Framebuffer> = base
                .present_image_views
                .iter()
                .map(|&present_image_view| {
                    let framebuffer_attachments = [present_image_view, base.depth_image_view];
                    let frame_buffer_create_info = vk::FramebufferCreateInfo {
                        s_type: vk::StructureType::FramebufferCreateInfo,
                        p_next: ptr::null(),
                        flags: Default::default(),
                        render_pass: renderpass,
                        attachment_count: framebuffer_attachments.len() as u32,
                        p_attachments: framebuffer_attachments.as_ptr(),
                        width: base.surface_resolution.width,
                        height: base.surface_resolution.height,
                        layers: 1,
                    };
                    base.device
                        .create_framebuffer(&frame_buffer_create_info, None)
                        .unwrap()
                })
                .collect();
            let index_buffer_data = [0u32, 1, 2, 2, 3, 0];
            let index_buffer_info = vk::BufferCreateInfo {
                s_type: vk::StructureType::BufferCreateInfo,
                p_next: ptr::null(),
                flags: vk::BufferCreateFlags::empty(),
                size: std::mem::size_of_val(&index_buffer_data) as u64,
                usage: vk::BUFFER_USAGE_INDEX_BUFFER_BIT,
                sharing_mode: vk::SharingMode::Exclusive,
                queue_family_index_count: 0,
                p_queue_family_indices: ptr::null(),
            };
            let index_buffer = base.device.create_buffer(&index_buffer_info, None).unwrap();
            let index_buffer_memory_req = base.device.get_buffer_memory_requirements(index_buffer);
            let index_buffer_memory_index =
                find_memorytype_index(
                    &index_buffer_memory_req,
                    &base.device_memory_properties,
                    vk::MEMORY_PROPERTY_HOST_VISIBLE_BIT,
                ).expect("Unable to find suitable memorytype for the index buffer.");
            let index_allocate_info = vk::MemoryAllocateInfo {
                s_type: vk::StructureType::MemoryAllocateInfo,
                p_next: ptr::null(),
                allocation_size: index_buffer_memory_req.size,
                memory_type_index: index_buffer_memory_index,
            };
            let index_buffer_memory = base
                .device
                .allocate_memory(&index_allocate_info, None)
                .unwrap();
            let index_ptr: *mut vk::c_void = base
                .device
                .map_memory(
                    index_buffer_memory,
                    0,
                    index_buffer_memory_req.size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();
            let mut index_slice = Align::new(
                index_ptr,
                align_of::<u32>() as u64,
                index_buffer_memory_req.size,
            );
            index_slice.copy_from_slice(&index_buffer_data);
            base.device.unmap_memory(index_buffer_memory);
            base.device
                .bind_buffer_memory(index_buffer, index_buffer_memory, 0)
                .unwrap();

            let vertices = [
                Vertex {
                    pos: [-1.0, -1.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                },
                Vertex {
                    pos: [-1.0, 1.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                },
                Vertex {
                    pos: [1.0, 1.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                },
                Vertex {
                    pos: [1.0, -1.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                },
            ];
            let vertex_input_buffer_info = vk::BufferCreateInfo {
                s_type: vk::StructureType::BufferCreateInfo,
                p_next: ptr::null(),
                flags: vk::BufferCreateFlags::empty(),
                size: std::mem::size_of_val(&vertices) as u64,
                usage: vk::BUFFER_USAGE_VERTEX_BUFFER_BIT,
                sharing_mode: vk::SharingMode::Exclusive,
                queue_family_index_count: 0,
                p_queue_family_indices: ptr::null(),
            };
            let vertex_input_buffer = base
                .device
                .create_buffer(&vertex_input_buffer_info, None)
                .unwrap();
            let vertex_input_buffer_memory_req = base
                .device
                .get_buffer_memory_requirements(vertex_input_buffer);
            let vertex_input_buffer_memory_index =
                find_memorytype_index(
                    &vertex_input_buffer_memory_req,
                    &base.device_memory_properties,
                    vk::MEMORY_PROPERTY_HOST_VISIBLE_BIT,
                ).expect("Unable to find suitable memorytype for the vertex buffer.");

            let vertex_buffer_allocate_info = vk::MemoryAllocateInfo {
                s_type: vk::StructureType::MemoryAllocateInfo,
                p_next: ptr::null(),
                allocation_size: vertex_input_buffer_memory_req.size,
                memory_type_index: vertex_input_buffer_memory_index,
            };
            let vertex_input_buffer_memory = base
                .device
                .allocate_memory(&vertex_buffer_allocate_info, None)
                .unwrap();
            let vert_ptr = base
                .device
                .map_memory(
                    vertex_input_buffer_memory,
                    0,
                    vertex_input_buffer_memory_req.size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();
            let mut slice = Align::new(
                vert_ptr,
                align_of::<Vertex>() as u64,
                vertex_input_buffer_memory_req.size,
            );
            slice.copy_from_slice(&vertices);
            base.device.unmap_memory(vertex_input_buffer_memory);
            base.device
                .bind_buffer_memory(vertex_input_buffer, vertex_input_buffer_memory, 0)
                .unwrap();

            let uniform_color_buffer = Buffer::new(
                &base,
                Vector4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
            );
            let uniform_color_buffer2 = Buffer::new(
                &base,
                Vector4 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                    w: 1.0,
                },
            );
            let mut uniform_time_buffer = Buffer::new(
                &base,
                0.0,
                // Vector4 {
                //     x: 0.5,
                //     y: 0.0,
                //     z: 1.0,
                //     w: 1.0,
                // },
            );
            let descriptor_sizes = [vk::DescriptorPoolSize {
                typ: vk::DescriptorType::UniformBuffer,
                descriptor_count: 1,
            }];
            let descriptor_pool_info = vk::DescriptorPoolCreateInfo {
                s_type: vk::StructureType::DescriptorPoolCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                pool_size_count: descriptor_sizes.len() as u32,
                p_pool_sizes: descriptor_sizes.as_ptr(),
                max_sets: 1,
            };
            let descriptor_pool = base
                .device
                .create_descriptor_pool(&descriptor_pool_info, None)
                .unwrap();
            let desc_layout_bindings = [vk::DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: vk::DescriptorType::UniformBuffer,
                descriptor_count: 1,
                stage_flags: vk::SHADER_STAGE_FRAGMENT_BIT,
                p_immutable_samplers: ptr::null(),
            }];
            let descriptor_info = vk::DescriptorSetLayoutCreateInfo {
                s_type: vk::StructureType::DescriptorSetLayoutCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                binding_count: desc_layout_bindings.len() as u32,
                p_bindings: desc_layout_bindings.as_ptr(),
            };

            let desc_set_layouts = vec![
                base.device
                    .create_descriptor_set_layout(&descriptor_info, None)
                    .unwrap(),
            ];
            let desc_alloc_info = vk::DescriptorSetAllocateInfo {
                s_type: vk::StructureType::DescriptorSetAllocateInfo,
                p_next: ptr::null(),
                descriptor_pool: descriptor_pool,
                descriptor_set_count: desc_set_layouts.len() as u32,
                p_set_layouts: desc_set_layouts.as_ptr(),
            };
            let descriptor_sets = base
                .device
                .allocate_descriptor_sets(&desc_alloc_info)
                .unwrap();

            let uniform_time_buffer_descriptor = vk::DescriptorBufferInfo {
                buffer: uniform_time_buffer.buffer,
                offset: 0,
                range: mem::size_of::<f32>() as u64,
            };

            let write_desc_sets = [vk::WriteDescriptorSet {
                s_type: vk::StructureType::WriteDescriptorSet,
                p_next: ptr::null(),
                dst_set: descriptor_sets[0],
                dst_binding: 0,
                dst_array_element: 0,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::UniformBuffer,
                p_image_info: ptr::null(),
                p_buffer_info: &uniform_time_buffer_descriptor,
                p_texel_buffer_view: ptr::null(),
            }];
            base.device.update_descriptor_sets(&write_desc_sets, &[]);

            let layout_create_info = vk::PipelineLayoutCreateInfo {
                s_type: vk::StructureType::PipelineLayoutCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                set_layout_count: desc_set_layouts.len() as u32,
                p_set_layouts: desc_set_layouts.as_ptr(),
                push_constant_range_count: 0,
                p_push_constant_ranges: ptr::null(),
            };

            let pipeline_layout = base
                .device
                .create_pipeline_layout(&layout_create_info, None)
                .unwrap();

            let viewports = vec![vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: base.surface_resolution.width as f32,
                height: base.surface_resolution.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }];
            let scissors = vec![vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: base.surface_resolution.clone(),
            }];

            let pipeline = GraphicsPipeline {
                renderpass,
                pipeline_layout,
                scissors,
                desc_set_layouts,
                viewports,
            };
            Quad {
                base,
                uniform_time_buffer,
                framebuffers,
                renderpass,
                pipeline_layout,
                descriptor_sets,
                pipeline,
                vertex_input_buffer,
                index_buffer,
            }
        }
    }
    pub fn render_loop<F: FnMut(&mut Self)>(&mut self, mut f: F) {
        use winit::*;
        let mut should_close = false;
        while !should_close {
            f(self);
            self.base
                .events_loop
                .borrow_mut()
                .poll_events(|event| match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::KeyboardInput { input, .. } => {
                            if let Some(VirtualKeyCode::Escape) = input.virtual_keycode {
                                should_close = true;
                            }
                        }
                        _ => (),
                    },
                    _ => (),
                });
        }
    }

    /// Compiles all fragemnt shaders and creates a pipeline object. This is useful because
    /// pipeline creation might fail for an invalid shader. We want to make sure that all shaders
    /// from rlsl are valid, or at least make it through through pipeline creation.
    pub fn compile_all(&self, base_path: &Path) {
        let vertex_file = base_path.join("vertex.spv");
        let dirs = read_dir(base_path).expect("Unable to read path to shaders");
        dirs.filter_map(|dir_entry| dir_entry.ok())
            .filter(|dir_entry| {
                let path = dir_entry.path();
                if let Some(ext) = path.extension() {
                    ext == "spv"
                        && !path
                            .file_stem()
                            .map(|name| name == "vertex")
                            .unwrap_or(false)
                } else {
                    false
                }
            })
            .for_each(|dir_entry| {
                println!("Compiling: {}", dir_entry.path().display());
                let _ = self.pipeline.create(
                    &self.base.device,
                    ("vertex", &vertex_file),
                    ("fragment", &dir_entry.path()),
                );
            });
    }

    // pub fn notify_pipeline(
    //     &self,
    //     vertex_info: (&str, &Path),
    //     fragment_info: (&str, &Path),
    // ) -> Receiver<GraphicsPipeline> {
    // }
    pub fn render_all(&mut self, vertex_info: (&str, &Path), fragment_infos: &[(&str, &Path)]) {
        let pipelines: Vec<_> = fragment_infos
            .iter()
            .map(|&fragment_info| {
                self.pipeline
                    .create(&self.base.device, vertex_info, fragment_info)
            })
            .collect();
        let count = pipelines.len();
        let mut start_time = SystemTime::now();
        let transition_duration = Duration::from_secs(10);
        let mut index = 0;
        self.render(|quad| {
            let dur = SystemTime::now()
                .duration_since(start_time)
                .expect("duration");
            if dur >= transition_duration {
                index = (index + 1) % count;
                start_time = SystemTime::now();
            }
            pipelines[index]
        });
    }
    /// Renders a single vertex and fragment shader
    pub fn render_single(&mut self, vertex_info: (&str, &Path), fragment_info: (&str, &Path)) {
        let (tx, rx) = channel();
        let mut watcher: RecommendedWatcher =
            Watcher::new(tx, Duration::from_millis(500)).expect("watcher");
        watcher.watch(fragment_info.1, RecursiveMode::Recursive);
        let mut graphics_pipeline =
            self.pipeline
                .create(&self.base.device, vertex_info, fragment_info);
        self.render(|quad| {
            if let Some(watch_res) = rx.try_recv().ok() {
                match watch_res {
                    notify::DebouncedEvent::Write(_) => {
                        println!("Detected Chage: Recreate pipeline");
                        unsafe {
                            quad.base.device.destroy_pipeline(graphics_pipeline, None);
                        }
                        graphics_pipeline =
                            quad.pipeline
                                .create(&quad.base.device, vertex_info, fragment_info);
                    }
                    _ => (),
                };
            }
            graphics_pipeline
        });
    }
    pub fn render<F: FnMut(&mut Self) -> vk::Pipeline>(&mut self, mut f: F) {
        unsafe {
            let start = SystemTime::now();
            let mut frame_start_time = SystemTime::now();
            self.render_loop(|quad| {
                let graphics_pipeline = f(quad);
                let current = SystemTime::now();
                let duration = current.duration_since(start).expect("dur");
                let secs = duration.as_secs() as f64;
                let subsecconds = duration.subsec_nanos() as f64 / 1_000_000_000.0;

                let time = (secs + subsecconds) as f32;
                //println!("{}", time);
                quad.uniform_time_buffer.update(
                    &quad.base,
                    time
                    // Vector4 {
                    //     x: time,
                    //     y: 0.0,
                    //     z: 0.0,
                    //     w: 1.0,
                    // },
                );
                let present_index = quad
                    .base
                    .swapchain_loader
                    .acquire_next_image_khr(
                        quad.base.swapchain,
                        std::u64::MAX,
                        quad.base.present_complete_semaphore,
                        vk::Fence::null(),
                    )
                    .unwrap();
                let clear_values = [
                    vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.0, 0.0],
                        },
                    },
                    vk::ClearValue {
                        depth: vk::ClearDepthStencilValue {
                            depth: 1.0,
                            stencil: 0,
                        },
                    },
                ];

                let render_pass_begin_info = vk::RenderPassBeginInfo {
                    s_type: vk::StructureType::RenderPassBeginInfo,
                    p_next: ptr::null(),
                    render_pass: quad.renderpass,
                    framebuffer: quad.framebuffers[present_index as usize],
                    render_area: vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: quad.base.surface_resolution.clone(),
                    },
                    clear_value_count: clear_values.len() as u32,
                    p_clear_values: clear_values.as_ptr(),
                };
                record_submit_commandbuffer(
                    &quad.base.device,
                    quad.base.draw_command_buffer,
                    quad.base.present_queue,
                    &[vk::PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT],
                    &[quad.base.present_complete_semaphore],
                    &[quad.base.rendering_complete_semaphore],
                    |device, draw_command_buffer| {
                        device.cmd_begin_render_pass(
                            draw_command_buffer,
                            &render_pass_begin_info,
                            vk::SubpassContents::Inline,
                        );
                        device.cmd_bind_descriptor_sets(
                            draw_command_buffer,
                            vk::PipelineBindPoint::Graphics,
                            quad.pipeline_layout,
                            0,
                            &quad.descriptor_sets[..],
                            &[],
                        );
                        device.cmd_bind_pipeline(
                            draw_command_buffer,
                            vk::PipelineBindPoint::Graphics,
                            graphics_pipeline,
                        );
                        device.cmd_set_viewport(draw_command_buffer, 0, &quad.pipeline.viewports);
                        device.cmd_set_scissor(draw_command_buffer, 0, &quad.pipeline.scissors);
                        device.cmd_bind_vertex_buffers(
                            draw_command_buffer,
                            0,
                            &[quad.vertex_input_buffer],
                            &[0],
                        );
                        device.cmd_bind_index_buffer(
                            draw_command_buffer,
                            quad.index_buffer,
                            0,
                            vk::IndexType::Uint32,
                        );
                        device.cmd_draw_indexed(draw_command_buffer, 6, 1, 0, 0, 1);
                        // Or draw without the index buffer
                        // device.cmd_draw(draw_command_buffer, 3, 1, 0, 0);
                        device.cmd_end_render_pass(draw_command_buffer);
                    },
                );
                //let mut present_info_err = mem::uninitialized();
                let present_info = vk::PresentInfoKHR {
                    s_type: vk::StructureType::PresentInfoKhr,
                    p_next: ptr::null(),
                    wait_semaphore_count: 1,
                    p_wait_semaphores: &quad.base.rendering_complete_semaphore,
                    swapchain_count: 1,
                    p_swapchains: &quad.base.swapchain,
                    p_image_indices: &present_index,
                    p_results: ptr::null_mut(),
                };
                quad.base
                    .swapchain_loader
                    .queue_present_khr(quad.base.present_queue, &present_info)
                    .unwrap();
                let current_time = SystemTime::now();
                let dt = current_time.duration_since(frame_start_time);
                frame_start_time = current_time;
            });
        }
    }
}
pub fn record_submit_commandbuffer<D: DeviceV1_0, F: FnOnce(&D, vk::CommandBuffer)>(
    device: &D,
    command_buffer: vk::CommandBuffer,
    submit_queue: vk::Queue,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    f: F,
) {
    unsafe {
        device
            .reset_command_buffer(
                command_buffer,
                vk::COMMAND_BUFFER_RESET_RELEASE_RESOURCES_BIT,
            )
            .expect("Reset command buffer failed.");
        let command_buffer_begin_info = vk::CommandBufferBeginInfo {
            s_type: vk::StructureType::CommandBufferBeginInfo,
            p_next: ptr::null(),
            p_inheritance_info: ptr::null(),
            flags: vk::COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
        };
        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Begin commandbuffer");
        f(device, command_buffer);
        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer");
        let fence_create_info = vk::FenceCreateInfo {
            s_type: vk::StructureType::FenceCreateInfo,
            p_next: ptr::null(),
            flags: vk::FenceCreateFlags::empty(),
        };
        let submit_fence = device
            .create_fence(&fence_create_info, None)
            .expect("Create fence failed.");
        let submit_info = vk::SubmitInfo {
            s_type: vk::StructureType::SubmitInfo,
            p_next: ptr::null(),
            wait_semaphore_count: wait_semaphores.len() as u32,
            p_wait_semaphores: wait_semaphores.as_ptr(),
            p_wait_dst_stage_mask: wait_mask.as_ptr(),
            command_buffer_count: 1,
            p_command_buffers: &command_buffer,
            signal_semaphore_count: signal_semaphores.len() as u32,
            p_signal_semaphores: signal_semaphores.as_ptr(),
        };
        device
            .queue_submit(submit_queue, &[submit_info], submit_fence)
            .expect("queue submit failed.");
        device
            .wait_for_fences(&[submit_fence], true, std::u64::MAX)
            .expect("Wait for fence failed.");
        device.destroy_fence(submit_fence, None);
    }
}

#[cfg(all(unix, not(target_os = "android")))]
unsafe fn create_surface<E: EntryV1_0, I: InstanceV1_0>(
    entry: &E,
    instance: &I,
    window: &winit::Window,
) -> Result<vk::SurfaceKHR, vk::Result> {
    use winit::os::unix::WindowExt;
    let x11_display = window.get_xlib_display().unwrap();
    let x11_window = window.get_xlib_window().unwrap();
    let x11_create_info = vk::XlibSurfaceCreateInfoKHR {
        s_type: vk::StructureType::XlibSurfaceCreateInfoKhr,
        p_next: ptr::null(),
        flags: Default::default(),
        window: x11_window as vk::Window,
        dpy: x11_display as *mut vk::Display,
    };
    let xlib_surface_loader =
        XlibSurface::new(entry, instance).expect("Unable to load xlib surface");
    xlib_surface_loader.create_xlib_surface_khr(&x11_create_info, None)
}

#[cfg(windows)]
unsafe fn create_surface<E: EntryV1_0, I: InstanceV1_0>(
    entry: &E,
    instance: &I,
    window: &winit::Window,
) -> Result<vk::SurfaceKHR, vk::Result> {
    use winit::os::windows::WindowExt;
    let hwnd = window.get_hwnd() as *mut winapi::windef::HWND__;
    let hinstance = unsafe { user32::GetWindow(hwnd, 0) as *const vk::c_void };
    let win32_create_info = vk::Win32SurfaceCreateInfoKHR {
        s_type: vk::StructureType::Win32SurfaceCreateInfoKhr,
        p_next: ptr::null(),
        flags: Default::default(),
        hinstance: hinstance,
        hwnd: hwnd as *const vk::c_void,
    };
    let win32_surface_loader =
        Win32Surface::new(entry, instance).expect("Unable to load win32 surface");
    win32_surface_loader.create_win32_surface_khr(&win32_create_info, None)
}

#[cfg(all(unix, not(target_os = "android")))]
fn extension_names() -> Vec<*const i8> {
    vec![
        Surface::name().as_ptr(),
        XlibSurface::name().as_ptr(),
        DebugReport::name().as_ptr(),
    ]
}

#[cfg(all(windows))]
fn extension_names() -> Vec<*const i8> {
    vec![
        Surface::name().as_ptr(),
        Win32Surface::name().as_ptr(),
        DebugReport::name().as_ptr(),
    ]
}

unsafe extern "system" fn vulkan_debug_callback(
    _: vk::DebugReportFlagsEXT,
    _: vk::DebugReportObjectTypeEXT,
    _: vk::uint64_t,
    _: vk::size_t,
    _: vk::int32_t,
    _: *const vk::c_char,
    p_message: *const vk::c_char,
    _: *mut vk::c_void,
) -> u32 {
    println!("{:?}", CStr::from_ptr(p_message));
    0
}

pub fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    // Try to find an exactly matching memory flag
    let best_suitable_index =
        find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
            property_flags == flags
        });
    if best_suitable_index.is_some() {
        return best_suitable_index;
    }
    // Otherwise find a memory flag that works
    find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
        property_flags & flags == flags
    })
}

pub fn find_memorytype_index_f<F: Fn(vk::MemoryPropertyFlags, vk::MemoryPropertyFlags) -> bool>(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
    f: F,
) -> Option<u32> {
    let mut memory_type_bits = memory_req.memory_type_bits;
    for (index, ref memory_type) in memory_prop.memory_types.iter().enumerate() {
        if memory_type_bits & 1 == 1 {
            if f(memory_type.property_flags, flags) {
                return Some(index as u32);
            }
        }
        memory_type_bits = memory_type_bits >> 1;
    }
    None
}

fn resize_callback(width: u32, height: u32) {
    println!("Window resized to {}x{}", width, height);
}

pub struct Buffer<T> {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub req: vk::MemoryRequirements,
    _m: PhantomData<T>,
}

impl<T: Copy> Buffer<T> {
    pub unsafe fn update(&mut self, base: &ExampleBase, t: T) {
        let uniform_ptr = base
            .device
            .map_memory(self.memory, 0, self.req.size, vk::MemoryMapFlags::empty())
            .unwrap();
        let mut uniform_aligned_slice =
            Align::new(uniform_ptr, align_of::<T>() as u64, self.req.size);
        uniform_aligned_slice.copy_from_slice(&[t]);
        base.device.unmap_memory(self.memory);
    }
    pub unsafe fn new(base: &ExampleBase, uniform_color_buffer_data: T) -> Self {
        let uniform_color_buffer_info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BufferCreateInfo,
            p_next: ptr::null(),
            flags: vk::BufferCreateFlags::empty(),
            size: std::mem::size_of::<T>() as u64,
            usage: vk::BUFFER_USAGE_UNIFORM_BUFFER_BIT,
            sharing_mode: vk::SharingMode::Exclusive,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
        };
        let uniform_color_buffer = base
            .device
            .create_buffer(&uniform_color_buffer_info, None)
            .unwrap();
        let uniform_color_buffer_memory_req = base
            .device
            .get_buffer_memory_requirements(uniform_color_buffer);
        let uniform_color_buffer_memory_index =
            find_memorytype_index(
                &uniform_color_buffer_memory_req,
                &base.device_memory_properties,
                vk::MEMORY_PROPERTY_HOST_VISIBLE_BIT,
            ).expect("Unable to find suitable memorytype for the vertex buffer.");

        let uniform_color_buffer_allocate_info = vk::MemoryAllocateInfo {
            s_type: vk::StructureType::MemoryAllocateInfo,
            p_next: ptr::null(),
            allocation_size: uniform_color_buffer_memory_req.size,
            memory_type_index: uniform_color_buffer_memory_index,
        };
        let uniform_color_buffer_memory = base
            .device
            .allocate_memory(&uniform_color_buffer_allocate_info, None)
            .unwrap();
        base.device
            .bind_buffer_memory(uniform_color_buffer, uniform_color_buffer_memory, 0)
            .unwrap();
        let uniform_ptr = base
            .device
            .map_memory(
                uniform_color_buffer_memory,
                0,
                uniform_color_buffer_memory_req.size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();
        let mut uniform_aligned_slice = Align::new(
            uniform_ptr,
            align_of::<T>() as u64,
            uniform_color_buffer_memory_req.size,
        );
        uniform_aligned_slice.copy_from_slice(&[uniform_color_buffer_data]);
        base.device.unmap_memory(uniform_color_buffer_memory);
        Buffer {
            buffer: uniform_color_buffer,
            memory: uniform_color_buffer_memory,
            req: uniform_color_buffer_memory_req,
            _m: PhantomData,
        }
    }
}
pub struct ExampleBase {
    pub entry: Entry<V1_0>,
    pub instance: Instance<V1_0>,
    pub device: Device<V1_0>,
    pub surface_loader: Surface,
    pub swapchain_loader: Swapchain,
    pub debug_report_loader: DebugReport,
    pub window: winit::Window,
    pub events_loop: RefCell<winit::EventsLoop>,
    pub debug_call_back: vk::DebugReportCallbackEXT,

    pub pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,

    pub swapchain: vk::SwapchainKHR,
    pub present_images: Vec<vk::Image>,
    pub present_image_views: Vec<vk::ImageView>,

    pub pool: vk::CommandPool,
    pub draw_command_buffer: vk::CommandBuffer,
    pub setup_command_buffer: vk::CommandBuffer,

    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,

    pub present_complete_semaphore: vk::Semaphore,
    pub rendering_complete_semaphore: vk::Semaphore,
}

impl ExampleBase {
    pub fn new(window_width: u32, window_height: u32) -> Self {
        unsafe {
            let events_loop = winit::EventsLoop::new();
            let window = winit::WindowBuilder::new()
                .with_title("Ash - Example")
                .with_dimensions(window_width, window_height)
                //.with_window_resize_callback(resize_callback)
                .build(&events_loop)
                .unwrap();
            let entry = Entry::new().unwrap();
            let app_name = CString::new("VulkanTriangle").unwrap();
            let raw_name = app_name.as_ptr();

            let layer_names = [CString::new("VK_LAYER_LUNARG_standard_validation").unwrap()];
            //let layer_names: [CString;0] = [];
            let layers_names_raw: Vec<*const i8> = layer_names
                .iter()
                .map(|raw_name| raw_name.as_ptr())
                .collect();
            let extension_names_raw = extension_names();
            let appinfo = vk::ApplicationInfo {
                p_application_name: raw_name,
                s_type: vk::StructureType::ApplicationInfo,
                p_next: ptr::null(),
                application_version: 0,
                p_engine_name: raw_name,
                engine_version: 0,
                api_version: vk_make_version!(1, 1, 0),
            };
            let create_info = vk::InstanceCreateInfo {
                s_type: vk::StructureType::InstanceCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                p_application_info: &appinfo,
                pp_enabled_layer_names: layers_names_raw.as_ptr(),
                enabled_layer_count: layers_names_raw.len() as u32,
                pp_enabled_extension_names: extension_names_raw.as_ptr(),
                enabled_extension_count: extension_names_raw.len() as u32,
            };
            let instance: Instance<V1_0> = entry
                .create_instance(&create_info, None)
                .expect("Instance creation error");
            let debug_info = vk::DebugReportCallbackCreateInfoEXT {
                s_type: vk::StructureType::DebugReportCallbackCreateInfoExt,
                p_next: ptr::null(),
                flags: vk::DEBUG_REPORT_ERROR_BIT_EXT
                    | vk::DEBUG_REPORT_WARNING_BIT_EXT
                    | vk::DEBUG_REPORT_PERFORMANCE_WARNING_BIT_EXT,
                pfn_callback: vulkan_debug_callback,
                p_user_data: ptr::null_mut(),
            };
            let debug_report_loader =
                DebugReport::new(&entry, &instance).expect("Unable to load debug report");
            let debug_call_back = debug_report_loader
                .create_debug_report_callback_ext(&debug_info, None)
                .unwrap();
            let surface = create_surface(&entry, &instance, &window).unwrap();
            let pdevices = instance
                .enumerate_physical_devices()
                .expect("Physical device error");
            let surface_loader =
                Surface::new(&entry, &instance).expect("Unable to load the Surface extension");
            let (pdevice, queue_family_index) = pdevices
                .iter()
                .map(|pdevice| {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .filter_map(|(index, ref info)| {
                            let supports_graphic_and_surface =
                                info.queue_flags.subset(vk::QUEUE_GRAPHICS_BIT)
                                    && surface_loader.get_physical_device_surface_support_khr(
                                        *pdevice,
                                        index as u32,
                                        surface,
                                    );
                            match supports_graphic_and_surface {
                                true => Some((*pdevice, index)),
                                _ => None,
                            }
                        })
                        .nth(0)
                })
                .filter_map(|v| v)
                .nth(0)
                .expect("Couldn't find suitable device.");
            let queue_family_index = queue_family_index as u32;
            let device_extension_names_raw = [Swapchain::name().as_ptr()];
            let features = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                ..Default::default()
            };
            let priorities = [1.0];
            let queue_info = vk::DeviceQueueCreateInfo {
                s_type: vk::StructureType::DeviceQueueCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                queue_family_index: queue_family_index as u32,
                p_queue_priorities: priorities.as_ptr(),
                queue_count: priorities.len() as u32,
            };
            let device_create_info = vk::DeviceCreateInfo {
                s_type: vk::StructureType::DeviceCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                queue_create_info_count: 1,
                p_queue_create_infos: &queue_info,
                enabled_layer_count: 0,
                pp_enabled_layer_names: ptr::null(),
                enabled_extension_count: device_extension_names_raw.len() as u32,
                pp_enabled_extension_names: device_extension_names_raw.as_ptr(),
                p_enabled_features: &features,
            };
            let device: Device<V1_0> = instance
                .create_device(pdevice, &device_create_info, None)
                .unwrap();
            let present_queue = device.get_device_queue(queue_family_index as u32, 0);

            let surface_formats = surface_loader
                .get_physical_device_surface_formats_khr(pdevice, surface)
                .unwrap();
            let surface_format = surface_formats
                .iter()
                .map(|sfmt| match sfmt.format {
                    vk::Format::Undefined => vk::SurfaceFormatKHR {
                        format: vk::Format::B8g8r8Unorm,
                        color_space: sfmt.color_space,
                    },
                    _ => sfmt.clone(),
                })
                .nth(0)
                .expect("Unable to find suitable surface format.");
            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities_khr(pdevice, surface)
                .unwrap();
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }
            let surface_resolution = match surface_capabilities.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: window_width,
                    height: window_height,
                },
                _ => surface_capabilities.current_extent,
            };
            let pre_transform = if surface_capabilities
                .supported_transforms
                .subset(vk::SURFACE_TRANSFORM_IDENTITY_BIT_KHR)
            {
                vk::SURFACE_TRANSFORM_IDENTITY_BIT_KHR
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes_khr(pdevice, surface)
                .unwrap();
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::Mailbox)
                .unwrap_or(vk::PresentModeKHR::Fifo);
            let swapchain_loader =
                Swapchain::new(&instance, &device).expect("Unable to load swapchain");
            let swapchain_create_info = vk::SwapchainCreateInfoKHR {
                s_type: vk::StructureType::SwapchainCreateInfoKhr,
                p_next: ptr::null(),
                flags: Default::default(),
                surface: surface,
                min_image_count: desired_image_count,
                image_color_space: surface_format.color_space,
                image_format: surface_format.format,
                image_extent: surface_resolution.clone(),
                image_usage: vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
                image_sharing_mode: vk::SharingMode::Exclusive,
                pre_transform: pre_transform,
                composite_alpha: vk::COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
                present_mode: present_mode,
                clipped: 1,
                old_swapchain: vk::SwapchainKHR::null(),
                image_array_layers: 1,
                p_queue_family_indices: ptr::null(),
                queue_family_index_count: 0,
            };
            let swapchain = swapchain_loader
                .create_swapchain_khr(&swapchain_create_info, None)
                .unwrap();
            let pool_create_info = vk::CommandPoolCreateInfo {
                s_type: vk::StructureType::CommandPoolCreateInfo,
                p_next: ptr::null(),
                flags: vk::COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT,
                queue_family_index: queue_family_index,
            };
            let pool = device.create_command_pool(&pool_create_info, None).unwrap();
            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo {
                s_type: vk::StructureType::CommandBufferAllocateInfo,
                p_next: ptr::null(),
                command_buffer_count: 2,
                command_pool: pool,
                level: vk::CommandBufferLevel::Primary,
            };
            let command_buffers = device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap();
            let setup_command_buffer = command_buffers[0];
            let draw_command_buffer = command_buffers[1];

            let present_images = swapchain_loader
                .get_swapchain_images_khr(swapchain)
                .unwrap();
            let present_image_views: Vec<vk::ImageView> = present_images
                .iter()
                .map(|&image| {
                    let create_view_info = vk::ImageViewCreateInfo {
                        s_type: vk::StructureType::ImageViewCreateInfo,
                        p_next: ptr::null(),
                        flags: Default::default(),
                        view_type: vk::ImageViewType::Type2d,
                        format: surface_format.format,
                        components: vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        },
                        subresource_range: vk::ImageSubresourceRange {
                            aspect_mask: vk::IMAGE_ASPECT_COLOR_BIT,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        },
                        image: image,
                    };
                    device.create_image_view(&create_view_info, None).unwrap()
                })
                .collect();
            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);
            let depth_image_create_info = vk::ImageCreateInfo {
                s_type: vk::StructureType::ImageCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                image_type: vk::ImageType::Type2d,
                format: vk::Format::D16Unorm,
                extent: vk::Extent3D {
                    width: surface_resolution.width,
                    height: surface_resolution.height,
                    depth: 1,
                },
                mip_levels: 1,
                array_layers: 1,
                samples: vk::SAMPLE_COUNT_1_BIT,
                tiling: vk::ImageTiling::Optimal,
                usage: vk::IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT,
                sharing_mode: vk::SharingMode::Exclusive,
                queue_family_index_count: 0,
                p_queue_family_indices: ptr::null(),
                initial_layout: vk::ImageLayout::Undefined,
            };
            let depth_image = device.create_image(&depth_image_create_info, None).unwrap();
            let depth_image_memory_req = device.get_image_memory_requirements(depth_image);
            let depth_image_memory_index =
                find_memorytype_index(
                    &depth_image_memory_req,
                    &device_memory_properties,
                    vk::MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
                ).expect("Unable to find suitable memory index for depth image.");

            let depth_image_allocate_info = vk::MemoryAllocateInfo {
                s_type: vk::StructureType::MemoryAllocateInfo,
                p_next: ptr::null(),
                allocation_size: depth_image_memory_req.size,
                memory_type_index: depth_image_memory_index,
            };
            let depth_image_memory = device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap();
            device
                .bind_image_memory(depth_image, depth_image_memory, 0)
                .expect("Unable to bind depth image memory");
            record_submit_commandbuffer(
                &device,
                setup_command_buffer,
                present_queue,
                &[vk::PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT],
                &[],
                &[],
                |device, setup_command_buffer| {
                    let layout_transition_barrier = vk::ImageMemoryBarrier {
                        s_type: vk::StructureType::ImageMemoryBarrier,
                        p_next: ptr::null(),
                        src_access_mask: Default::default(),
                        dst_access_mask: vk::ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT
                            | vk::ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
                        old_layout: vk::ImageLayout::Undefined,
                        new_layout: vk::ImageLayout::DepthStencilAttachmentOptimal,
                        src_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED,
                        dst_queue_family_index: vk::VK_QUEUE_FAMILY_IGNORED,
                        image: depth_image,
                        subresource_range: vk::ImageSubresourceRange {
                            aspect_mask: vk::IMAGE_ASPECT_DEPTH_BIT,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        },
                    };
                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,
                        vk::PIPELINE_STAGE_LATE_FRAGMENT_TESTS_BIT,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barrier],
                    );
                },
            );
            let depth_image_view_info = vk::ImageViewCreateInfo {
                s_type: vk::StructureType::ImageViewCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
                view_type: vk::ImageViewType::Type2d,
                format: depth_image_create_info.format,
                components: vk::ComponentMapping {
                    r: vk::ComponentSwizzle::Identity,
                    g: vk::ComponentSwizzle::Identity,
                    b: vk::ComponentSwizzle::Identity,
                    a: vk::ComponentSwizzle::Identity,
                },
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: vk::IMAGE_ASPECT_DEPTH_BIT,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                image: depth_image,
            };
            let depth_image_view = device
                .create_image_view(&depth_image_view_info, None)
                .unwrap();
            let semaphore_create_info = vk::SemaphoreCreateInfo {
                s_type: vk::StructureType::SemaphoreCreateInfo,
                p_next: ptr::null(),
                flags: Default::default(),
            };
            let present_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            let rendering_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            ExampleBase {
                events_loop: RefCell::new(events_loop),
                entry: entry,
                instance: instance,
                device: device,
                queue_family_index: queue_family_index,
                pdevice: pdevice,
                device_memory_properties: device_memory_properties,
                window: window,
                surface_loader: surface_loader,
                surface_format: surface_format,
                present_queue: present_queue,
                surface_resolution: surface_resolution,
                swapchain_loader: swapchain_loader,
                swapchain: swapchain,
                present_images: present_images,
                present_image_views: present_image_views,
                pool: pool,
                draw_command_buffer: draw_command_buffer,
                setup_command_buffer: setup_command_buffer,
                depth_image: depth_image,
                depth_image_view: depth_image_view,
                present_complete_semaphore: present_complete_semaphore,
                rendering_complete_semaphore: rendering_complete_semaphore,
                surface: surface,
                debug_call_back: debug_call_back,
                debug_report_loader: debug_report_loader,
                depth_image_memory: depth_image_memory,
            }
        }
    }
}

impl Drop for ExampleBase {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device.free_memory(self.depth_image_memory, None);
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device.destroy_image(self.depth_image, None);
            for &image_view in self.present_image_views.iter() {
                self.device.destroy_image_view(image_view, None);
            }
            self.device.destroy_command_pool(self.pool, None);
            self.swapchain_loader
                .destroy_swapchain_khr(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface_khr(self.surface, None);
            self.debug_report_loader
                .destroy_debug_report_callback_ext(self.debug_call_back, None);
            self.instance.destroy_instance(None);
        }
    }
}
