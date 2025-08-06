//! Built-in GPU mining worker using wgpu
//!
//! This module provides a native GPU mining implementation using WebGPU/wgpu
//! for cross-platform GPU compute without requiring external processes.

use crate::core::{Nonce, Target, Work};
use crate::error::{Error, Result};
use crate::workers::{MiningResult, Worker};
use async_trait::async_trait;
use parking_lot::Mutex;
use std::mem::size_of;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use wgpu::util::DeviceExt;

/// GPU mining configuration
#[derive(Debug, Clone)]
pub struct GpuConfig {
    /// Device index to use (None = auto-select best device)
    pub device_index: Option<usize>,
    /// Workgroup size (number of threads per workgroup)
    pub workgroup_size: u32,
    /// Number of workgroups to dispatch
    pub workgroup_count: u32,
    /// Maximum nonces to process in one batch
    pub batch_size: u32,
    /// Enable GPU performance monitoring
    pub enable_monitoring: bool,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            device_index: None,
            workgroup_size: 256,
            workgroup_count: 1024,
            batch_size: 256 * 1024, // 256k nonces per batch
            enable_monitoring: true,
        }
    }
}

/// Work data structure for GPU (must be 16-byte aligned)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuWorkData {
    // Split into smaller arrays that bytemuck can handle
    data_part1: [u32; 64], // First 256 bytes
    data_part2: [u32; 8],  // Remaining 32 bytes (total 288 bytes)
}

/// Mining parameters for GPU
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuMiningParams {
    target: [u32; 8],    // 256-bit target
    start_nonce: u32,
    nonce_count: u32,
    nonce_offset: u32,   // Offset where nonce should be placed (278/4 = 69)
    padding: u32,
}

/// Mining result from GPU
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuMiningResult {
    found: u32,          // 0 = not found, 1 = found
    nonce: u32,
    hash: [u32; 8],      // 256-bit hash
}

/// Built-in GPU mining worker using wgpu
#[derive(Clone)]
pub struct GpuWorker {
    config: GpuConfig,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: Arc<wgpu::ComputePipeline>,
    bind_group_layout: Arc<wgpu::BindGroupLayout>,
    is_mining: Arc<AtomicBool>,
    hash_count: Arc<AtomicU64>,
    last_hashrate_time: Arc<Mutex<Instant>>,
    adapter_name: String,
}

