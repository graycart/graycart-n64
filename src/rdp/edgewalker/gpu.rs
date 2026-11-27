//! EdgeWalker compute device (`vulkan-rdp` feature).
//!
//! Owns a [`wgpu`] device and queue for the bespoke EdgeWalker compute LLE.
//! Opening the device prefers a Vulkan adapter, then falls back through the
//! same instance to DX12, Metal, or browser WebGPU. A missing adapter or any
//! failed device, shader, buffer, or map call returns
//! [`Trap::UnimplementedRegister`] for `EdgeWalker` at offset `0x0001`.
//! This type is intentionally not [`Serialize`](serde::Serialize): wgpu
//! handles are process-local.
//!
//! Cited: wgpu 24.0.5 — Instance, Adapter, Device, Queue
//!   https://docs.rs/wgpu/24.0.5/wgpu/
//! Cited: N64brew — Reality Display Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//! Cross-check: docs/ORCHESTRATOR-HANDOFF.md (Milestone M6)

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::trap::Trap;

use super::compute_unavailable;

/// Host-side GPU connection for EdgeWalker compute passes.
///
/// Not serializable. Dropping the value releases the device and queue.
#[derive(Debug)]
pub struct ComputeDevice {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: wgpu::AdapterInfo,
    fault: Arc<AtomicBool>,
}

impl ComputeDevice {
    /// Open a compute-capable device.
    ///
    /// Vulkan adapters are tried first (discrete, then integrated, then
    /// virtual, then CPU). If none of those open, the instance's default
    /// adapter (DX12, Metal, or WebGPU) is tried. Failure to acquire a
    /// device returns the EdgeWalker compute trap.
    pub fn open() -> Result<Self, Trap> {
        let instance = catch_trap(instance_new)?;

        let mut adapters = instance.enumerate_adapters(wgpu::Backends::VULKAN);
        adapters.sort_by_key(vulkan_rank);
        if let Some(fallback) = pollster::block_on(instance.request_adapter(&adapter_options())) {
            adapters.push(fallback);
        }

        let mut seen: Vec<(wgpu::Backend, u32, String)> = Vec::new();
        for adapter in adapters {
            let info = adapter.get_info();
            let key = (info.backend, info.device, info.name.clone());
            if seen.iter().any(|prior| prior == &key) {
                continue;
            }
            seen.push(key);
            if !adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
            {
                continue;
            }
            if let Some(device) = open_adapter(&adapter, info) {
                return Ok(device);
            }
        }
        Err(edgewalker_trap())
    }

