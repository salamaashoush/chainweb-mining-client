# GPU Miner Protocol Specification

This document describes the JSON-based protocol used for communication between the chainweb mining client and external GPU mining processes.

## Overview

The GPU miner operates as a separate process that communicates with the main mining client via stdin/stdout using line-delimited JSON messages. This design allows:

- Safe GPU mining without unsafe Rust code
- Support for multiple GPU implementations (CUDA, OpenCL, ROCm)
- Language-agnostic GPU miner implementations
- Easy integration with existing GPU mining software

## Message Format

All messages are JSON objects with a `type` field, followed by a newline character.

### Request Messages (Client → GPU Miner)

#### Initialize
```json
{
  "type": "init",
  "gpu_indices": [0, 1],  // Optional: specific GPUs to use
  "batch_size": 1000000   // Nonces per batch
}
```

#### Mine
```json
{
  "type": "mine",
  "work": "0x...",        // 286 bytes hex encoded
  "target": "0x...",      // 32 bytes hex encoded
  "start_nonce": 0,       // Starting nonce value
  "nonce_count": 1000000  // Number of nonces to check
}
```

#### Stop
```json
{
  "type": "stop"
}
```

#### Get Info
```json
{
  "type": "info"
}
```

#### Shutdown
```json
{
  "type": "shutdown"
}
```

### Response Messages (GPU Miner → Client)

#### Initialized
```json
{
  "type": "initialized",
  "gpu_count": 2,
  "total_memory": 16777216000,  // Total GPU memory in bytes
  "max_batch_size": 10000000    // Maximum supported batch size
}
```

#### Solution Found
```json
{
  "type": "solution",
  "nonce": 12345678,
  "hash": "0x..."  // 32 bytes hex encoded
}
```

#### Batch Complete
```json
{
  "type": "complete",
  "hashes_computed": 1000000,
  "duration_ms": 250
}
```

#### GPU Information
```json
{
  "type": "info",
  "gpus": [
    {
      "index": 0,
      "name": "NVIDIA GeForce RTX 3090",
      "memory": 25769803776,
      "compute_capability": "8.6",
      "temperature": 65.0,
      "utilization": 98.5
    }
  ]
}
```

#### Error
```json
{
  "type": "error",
  "message": "CUDA error: out of memory"
}
```

## Implementation Example

Here's a minimal Python example of a GPU miner that implements this protocol:

```python
#!/usr/bin/env python3
import json
import sys
import hashlib

def process_message(msg):
    msg_type = msg.get("type")
    
    if msg_type == "init":
        return {
            "type": "initialized",
            "gpu_count": 1,
            "total_memory": 8589934592,
            "max_batch_size": 1000000
        }
    
    elif msg_type == "mine":
        # Simplified example - real implementation would use GPU
        work_hex = msg["work"]
        target_hex = msg["target"]
        start_nonce = msg["start_nonce"]
        nonce_count = msg["nonce_count"]
        
        # Simulate mining (would be GPU kernel in real implementation)
        # ...
        
        return {
            "type": "complete",
            "hashes_computed": nonce_count,
            "duration_ms": 100
        }
    
    elif msg_type == "shutdown":
        sys.exit(0)
    
    return {"type": "error", "message": "Unknown message type"}

def main():
    while True:
        line = sys.stdin.readline().strip()
        if not line:
            break
        
        try:
            request = json.loads(line)
            response = process_message(request)
            print(json.dumps(response))
            sys.stdout.flush()
        except Exception as e:
            error = {"type": "error", "message": str(e)}
            print(json.dumps(error))
            sys.stdout.flush()

if __name__ == "__main__":
    main()
```

## CUDA Implementation Notes

A real GPU miner would implement Blake2s-256 in CUDA/OpenCL:

```cuda
__global__ void blake2s_mine_kernel(
    const uint8_t* work,
    const uint8_t* target,
    uint64_t start_nonce,
    uint64_t* result_nonce,
    uint8_t* result_hash
) {
    uint64_t nonce = blockIdx.x * blockDim.x + threadIdx.x + start_nonce;
    
    // Copy work to local memory
    uint8_t local_work[286];
    memcpy(local_work, work, 286);
    
    // Set nonce
    memcpy(&local_work[278], &nonce, 8);
    
    // Compute Blake2s-256
    uint8_t hash[32];
    blake2s(hash, local_work, 286);
    
    // Check target
    if (meets_target(hash, target)) {
        *result_nonce = nonce;
        memcpy(result_hash, hash, 32);
    }
}
```

## Performance Considerations

1. **Batch Size**: Larger batches reduce communication overhead but increase latency
2. **Memory Management**: Pin host memory for faster GPU transfers
3. **Multi-GPU**: Distribute work across GPUs with load balancing
4. **Async Operations**: Use CUDA streams for overlapping computation and communication

## Security Considerations

1. Validate all input data (work size, target format)
2. Implement timeouts to prevent hanging
3. Sanitize error messages to avoid leaking sensitive information
4. Consider process isolation for untrusted GPU miners