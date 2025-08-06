//! GPU enumeration example
//! 
//! This example shows how to enumerate available GPUs for mining

use chainweb_mining_client::workers::gpu::enumerate_gpus;

#[tokio::main]
async fn main() {
    println!("Enumerating available GPUs...\n");
    
    let gpus = enumerate_gpus().await;
    
    if gpus.is_empty() {
        println!("No GPU devices found!");
        println!("\nMake sure you have:");
        println!("- A GPU installed");
        println!("- Appropriate GPU drivers installed");
        println!("- Vulkan, Metal, or DirectX 12 support");
        return;
    }
    
    println!("Found {} GPU device(s):\n", gpus.len());
    
    for (index, name, device_type) in &gpus {
        println!("GPU {}: {}", index, name);
        println!("  Type: {:?}", device_type);
        
        // Recommend based on device type
        match device_type {
            wgpu::DeviceType::DiscreteGpu => {
                println!("  ✓ Recommended for mining (discrete GPU)");
            }
            wgpu::DeviceType::IntegratedGpu => {
                println!("  ⚠ Integrated GPU - lower performance expected");
            }
            wgpu::DeviceType::VirtualGpu => {
                println!("  ⚠ Virtual GPU - performance may vary");
            }
            wgpu::DeviceType::Cpu => {
                println!("  ✗ CPU fallback - not recommended for mining");
            }
            wgpu::DeviceType::Other => {
                println!("  ? Unknown device type");
            }
        }
        println!();
    }
    
    println!("\nTo use a specific GPU, set 'device_index' in your configuration:");
    println!("  [worker]");
    println!("  type = \"gpu\"");
    println!("  device_index = 0  # Use the first GPU");
    println!("\nOr omit 'device_index' to auto-select the best GPU.");
}