#!/usr/bin/env python3
"""
GPU Miner Stub Implementation

This is a reference implementation of the GPU miner protocol for testing.
A real implementation would use CUDA/OpenCL for actual GPU mining.
"""

import json
import sys
import time
import random
from datetime import datetime

class GPUMinerStub:
    def __init__(self):
        self.is_mining = False
        self.gpu_info = {
            "index": 0,
            "name": "Simulated GPU",
            "memory": 8 * 1024 * 1024 * 1024,  # 8GB
            "compute_capability": "7.5",
            "temperature": 65.0,
            "utilization": 0.0
        }
        self.hash_rate = 1_000_000_000  # 1 GH/s simulated
        
    def log(self, message):
        """Log to stderr to avoid interfering with protocol"""
        print(f"[{datetime.now().isoformat()}] {message}", file=sys.stderr)
        
    def handle_init(self, msg):
        """Handle initialization request"""
        self.log(f"Initializing with batch_size={msg.get('batch_size', 1000000)}")
        
        return {
            "type": "initialized",
            "gpu_count": 1,
            "total_memory": self.gpu_info["memory"],
            "max_batch_size": 10_000_000
        }
    
    def handle_mine(self, msg):
        """Handle mining request (simulated)"""
        work_hex = msg["work"]
        target_hex = msg["target"]
        start_nonce = msg["start_nonce"]
        nonce_count = msg["nonce_count"]
        
        self.log(f"Mining: start_nonce={start_nonce}, count={nonce_count}")
        self.is_mining = True
        self.gpu_info["utilization"] = 98.5
        
        # Simulate mining time based on hash rate
        duration_ms = int((nonce_count / self.hash_rate) * 1000)
        time.sleep(duration_ms / 1000.0)
        
        # Simulate finding a solution occasionally (1 in 10 million chance)
        if random.randint(1, 10_000_000) == 1:
            # Generate a fake solution
            nonce = start_nonce + random.randint(0, nonce_count - 1)
            fake_hash = "0" * 64  # Would be real hash in actual implementation
            
            self.log(f"Found solution! nonce={nonce}")
            return {
                "type": "solution",
                "nonce": nonce,
                "hash": fake_hash
            }
        
        return {
            "type": "complete",
            "hashes_computed": nonce_count,
            "duration_ms": duration_ms
        }
    
    def handle_stop(self, msg):
        """Handle stop mining request"""
        self.log("Stopping mining")
        self.is_mining = False
        self.gpu_info["utilization"] = 0.0
        return {"type": "stopped"}
    
    def handle_info(self, msg):
        """Handle GPU info request"""
        return {
            "type": "info",
            "gpus": [self.gpu_info]
        }
    
    def handle_shutdown(self, msg):
        """Handle shutdown request"""
        self.log("Shutting down")
        sys.exit(0)
    
    def process_message(self, msg):
        """Process incoming message and return response"""
        msg_type = msg.get("type")
        
        handlers = {
            "init": self.handle_init,
            "mine": self.handle_mine,
            "stop": self.handle_stop,
            "info": self.handle_info,
            "shutdown": self.handle_shutdown
        }
        
        handler = handlers.get(msg_type)
        if handler:
            return handler(msg)
        else:
            return {
                "type": "error",
                "message": f"Unknown message type: {msg_type}"
            }
    
    def run(self):
        """Main event loop"""
        self.log("GPU Miner Stub started")
        
        while True:
            try:
                line = sys.stdin.readline()
                if not line:
                    break
                
                line = line.strip()
                if not line:
                    continue
                
                self.log(f"Received: {line}")
                
                request = json.loads(line)
                response = self.process_message(request)
                
                response_json = json.dumps(response)
                print(response_json)
                sys.stdout.flush()
                
                self.log(f"Sent: {response_json}")
                
            except json.JSONDecodeError as e:
                error = {"type": "error", "message": f"JSON decode error: {e}"}
                print(json.dumps(error))
                sys.stdout.flush()
            except Exception as e:
                error = {"type": "error", "message": str(e)}
                print(json.dumps(error))
                sys.stdout.flush()
                self.log(f"Error: {e}")

def main():
    miner = GPUMinerStub()
    miner.run()

if __name__ == "__main__":
    main()