impl GpuWorker {
    /// Create a new GPU worker
    pub async fn new(config: GpuConfig) -> Result<Self> {
        info!("Initializing wgpu GPU worker");
        
        // Create wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        // Enumerate available adapters
        let adapters: Vec<_> = instance.enumerate_adapters(wgpu::Backends::all());
        
        if adapters.is_empty() {
            return Err(Error::worker_initialization_failed(
                "GPU",
                "No GPU adapters found"
            ));
        }
        
        // Select adapter
        let adapter = if let Some(index) = config.device_index {
            adapters.get(index).ok_or_else(|| {
                Error::worker_initialization_failed(
                    "GPU",
                    format!("GPU device index {} not found", index)
                )
            })?
        } else {
            // Auto-select: prefer discrete GPU
            adapters
                .iter()
                .find(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu)
                .unwrap_or(&adapters[0])
        };
        
        let adapter_info = adapter.get_info();
        info!(
            "Selected GPU: {} ({:?})",
            adapter_info.name, adapter_info.device_type
        );
        
        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Chainweb Mining GPU"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| {
                Error::worker_initialization_failed(
                    "GPU",
                    format!("Failed to request GPU device: {}", e)
                )
            })?;
        
        let device: Arc<wgpu::Device> = Arc::new(device);
        let queue: Arc<wgpu::Queue> = Arc::new(queue);
        
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blake2s Mining Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/blake2s.wgsl").into()),
        });
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mining Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        
        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mining Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Blake2s Mining Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("mine"),
            compilation_options: Default::default(),
            cache: None,
        });
        
        Ok(Self {
            config,
            device,
            queue,
            pipeline: Arc::new(pipeline),
            bind_group_layout: Arc::new(bind_group_layout),
            is_mining: Arc::new(AtomicBool::new(false)),
            hash_count: Arc::new(AtomicU64::new(0)),
            last_hashrate_time: Arc::new(Mutex::new(Instant::now())),
            adapter_name: adapter_info.name,
        })
    }
    
    /// Prepare work data for GPU
    fn prepare_work_data(&self, work: &Work) -> GpuWorkData {
        let mut data_part1 = [0u32; 64];
        let mut data_part2 = [0u32; 8];
        let work_bytes = work.as_bytes();
        
        // Convert work bytes to u32 arrays (little-endian)
        for i in 0..72 {
            let offset = i * 4;
            let value = if offset + 3 < work_bytes.len() {
                u32::from_le_bytes([
                    work_bytes[offset],
                    work_bytes[offset + 1],
                    work_bytes[offset + 2],
                    work_bytes[offset + 3],
                ])
            } else if offset < work_bytes.len() {
                // Handle last partial word
                let mut bytes = [0u8; 4];
                let remaining = work_bytes.len() - offset;
                bytes[..remaining].copy_from_slice(&work_bytes[offset..]);
                u32::from_le_bytes(bytes)
            } else {
                0
            };
            
            if i < 64 {
                data_part1[i] = value;
            } else {
                data_part2[i - 64] = value;
            }
        }
        
        GpuWorkData { data_part1, data_part2 }
    }
    
    /// Prepare target for GPU
    fn prepare_target(&self, target: &Target) -> [u32; 8] {
        let mut gpu_target = [0u32; 8];
        let target_bytes = target.as_bytes();
        
        // Convert target bytes to u32 array (little-endian)
        for i in 0..8 {
            let offset = i * 4;
            gpu_target[i] = u32::from_le_bytes([
                target_bytes[offset],
                target_bytes[offset + 1],
                target_bytes[offset + 2],
                target_bytes[offset + 3],
            ]);
        }
        
        gpu_target
    }
    
    /// Mine a batch of nonces on GPU
    async fn mine_batch(
        &self,
        work: &Work,
        target: &Target,
        start_nonce: u64,
        batch_size: u32,
    ) -> Result<Option<MiningResult>> {
        // Prepare data
        let work_data = self.prepare_work_data(work);
        let gpu_target = self.prepare_target(target);
        let params = GpuMiningParams {
            target: gpu_target,
            start_nonce: start_nonce as u32,
            nonce_count: batch_size,
            nonce_offset: 69, // 278 / 4 = 69.5, rounded down
            padding: 0,
        };
        
        // Create buffers
        let work_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Work Buffer"),
            contents: bytemuck::cast_slice(&[work_data]),
            usage: wgpu::BufferUsages::STORAGE,
        });
        
        let params_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Params Buffer"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        
        let result_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Result Buffer"),
            contents: bytemuck::cast_slice(&[GpuMiningResult {
                found: 0,
                nonce: 0,
                hash: [0; 8],
            }]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
        
        // Create staging buffer for reading results
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: size_of::<GpuMiningResult>() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mining Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: work_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: result_buffer.as_entire_binding(),
                },
            ],
        });
        
        // Create command encoder
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Mining Encoder"),
        });
        
        // Dispatch compute work
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Mining Pass"),
                timestamp_writes: None,
            });
            
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            
            // Calculate dispatch size
            let threads_per_workgroup = self.config.workgroup_size;
            let total_threads = batch_size;
            let num_workgroups = (total_threads + threads_per_workgroup - 1) / threads_per_workgroup;
            
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }
        
        // Copy result to staging buffer
        encoder.copy_buffer_to_buffer(
            &result_buffer,
            0,
            &staging_buffer,
            0,
            size_of::<GpuMiningResult>() as u64,
        );
        
        // Submit work
        self.queue.submit(std::iter::once(encoder.finish()));
        
        // Read results
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        
        self.device.poll(wgpu::Maintain::Wait);
        
        rx.await
            .map_err(|_| Error::worker("GPU result channel closed"))?
            .map_err(|e| Error::worker(format!("Failed to map GPU buffer: {:?}", e)))?;
        
        let data = buffer_slice.get_mapped_range();
        let result: &GpuMiningResult = bytemuck::from_bytes(&data);
        
        // Update hash count
        self.hash_count.fetch_add(batch_size as u64, Ordering::Relaxed);
        
        if result.found != 0 {
            // Found a solution!
            let nonce = Nonce::new(result.nonce as u64);
            let mut hash = [0u8; 32];
            
            // Convert hash from u32 array to byte array
            for i in 0..8 {
                let bytes = result.hash[i].to_le_bytes();
                hash[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
            }
            
            let mut solved_work = work.clone();
            solved_work.set_nonce(nonce);
            
            Ok(Some(MiningResult {
                work: solved_work,
                nonce,
                hash,
            }))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl Worker for GpuWorker {
    async fn mine(
        &self,
        work: Work,
        target: Target,
        result_tx: mpsc::Sender<MiningResult>,
    ) -> Result<()> {
        if self.is_mining.load(Ordering::Relaxed) {
            return Err(Error::worker("Already mining"));
        }
        
        self.is_mining.store(true, Ordering::Relaxed);
        self.hash_count.store(0, Ordering::Relaxed);
        *self.last_hashrate_time.lock() = Instant::now();
        
        info!("Starting GPU mining on {}", self.adapter_name);
        
        let is_mining = self.is_mining.clone();
        let batch_size = self.config.batch_size;
        let worker = self.clone();
        
        tokio::spawn(async move {
            let mut nonce = 0u64;
            
            while is_mining.load(Ordering::Relaxed) {
                match worker.mine_batch(&work, &target, nonce, batch_size).await {
                    Ok(Some(result)) => {
                        info!("GPU found solution: nonce={}", result.nonce);
                        let _ = result_tx.send(result).await;
                        break;
                    }
                    Ok(None) => {
                        // No solution in this batch, continue
                        nonce += batch_size as u64;
                        
                        // Check for nonce overflow
                        if nonce > u64::MAX - batch_size as u64 {
                            warn!("Nonce space exhausted");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("GPU mining error: {}", e);
                        break;
                    }
                }
            }
            
            is_mining.store(false, Ordering::Relaxed);
        });
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        self.is_mining.store(false, Ordering::Relaxed);
        Ok(())
    }
    
    fn worker_type(&self) -> &str {
        "GPU"
    }
    
    async fn hashrate(&self) -> u64 {
        let hashes = self.hash_count.load(Ordering::Relaxed);
        let elapsed = self.last_hashrate_time.lock().elapsed();
        
        if elapsed.as_secs() == 0 {
            return 0;
        }
        
        // Reset counters for next measurement
        self.hash_count.store(0, Ordering::Relaxed);
        *self.last_hashrate_time.lock() = Instant::now();
        
        (hashes as f64 / elapsed.as_secs_f64()) as u64
    }
}

/// Enumerate available GPU devices
pub async fn enumerate_gpus() -> Vec<(usize, String, wgpu::DeviceType)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    
    instance
        .enumerate_adapters(wgpu::Backends::all())
        .into_iter()
        .enumerate()
        .map(|(i, adapter)| {
            let info = adapter.get_info();
            (i, info.name, info.device_type)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_enumerate_gpus() {
        let gpus = enumerate_gpus().await;
        // This test will pass even if no GPUs are found
        for (index, name, device_type) in &gpus {
            println!("GPU {}: {} ({:?})", index, name, device_type);
        }
    }
    
    #[test]
    fn test_gpu_config() {
        let config = GpuConfig::default();
        assert_eq!(config.workgroup_size, 256);
        assert_eq!(config.workgroup_count, 1024);
    }
}