    /// The underlying device. Valid for the lifetime of this value.
    #[must_use]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The queue that records EdgeWalker uploads, dispatches, and readback.
    #[must_use]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Adapter that supplied [`Self::device`].
    #[must_use]
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.info
    }

    /// Compile `wgsl` and build a compute pipeline for `entry_point`.
    ///
    /// `layout` is inferred from the shader. A parse, validation, or device
    /// error returns the EdgeWalker compute trap.
    pub fn create_compute_pipeline(
        &self,
        wgsl: &str,
        entry_point: &str,
    ) -> Result<wgpu::ComputePipeline, Trap> {
        self.failed()?;
        if entry_point.is_empty() {
            return Err(edgewalker_trap());
        }
        self.push_scopes();
        let created = catch_trap(|| {
            let module = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("edgewalker-compute"),
                    source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(wgsl)),
                });
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry_point),
                    layout: None,
                    module: &module,
                    entry_point: Some(entry_point),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                })
        });
        self.finish(created)
    }

    /// Storage buffer with `STORAGE | COPY_SRC | COPY_DST`.
    ///
    /// `size` must be non-zero, a multiple of [`wgpu::COPY_BUFFER_ALIGNMENT`],
    /// and within the device's `max_buffer_size`.
    pub fn create_storage_buffer(&self, size: u64) -> Result<wgpu::Buffer, Trap> {
        self.failed()?;
        if size == 0
            || !size.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
            || size > self.device.limits().max_buffer_size
        {
            return Err(edgewalker_trap());
        }
        self.push_scopes();
        let created = catch_trap(|| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("edgewalker-storage"),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        self.finish(created)
    }

    /// Write `data` into `buffer` at `offset`.
    ///
    /// `offset` and `data.len()` must be multiples of 4, and the write must
    /// stay inside the buffer. The upload is ordered before later submissions
    /// on [`Self::queue`].
    pub fn write_buffer(
        &self,
        buffer: &wgpu::Buffer,
        offset: u64,
        data: &[u8],
    ) -> Result<(), Trap> {
        self.failed()?;
        let len = u64::try_from(data.len()).map_err(|_| edgewalker_trap())?;
        let end = offset.checked_add(len).ok_or_else(edgewalker_trap)?;
        if !offset.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
            || !len.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
            || end > buffer.size()
        {
            return Err(edgewalker_trap());
        }
        self.push_scopes();
        let wrote = catch_trap(|| {
            self.queue.write_buffer(buffer, offset, data);
        });
        self.finish(wrote)
    }

    /// Bind whole storage buffers into group `group_index` of `pipeline`.
    ///
    /// Each pair is `(binding index, buffer)`. The pipeline layout comes from
    /// the shader reflection of [`Self::create_compute_pipeline`].
    pub fn create_storage_bind_group(
        &self,
        pipeline: &wgpu::ComputePipeline,
        group_index: u32,
        bindings: &[(u32, &wgpu::Buffer)],
    ) -> Result<wgpu::BindGroup, Trap> {
        self.failed()?;
        self.push_scopes();
        let created = catch_trap(|| {
            let layout = pipeline.get_bind_group_layout(group_index);
            let entries: Vec<wgpu::BindGroupEntry<'_>> = bindings
                .iter()
                .map(|(binding, buffer)| wgpu::BindGroupEntry {
                    binding: *binding,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: None,
                    }),
                })
                .collect();
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("edgewalker-storage"),
                layout: &layout,
                entries: &entries,
            })
        });
        self.finish(created)
    }

    /// Record a compute dispatch and wait until that submission finishes.
    ///
    /// `bind_group`, when set, is bound at group 0. `(x, y, z)` are workgroup
    /// counts. A validation or device error returns the EdgeWalker trap.
    pub fn dispatch_workgroups(
        &self,
        pipeline: &wgpu::ComputePipeline,
        bind_group: Option<&wgpu::BindGroup>,
        x: u32,
        y: u32,
        z: u32,
    ) -> Result<(), Trap> {
        self.failed()?;
        self.push_scopes();
        let submitted = catch_trap(|| {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("edgewalker-dispatch"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("edgewalker-dispatch"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                if let Some(group) = bind_group {
                    pass.set_bind_group(0, group, &[]);
                }
                pass.dispatch_workgroups(x, y, z);
            }
            self.queue.submit(std::iter::once(encoder.finish()));
        });
        self.finish(submitted)
    }

    /// Copy `buffer` back to the host and wait until the bytes are mapped.
    ///
    /// `buffer` must use `COPY_SRC` and its size must be a non-zero multiple
    /// of 4. The returned vector has that exact length (staging is padded to
    /// the map alignment of 8 when the source is only 4-byte aligned).
    pub fn readback(&self, buffer: &wgpu::Buffer) -> Result<Vec<u8>, Trap> {
        self.failed()?;
        let size = buffer.size();
        if size == 0 || !size.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT) {
            return Err(edgewalker_trap());
        }
        let staging_size = size.next_multiple_of(wgpu::MAP_ALIGNMENT);
        if staging_size > self.device.limits().max_buffer_size {
            return Err(edgewalker_trap());
        }
        self.push_scopes();
        let copied =
            catch_trap(|| self.copy_mapped(buffer, size, staging_size)).and_then(|inner| inner);
        self.finish(copied)
    }

    fn copy_mapped(
        &self,
        buffer: &wgpu::Buffer,
        size: u64,
        staging_size: u64,
    ) -> Result<Vec<u8>, Trap> {
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("edgewalker-readback"),
            size: staging_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("edgewalker-readback"),
            });
        encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, size);
        self.queue.submit(std::iter::once(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|_| edgewalker_trap())?
            .map_err(|_| edgewalker_trap())?;

        let len = usize::try_from(size).map_err(|_| edgewalker_trap())?;
        let data = {
            let view = staging.slice(..).get_mapped_range();
            view[..len].to_vec()
        };
        staging.unmap();
        Ok(data)
    }

    fn failed(&self) -> Result<(), Trap> {
        if self.fault.load(Ordering::Acquire) {
            Err(edgewalker_trap())
        } else {
            Ok(())
        }
    }

    fn push_scopes(&self) {
        self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
    }

    fn finish<T>(&self, value: Result<T, Trap>) -> Result<T, Trap> {
        let scopes = self.pop_scopes();
        match (value, scopes) {
            (Ok(value), Ok(())) => Ok(value),
            _ => Err(edgewalker_trap()),
        }
    }

    fn pop_scopes(&self) -> Result<(), Trap> {
        let _ = catch_trap(|| {
            self.device.poll(wgpu::Maintain::Wait);
        });
        let validation = pollster::block_on(self.device.pop_error_scope());
        let oom = pollster::block_on(self.device.pop_error_scope());
        #[cfg(test)]
        if let Some(ref err) = validation {
            eprintln!("EdgeWalker validation error: {err}");
        }
        #[cfg(test)]
        if let Some(ref err) = oom {
            eprintln!("EdgeWalker OOM error: {err}");
        }
        if validation.is_some() || oom.is_some() || self.fault.load(Ordering::Acquire) {
            Err(edgewalker_trap())
        } else {
            Ok(())
        }
    }
}

