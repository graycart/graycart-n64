// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! EdgeWalker — Graycart's standalone bespoke Vulkan compute LLE RDP.
//!
//! [`EdgeWalker`] implements [`super::RdpBackend`]. It is the M6 GPU LLE:
//! pure Rust + WGSL, zero paraLLEl-RDP / Angrylion / GPL forks, zero C++ FFI,
//! and zero CPU fallback on the command path. Written directly from N64brew
//! RDP/VI hardware specs.
//!
//! **M6 gate:** bit-exact frame hash EdgeWalker == [`super::CpuRdpLle`].
//!
//! Without the `vulkan-rdp` feature, all execution paths return
//! [`Trap::UnimplementedRegister`] for device `EdgeWalker` at offset
//! [`EdgeWalker::COMPUTE_DISPATCH`]. With the feature, latched state updates on
//! host while TMEM DMA loads, triangle/rectangle rasterization, 1-/2-cycle
//! color combining, blending, Z-buffer operations, and 9th-bit coverage execute
//! on persistent Vulkan compute pipelines via `wgpu`.
//!
//! Cited: N64brew — Reality Display Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Interface
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//! Cross-check: docs/ORCHESTRATOR-HANDOFF.md (Milestone M6)

use serde::{Deserialize, Serialize};

use crate::bus::Rdram;
use crate::trap::Trap;

use super::tmem::TileDesc;
use super::{RdpBackend, RdpImageState, RdpScissor, RdpSyncKind};

#[cfg(feature = "vulkan-rdp")]
mod gpu;

#[cfg(feature = "vulkan-rdp")]
pub use gpu::ComputeDevice;

pub mod combine_gpu;
pub mod gpu_types;
pub mod parity;
/// Compute stages of EdgeWalker: ingestion, tile binning, span, and VI.
pub mod pipeline;
pub mod raster_gpu;
pub mod tmem_gpu;

#[cfg(test)]
mod parity_tests;

#[cfg(test)]
mod tests;

#[cfg(feature = "vulkan-rdp")]
#[derive(Debug)]
struct EdgeWalkerGpu {
    device: gpu::ComputeDevice,
    ingest_pipeline: wgpu::ComputePipeline,
    tmem_load_pipeline: wgpu::ComputePipeline,
    raster_pipeline: wgpu::ComputePipeline,
    tmem_load_params_buf: wgpu::Buffer,
    raster_params_buf: wgpu::Buffer,
    tiles_buf: wgpu::Buffer,
    tmem_buf: wgpu::Buffer,
    rdram_buf: std::sync::Mutex<Option<wgpu::Buffer>>,
    hidden_buf: std::sync::Mutex<Option<wgpu::Buffer>>,
}

/// Graycart's bespoke Vulkan compute LLE RDP.
///
/// **M6 gate:** bit-exact frame hash EdgeWalker == [`super::CpuRdpLle`].
///
/// Only hardware register/TMEM state is serialized. Persistent GPU pipelines
/// and VRAM mirrors live in `gpu` and are skipped by serde.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeWalker {
    /// `Set Color Image` (0x3F).
    color_image: RdpImageState,
    /// `Set Depth Image` (0x3E), DRAM address bits 23:0.
    #[serde(default)]
    depth_image_addr: u32,
    /// `Set Texture Image` (0x3D).
    #[serde(default)]
    texture_image: RdpImageState,
    /// `Set Other Modes` (0x2F), bits 55:0.
    other_modes: u64,
    /// `Set Fill Color` (0x37), bits 31:0.
    fill_color: u32,
    /// `Set Fog Color` (0x38), RGBA8888.
    #[serde(default)]
    fog_color: u32,
    /// `Set Blend Color` (0x39), RGBA8888.
    #[serde(default)]
    blend_color: u32,
    /// `Set Primitive Color` (0x3A), RGBA8888.
    #[serde(default)]
    prim_color: u32,
    /// `Set Primitive Color` min LOD, bits 47:40.
    #[serde(default)]
    prim_min_level: u8,
    /// `Set Primitive Color` LOD frac, bits 39:32.
    #[serde(default)]
    prim_lod_frac: u8,
    /// `Set Environment Color` (0x3B), RGBA8888.
    #[serde(default)]
    env_color: u32,
    /// `Set Combine Mode` (0x3C), bits 55:0.
    #[serde(default)]
    combine_mode: u64,
    /// `Set Primitive Depth` (0x2E) Z and delta-Z.
    #[serde(default)]
    prim_z: u16,
    #[serde(default)]
    prim_dz: u16,
    /// `Set Scissor` (0x2D).
    scissor: RdpScissor,
    /// Eight hardware tile descriptors (`Set Tile`, `Set Tile Size`).
    #[serde(default = "default_tiles")]
    tiles: [TileDesc; 8],
    /// Shadow copy of 4 KiB TMEM for deterministic serde serialization.
    #[serde(default = "default_tmem_shadow")]
    tmem_shadow: Vec<u8>,
    #[cfg(feature = "vulkan-rdp")]
    #[serde(skip)]
    gpu: Option<std::sync::Arc<EdgeWalkerGpu>>,
}

fn default_tiles() -> [TileDesc; 8] {
    [TileDesc::default(); 8]
}

fn default_tmem_shadow() -> Vec<u8> {
    vec![0u8; crate::rdp::tmem::TMEM_SIZE]
}

impl EdgeWalker {
    /// Offset named by [`Self::dispatch_compute`] when no compute device is linked.
    ///
    /// This is not a DPC/DPS register. The trap variant is
    /// [`Trap::UnimplementedRegister`] because that variant carries the device name.
    pub const COMPUTE_DISPATCH: u32 = 0x0001;

    #[must_use]
    pub fn new() -> Self {
        Self {
            color_image: RdpImageState::default(),
            depth_image_addr: 0,
            texture_image: RdpImageState::default(),
            other_modes: 0,
            fill_color: 0,
            fog_color: 0,
            blend_color: 0,
            prim_color: 0,
            prim_min_level: 0,
            prim_lod_frac: 0,
            env_color: 0,
            combine_mode: 0,
            prim_z: 0,
            prim_dz: 0,
            scissor: RdpScissor::default(),
            tiles: default_tiles(),
            tmem_shadow: default_tmem_shadow(),
            #[cfg(feature = "vulkan-rdp")]
            gpu: None,
        }
    }

    /// Open a device and run the ingest stage on one `Sync Full` word.
    ///
    /// Without `vulkan-rdp`, returns [`compute_unavailable`].
    pub fn dispatch_compute(&mut self) -> Result<(), Trap> {
        #[cfg(feature = "vulkan-rdp")]
        {
            let gpu = self.ensure_gpu()?;
            run_ingest_on_gpu(&gpu, &[u64::from(CMD_SYNC_FULL) << 56]).map(|_| ())
        }
        #[cfg(not(feature = "vulkan-rdp"))]
        {
            let _ = self;
            Err(compute_unavailable())
        }
    }

    #[cfg(feature = "vulkan-rdp")]
    fn ensure_gpu(&mut self) -> Result<std::sync::Arc<EdgeWalkerGpu>, Trap> {
        if let Some(ref gpu) = self.gpu {
            return Ok(std::sync::Arc::clone(gpu));
        }
        let device = gpu::ComputeDevice::open()?;
        let ingest_wgsl = pipeline::shader_wgsl(pipeline::Stage::Ingest);
        let ingest_pipeline = device.create_compute_pipeline(ingest_wgsl, "ingest_main")?;
        let tmem_load_pipeline =
            device.create_compute_pipeline(tmem_gpu::TMEM_LOAD_WGSL, "tmem_load_main")?;
        let raster_wgsl = raster_gpu::assemble_raster_wgsl(
            tmem_gpu::TMEM_SAMPLE_WGSL,
            combine_gpu::COMBINE_BLEND_WGSL,
        );
        let raster_pipeline = device.create_compute_pipeline(&raster_wgsl, "raster_main")?;

        let tmem_load_params_buf =
            device.create_storage_buffer(gpu_types::TMEM_LOAD_PARAMS_BYTES as u64)?;
        let raster_params_buf =
            device.create_storage_buffer(gpu_types::RASTER_PARAMS_BYTES as u64)?;
        let tiles_buf = device.create_storage_buffer(gpu_types::ALL_TILES_BYTES as u64)?;
        let tmem_buf = device.create_storage_buffer(crate::rdp::tmem::TMEM_SIZE as u64)?;
        if self.tmem_shadow.len() == crate::rdp::tmem::TMEM_SIZE {
            device.write_buffer(&tmem_buf, 0, &self.tmem_shadow)?;
        }

        let gpu_state = std::sync::Arc::new(EdgeWalkerGpu {
            device,
            ingest_pipeline,
            tmem_load_pipeline,
            raster_pipeline,
            tmem_load_params_buf,
            raster_params_buf,
            tiles_buf,
            tmem_buf,
            rdram_buf: std::sync::Mutex::new(None),
            hidden_buf: std::sync::Mutex::new(None),
        });
        self.gpu = Some(std::sync::Arc::clone(&gpu_state));
        Ok(gpu_state)
    }