fn edgewalker_trap() -> Trap {
    compute_unavailable()
}

fn instance_new() -> wgpu::Instance {
    wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN
            | wgpu::Backends::DX12
            | wgpu::Backends::METAL
            | wgpu::Backends::BROWSER_WEBGPU,
        flags: wgpu::InstanceFlags::from_build_config(),
        ..Default::default()
    })
}

fn adapter_options<'a>() -> wgpu::RequestAdapterOptions<'a, 'a> {
    wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }
}

fn vulkan_rank(adapter: &wgpu::Adapter) -> u8 {
    match adapter.get_info().device_type {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Cpu => 3,
        wgpu::DeviceType::Other => 4,
    }
}

fn open_adapter(adapter: &wgpu::Adapter, info: wgpu::AdapterInfo) -> Option<ComputeDevice> {
    let limits = adapter.limits();
    let opened = catch_trap(|| {
        pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("graycart-edgewalker"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
    })
    .ok()?;
    let (device, queue) = opened.ok()?;
    let fault = Arc::new(AtomicBool::new(false));
    install_handlers(&device, &fault);
    let _ = catch_trap(|| {
        device.poll(wgpu::Maintain::Wait);
    });
    if fault.load(Ordering::Acquire) {
        return None;
    }
    Some(ComputeDevice {
        device,
        queue,
        info,
        fault,
    })
}

fn install_handlers(device: &wgpu::Device, fault: &Arc<AtomicBool>) {
    let flag = Arc::clone(fault);
    device.on_uncaptured_error(Box::new(move |_error| {
        flag.store(true, Ordering::Release);
    }));
    let flag = Arc::clone(fault);
    device.set_device_lost_callback(move |_reason, _message| {
        flag.store(true, Ordering::Release);
    });
}

fn catch_trap<T>(op: impl FnOnce() -> T) -> Result<T, Trap> {
    match std::panic::catch_unwind(AssertUnwindSafe(op)) {
        Ok(value) => Ok(value),
        Err(_) => Err(edgewalker_trap()),
    }
}

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod tests;