    #[cfg(feature = "vulkan-rdp")]
    fn submit_gpu(&mut self, cmds: &[u64], rdram: &mut Rdram) -> Result<(), Trap> {
        if cmds.is_empty() {
            return Ok(());
        }
        let gpu = self.ensure_gpu()?;
        let barriers = run_ingest_on_gpu(&gpu, cmds)?;
        let mut batch = GpuBatch {
            tiles_dirty: true,
            ..Default::default()
        };

        let mut start_idx = 0;
        for (kind, idx) in barriers {
            if kind == pipeline::INGEST_KIND_NONE {
                continue;
            }
            let end_idx = (idx as usize) + 1;
            if end_idx > start_idx && end_idx <= cmds.len() {
                self.exec_words(&gpu, &cmds[start_idx..end_idx], rdram, &mut batch)?;
                start_idx = end_idx;
            }
        }
        if start_idx < cmds.len() {
            self.exec_words(&gpu, &cmds[start_idx..], rdram, &mut batch)?;
        }

        if batch.rdram_dirty {
            readback_rdram(&gpu, rdram)?;
        }
        if batch.tmem_dirty {
            let tmem_back = gpu.device.readback(&gpu.tmem_buf)?;
            if tmem_back.len() == self.tmem_shadow.len() {
                self.tmem_shadow.copy_from_slice(&tmem_back);
            }
        }
        Ok(())
    }

    #[cfg(feature = "vulkan-rdp")]
    fn exec_words(
        &mut self,
        gpu: &EdgeWalkerGpu,
        cmds: &[u64],
        rdram: &Rdram,
        batch: &mut GpuBatch,
    ) -> Result<(), Trap> {
        let mut index = 0;
        while index < cmds.len() {
            let word = cmds[index];
            let cmd = opcode(word);
            let len = raster_gpu::primitive_word_count(word);
            if index + len > cmds.len() {
                return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
            }
            self.apply(gpu, &cmds[index..index + len], rdram, batch)?;
            index += len;
        }
        Ok(())
    }

    #[cfg(feature = "vulkan-rdp")]
    fn apply(
        &mut self,
        gpu: &EdgeWalkerGpu,
        words: &[u64],
        rdram: &Rdram,
        batch: &mut GpuBatch,
    ) -> Result<(), Trap> {
        let word = words[0];
        let cmd = opcode(word);
        if is_nop(cmd) || pipeline::sync_kind(word).is_some() {
            return Ok(());
        }
        match cmd {
            CMD_SET_SCISSOR => {
                self.scissor = scissor_from_word(word);
                Ok(())
            }
            CMD_SET_PRIM_DEPTH => {
                self.prim_z = ((word >> 16) & 0xFFFF) as u16;
                self.prim_dz = (word & 0xFFFF) as u16;
                Ok(())
            }
            CMD_SET_OTHER_MODES => {
                self.other_modes = word & CMD_PAYLOAD_MASK;
                Ok(())
            }
            CMD_SET_FILL_COLOR => {
                self.fill_color = word as u32;
                Ok(())
            }
            CMD_SET_FOG_COLOR => {
                self.fog_color = word as u32;
                Ok(())
            }
            CMD_SET_BLEND_COLOR => {
                self.blend_color = word as u32;
                Ok(())
            }
            CMD_SET_PRIM_COLOR => {
                self.prim_min_level = ((word >> 40) & 0xFF) as u8;
                self.prim_lod_frac = ((word >> 32) & 0xFF) as u8;
                self.prim_color = word as u32;
                Ok(())
            }
            CMD_SET_ENV_COLOR => {
                self.env_color = word as u32;
                Ok(())
            }
            CMD_SET_COMBINE => {
                self.combine_mode = word & CMD_PAYLOAD_MASK;
                Ok(())
            }
            CMD_SET_TEXTURE_IMAGE => {
                self.texture_image = image_from_word(word);
                Ok(())
            }
            CMD_SET_DEPTH_IMAGE => {
                self.depth_image_addr = (word & 0x00FF_FFFF) as u32;
                Ok(())
            }
            CMD_SET_COLOR_IMAGE => {
                self.color_image = image_from_word(word);
                Ok(())
            }
            CMD_SET_TILE => {
                tmem_gpu::decode_set_tile(&mut self.tiles, word)?;
                batch.tiles_dirty = true;
                Ok(())
            }
            CMD_SET_TILE_SIZE => {
                tmem_gpu::decode_set_tile_size(&mut self.tiles, word)?;
                batch.tiles_dirty = true;
                Ok(())
            }
            CMD_LOAD_TILE => {
                let params = tmem_gpu::decode_load_tile(&mut self.tiles, self.texture_image, word)?;
                batch.tiles_dirty = true;
                let threads = params.width.saturating_mul(params.height);
                dispatch_tmem_load(gpu, &params, threads, rdram, batch)
            }
            CMD_LOAD_BLOCK => {
                let Some(params) =
                    tmem_gpu::decode_load_block(&mut self.tiles, self.texture_image, word)?
                else {
                    batch.tiles_dirty = true;
                    return Ok(());
                };
                batch.tiles_dirty = true;
                let threads = params.width;
                dispatch_tmem_load(gpu, &params, threads, rdram, batch)
            }
            CMD_LOAD_TLUT => {
                let params = tmem_gpu::decode_load_tlut(&mut self.tiles, self.texture_image, word)?;
                batch.tiles_dirty = true;
                let threads = params.width.saturating_mul(params.height);
                dispatch_tmem_load(gpu, &params, threads, rdram, batch)
            }
            0x08..=0x0F | CMD_TEX_RECT | CMD_TEX_RECT_FLIP | CMD_FILL_RECT => {
                let state = raster_gpu::HostRasterState {
                    color_image: self.color_image,
                    depth_image_addr: self.depth_image_addr,
                    scissor: self.scissor,
                    other_modes: self.other_modes,
                    combine_mode: self.combine_mode,
                    fill_color: self.fill_color,
                    prim_color: self.prim_color,
                    env_color: self.env_color,
                    fog_color: self.fog_color,
                    blend_color: self.blend_color,
                    prim_lod_frac: self.prim_lod_frac,
                    prim_z: self.prim_z,
                    prim_dz: self.prim_dz,
                    convert_k4: 0,
                    convert_k5: 0,
                    key_center: [0; 3],
                    key_scale: [0; 3],
                    tiles: &self.tiles,
                };
                let Some((params, threads)) = raster_gpu::prepare_primitive(&state, words)? else {
                    return Ok(());
                };
                if threads == 0 {
                    return Ok(());
                }
                dispatch_raster_on_gpu(gpu, &self.tiles, &params, threads, rdram, batch)
            }
            _ => Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word }),
        }
    }

    #[cfg(feature = "vulkan-rdp")]
    fn sync_gpu(&mut self, kind: RdpSyncKind, rdram: &mut Rdram) -> Result<(), Trap> {
        let _ = (self, rdram);
        match kind {
            RdpSyncKind::Pipe | RdpSyncKind::Tile | RdpSyncKind::Load | RdpSyncKind::Full => Ok(()),
        }
    }
}

impl Default for EdgeWalker {
    fn default() -> Self {
        Self::new()
    }
}

impl RdpBackend for EdgeWalker {
    fn submit(&mut self, cmds: &[u64], rdram: &mut Rdram) -> Result<(), Trap> {
        #[cfg(feature = "vulkan-rdp")]
        {
            self.submit_gpu(cmds, rdram)
        }
        #[cfg(not(feature = "vulkan-rdp"))]
        {
            let _ = (self, cmds, rdram);
            Err(compute_unavailable())
        }
    }

    fn sync(&mut self, kind: RdpSyncKind, rdram: &mut Rdram) -> Result<(), Trap> {
        #[cfg(feature = "vulkan-rdp")]
        {
            self.sync_gpu(kind, rdram)
        }
        #[cfg(not(feature = "vulkan-rdp"))]
        {
            let _ = (self, kind, rdram);
            Err(compute_unavailable())
        }
    }
}

fn compute_unavailable() -> Trap {
    Trap::UnimplementedRegister {
        device: "EdgeWalker",
        offset: EdgeWalker::COMPUTE_DISPATCH,
        is_write: true,
    }
}

#[cfg(feature = "vulkan-rdp")]
#[derive(Default)]
struct GpuBatch {
    rdram_uploaded: bool,
    rdram_dirty: bool,
    tmem_dirty: bool,
    tiles_dirty: bool,
}

#[cfg(feature = "vulkan-rdp")]
const CMD_TEX_RECT: u8 = 0x24;
#[cfg(feature = "vulkan-rdp")]
const CMD_TEX_RECT_FLIP: u8 = 0x25;
#[cfg(feature = "vulkan-rdp")]
const CMD_SYNC_FULL: u8 = 0x29;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_SCISSOR: u8 = 0x2D;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_PRIM_DEPTH: u8 = 0x2E;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_OTHER_MODES: u8 = 0x2F;
#[cfg(feature = "vulkan-rdp")]
const CMD_LOAD_TLUT: u8 = 0x30;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_TILE_SIZE: u8 = 0x32;
#[cfg(feature = "vulkan-rdp")]
const CMD_LOAD_BLOCK: u8 = 0x33;
#[cfg(feature = "vulkan-rdp")]
const CMD_LOAD_TILE: u8 = 0x34;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_TILE: u8 = 0x35;
#[cfg(feature = "vulkan-rdp")]
const CMD_FILL_RECT: u8 = 0x36;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_FILL_COLOR: u8 = 0x37;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_FOG_COLOR: u8 = 0x38;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_BLEND_COLOR: u8 = 0x39;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_PRIM_COLOR: u8 = 0x3A;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_ENV_COLOR: u8 = 0x3B;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_COMBINE: u8 = 0x3C;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_TEXTURE_IMAGE: u8 = 0x3D;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_DEPTH_IMAGE: u8 = 0x3E;
#[cfg(feature = "vulkan-rdp")]
const CMD_SET_COLOR_IMAGE: u8 = 0x3F;
#[cfg(feature = "vulkan-rdp")]
const CMD_PAYLOAD_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;

#[cfg(feature = "vulkan-rdp")]
fn opcode(word: u64) -> u8 {
    ((word >> 56) & 0x3F) as u8
}

#[cfg(feature = "vulkan-rdp")]
fn is_nop(cmd: u8) -> bool {
    matches!(cmd, 0x00..=0x07 | 0x10..=0x23 | 0x31)
}

#[cfg(feature = "vulkan-rdp")]
fn image_from_word(word: u64) -> RdpImageState {
    RdpImageState {
        format: ((word >> 53) & 0x7) as u8,
        size: ((word >> 51) & 0x3) as u8,
        width: ((word >> 32) & 0x3FF) as u16,
        addr: (word & 0x00FF_FFFF) as u32,
    }
}

#[cfg(feature = "vulkan-rdp")]
fn scissor_from_word(word: u64) -> RdpScissor {
    RdpScissor {
        ulx: ((word >> 44) & 0xFFF) as u16,
        uly: ((word >> 32) & 0xFFF) as u16,
        field: (word & (1 << 25)) != 0,
        odd: (word & (1 << 24)) != 0,
        lrx: ((word >> 12) & 0xFFF) as u16,
        lry: (word & 0xFFF) as u16,
    }
}

#[cfg(feature = "vulkan-rdp")]
fn ensure_rdram_buffers(
    gpu: &EdgeWalkerGpu,
    rdram: &Rdram,
    batch: &mut GpuBatch,
) -> Result<(wgpu::Buffer, wgpu::Buffer), Trap> {
    let data_len = u64::try_from(rdram.len()).map_err(|_| compute_unavailable())?;
    let hidden_len =
        u64::try_from(rdram.hidden_bits_as_slice().len()).map_err(|_| compute_unavailable())?;
    if data_len == 0
        || !data_len.is_multiple_of(4)
        || hidden_len == 0
        || !hidden_len.is_multiple_of(4)
    {
        return Err(compute_unavailable());
    }

    let rdram_buf = {
        let mut lock = gpu.rdram_buf.lock().unwrap();
        if lock.as_ref().is_none_or(|b| b.size() != data_len) {
            *lock = Some(gpu.device.create_storage_buffer(data_len)?);
        }
        lock.as_ref().unwrap().clone()
    };
    let hidden_buf = {
        let mut lock = gpu.hidden_buf.lock().unwrap();
        if lock.as_ref().is_none_or(|b| b.size() != hidden_len) {
            *lock = Some(gpu.device.create_storage_buffer(hidden_len)?);
        }
        lock.as_ref().unwrap().clone()
    };

    if !batch.rdram_uploaded {
        gpu.device.write_buffer(&rdram_buf, 0, rdram.as_slice())?;
        gpu.device
            .write_buffer(&hidden_buf, 0, rdram.hidden_bits_as_slice())?;
        batch.rdram_uploaded = true;
    }

    Ok((rdram_buf, hidden_buf))
}

#[cfg(feature = "vulkan-rdp")]
fn readback_rdram(gpu: &EdgeWalkerGpu, rdram: &mut Rdram) -> Result<(), Trap> {
    let rdram_buf = gpu
        .rdram_buf
        .lock()
        .unwrap()
        .as_ref()
        .cloned()
        .ok_or_else(compute_unavailable)?;
    let hidden_buf = gpu
        .hidden_buf
        .lock()
        .unwrap()
        .as_ref()
        .cloned()
        .ok_or_else(compute_unavailable)?;

    let back_data = gpu.device.readback(&rdram_buf)?;
    let back_hidden = gpu.device.readback(&hidden_buf)?;
    if back_data.len() != rdram.len() || back_hidden.len() != rdram.hidden_bits_len() {
        return Err(compute_unavailable());
    }
    rdram.as_mut_slice().copy_from_slice(&back_data);
    rdram
        .hidden_bits_as_mut_slice()
        .copy_from_slice(&back_hidden);
    Ok(())
}

#[cfg(feature = "vulkan-rdp")]
fn dispatch_tmem_load(
    gpu: &EdgeWalkerGpu,
    params: &gpu_types::TmemLoadParamsPack,
    threads: u32,
    rdram: &Rdram,
    batch: &mut GpuBatch,
) -> Result<(), Trap> {
    if threads == 0 {
        return Ok(());
    }
    let (rdram_buf, _) = ensure_rdram_buffers(gpu, rdram, batch)?;
    gpu.device
        .write_buffer(&gpu.tmem_load_params_buf, 0, &params.to_bytes())?;
    let group = gpu.device.create_storage_bind_group(
        &gpu.tmem_load_pipeline,
        0,
        &[
            (0, &gpu.tmem_load_params_buf),
            (1, &rdram_buf),
            (2, &gpu.tmem_buf),
        ],
    )?;
    let groups = threads.div_ceil(pipeline::WORKGROUP_SIZE);
    gpu.device
        .dispatch_workgroups(&gpu.tmem_load_pipeline, Some(&group), groups, 1, 1)?;
    batch.tmem_dirty = true;
    Ok(())
}

#[cfg(feature = "vulkan-rdp")]
fn dispatch_raster_on_gpu(
    gpu: &EdgeWalkerGpu,
    tiles: &[TileDesc; 8],
    params: &gpu_types::RasterParamsPack,
    threads: u32,
    rdram: &Rdram,
    batch: &mut GpuBatch,
) -> Result<(), Trap> {
    let (rdram_buf, hidden_buf) = ensure_rdram_buffers(gpu, rdram, batch)?;
    gpu.device
        .write_buffer(&gpu.raster_params_buf, 0, &params.to_bytes())?;
    if batch.tiles_dirty {
        let packed_tiles = gpu_types::GpuTileDescPack::pack_all_tiles(tiles);
        gpu.device.write_buffer(&gpu.tiles_buf, 0, &packed_tiles)?;
        batch.tiles_dirty = false;
    }

    let group = gpu.device.create_storage_bind_group(
        &gpu.raster_pipeline,
        0,
        &[
            (0, &gpu.raster_params_buf),
            (1, &gpu.tiles_buf),
            (2, &gpu.tmem_buf),
            (3, &rdram_buf),
            (4, &hidden_buf),
        ],
    )?;
    let groups = threads.div_ceil(pipeline::WORKGROUP_SIZE);
    gpu.device
        .dispatch_workgroups(&gpu.raster_pipeline, Some(&group), groups, 1, 1)?;
    batch.rdram_dirty = true;
    Ok(())
}

#[cfg(feature = "vulkan-rdp")]
fn run_ingest_on_gpu(gpu: &EdgeWalkerGpu, cmds: &[u64]) -> Result<Vec<(u32, u32)>, Trap> {
    pipeline::Pipeline::new().dispatch_ingest_with_pipeline(&gpu.device, &gpu.ingest_pipeline, cmds)
}
