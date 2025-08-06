// Blake2s-256 GPU Mining Compute Shader
// This shader implements Blake2s-256 hashing for Chainweb mining

struct WorkData {
    // 286 bytes of work data, split into two arrays due to size limitations
    data_part1: array<u32, 64>, // First 256 bytes
    data_part2: array<u32, 8>,  // Remaining 32 bytes (total 288 bytes)
}

struct MiningParams {
    target: array<u32, 8>,  // 256-bit target (8 * 32 = 256)
    start_nonce: u32,
    nonce_count: u32,
    nonce_offset: u32,      // Offset in work data where nonce should be placed (278/4 = 69)
    padding: u32,
}

struct MiningResult {
    found: u32,             // 0 = not found, 1 = found
    nonce: u32,             // The winning nonce
    hash: array<u32, 8>,    // The resulting hash
}

@group(0) @binding(0)
var<storage, read> work_data: WorkData;

@group(0) @binding(1)
var<uniform> params: MiningParams;

@group(0) @binding(2)
var<storage, read_write> result: MiningResult;

// Blake2s constants
const BLAKE2S_IV: array<u32, 8> = array<u32, 8>(
    0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
    0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u
);

const BLAKE2S_SIGMA: array<array<u32, 16>, 10> = array<array<u32, 16>, 10>(
    array<u32, 16>(0u, 1u, 2u, 3u, 4u, 5u, 6u, 7u, 8u, 9u, 10u, 11u, 12u, 13u, 14u, 15u),
    array<u32, 16>(14u, 10u, 4u, 8u, 9u, 15u, 13u, 6u, 1u, 12u, 0u, 2u, 11u, 7u, 5u, 3u),
    array<u32, 16>(11u, 8u, 12u, 0u, 5u, 2u, 15u, 13u, 10u, 14u, 3u, 6u, 7u, 1u, 9u, 4u),
    array<u32, 16>(7u, 9u, 3u, 1u, 13u, 12u, 11u, 14u, 2u, 6u, 5u, 10u, 4u, 0u, 15u, 8u),
    array<u32, 16>(9u, 0u, 5u, 7u, 2u, 4u, 10u, 15u, 14u, 1u, 11u, 12u, 6u, 8u, 3u, 13u),
    array<u32, 16>(2u, 12u, 6u, 10u, 0u, 11u, 8u, 3u, 4u, 13u, 7u, 5u, 15u, 14u, 1u, 9u),
    array<u32, 16>(12u, 5u, 1u, 15u, 14u, 13u, 4u, 10u, 0u, 7u, 6u, 3u, 9u, 2u, 8u, 11u),
    array<u32, 16>(13u, 11u, 7u, 14u, 12u, 1u, 3u, 9u, 5u, 0u, 15u, 4u, 8u, 6u, 2u, 10u),
    array<u32, 16>(6u, 15u, 14u, 9u, 11u, 3u, 0u, 8u, 12u, 2u, 13u, 7u, 1u, 4u, 10u, 5u),
    array<u32, 16>(10u, 2u, 8u, 4u, 7u, 6u, 1u, 5u, 15u, 11u, 9u, 14u, 3u, 12u, 13u, 0u)
);

fn rotr(x: u32, n: u32) -> u32 {
    return (x >> n) | (x << (32u - n));
}

fn blake2s_g(v: ptr<function, array<u32, 16>>, a: u32, b: u32, c: u32, d: u32, x: u32, y: u32) {
    (*v)[a] = (*v)[a] + (*v)[b] + x;
    (*v)[d] = rotr((*v)[d] ^ (*v)[a], 16u);
    (*v)[c] = (*v)[c] + (*v)[d];
    (*v)[b] = rotr((*v)[b] ^ (*v)[c], 12u);
    (*v)[a] = (*v)[a] + (*v)[b] + y;
    (*v)[d] = rotr((*v)[d] ^ (*v)[a], 8u);
    (*v)[c] = (*v)[c] + (*v)[d];
    (*v)[b] = rotr((*v)[b] ^ (*v)[c], 7u);
}

fn blake2s_compress(h: ptr<function, array<u32, 8>>, m: ptr<function, array<u32, 16>>, t: u64, last: bool) {
    var v: array<u32, 16>;
    
    // Initialize working variables
    for (var i = 0u; i < 8u; i++) {
        v[i] = (*h)[i];
        v[i + 8u] = BLAKE2S_IV[i];
    }
    
    v[12] ^= u32(t & 0xFFFFFFFFu);
    v[13] ^= u32((t >> 32u) & 0xFFFFFFFFu);
    
    if (last) {
        v[14] ^= 0xFFFFFFFFu;
    }
    
    // Cryptographic mixing
    for (var round = 0u; round < 10u; round++) {
        let s = &BLAKE2S_SIGMA[round];
        
        blake2s_g(&v, 0u, 4u, 8u, 12u, (*m)[(*s)[0]], (*m)[(*s)[1]]);
        blake2s_g(&v, 1u, 5u, 9u, 13u, (*m)[(*s)[2]], (*m)[(*s)[3]]);
        blake2s_g(&v, 2u, 6u, 10u, 14u, (*m)[(*s)[4]], (*m)[(*s)[5]]);
        blake2s_g(&v, 3u, 7u, 11u, 15u, (*m)[(*s)[6]], (*m)[(*s)[7]]);
        blake2s_g(&v, 0u, 5u, 10u, 15u, (*m)[(*s)[8]], (*m)[(*s)[9]]);
        blake2s_g(&v, 1u, 6u, 11u, 12u, (*m)[(*s)[10]], (*m)[(*s)[11]]);
        blake2s_g(&v, 2u, 7u, 8u, 13u, (*m)[(*s)[12]], (*m)[(*s)[13]]);
        blake2s_g(&v, 3u, 4u, 9u, 14u, (*m)[(*s)[14]], (*m)[(*s)[15]]);
    }
    
    // Finalize
    for (var i = 0u; i < 8u; i++) {
        (*h)[i] ^= v[i] ^ v[i + 8u];
    }
}

fn blake2s_hash(work: array<u32, 72>, nonce: u32) -> array<u32, 8> {
    var h: array<u32, 8> = BLAKE2S_IV;
    
    // Blake2s parameter block (32 bytes)
    h[0] ^= 0x01010020u; // depth = 1, fanout = 1, digest_length = 32
    
    var m: array<u32, 16>;
    var data_offset = 0u;
    var t = 0u;
    
    // Process full 64-byte blocks
    for (var block = 0u; block < 4u; block++) {
        // Load message block
        for (var i = 0u; i < 16u; i++) {
            if (data_offset == params.nonce_offset && i < 2u) {
                // Insert nonce at the correct position
                if (i == 0u) {
                    m[i] = nonce;
                } else {
                    m[i] = nonce >> 32u;
                }
            } else {
                m[i] = work[data_offset];
            }
            data_offset++;
        }
        
        t += 64u;
        blake2s_compress(&h, &m, u64(t), false);
    }
    
    // Process final partial block (286 - 256 = 30 bytes)
    for (var i = 0u; i < 16u; i++) {
        if (i < 8u && data_offset < 72u) { // 8 * 4 = 32 bytes (covers the remaining 30)
            if (data_offset == params.nonce_offset) {
                m[i] = nonce;
            } else if (data_offset == params.nonce_offset + 1u) {
                m[i] = nonce >> 32u;
            } else {
                m[i] = work[data_offset];
            }
            data_offset++;
        } else {
            m[i] = 0u;
        }
    }
    
    t = 286u; // Total input length
    blake2s_compress(&h, &m, u64(t), true);
    
    return h;
}

fn check_target(hash: array<u32, 8>, target: array<u32, 8>) -> bool {
    // Compare hash with target (both in little-endian u32 format)
    // Start from the most significant word
    for (var i = 7u; i >= 0u; i--) {
        if (hash[i] < target[i]) {
            return true;
        } else if (hash[i] > target[i]) {
            return false;
        }
        if (i == 0u) { break; } // Prevent underflow
    }
    return false; // Equal means not less than target
}

@compute @workgroup_size(256)
fn mine(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;
    
    // Check if this thread should process
    if (thread_id >= params.nonce_count) {
        return;
    }
    
    // Calculate nonce for this thread
    let nonce = params.start_nonce + thread_id;
    
    // Copy work data to local array
    var local_work: array<u32, 72>;
    for (var i = 0u; i < 64u; i++) {
        local_work[i] = work_data.data_part1[i];
    }
    for (var i = 0u; i < 8u; i++) {
        local_work[64u + i] = work_data.data_part2[i];
    }
    
    // Compute hash
    let hash = blake2s_hash(local_work, nonce);
    
    // Check if hash meets target
    if (check_target(hash, params.target)) {
        // Use atomic compare-exchange to ensure only one thread writes result
        let old = atomicCompareExchangeWeak(&result.found, 0u, 1u);
        if (old.exchanged) {
            result.nonce = nonce;
            for (var i = 0u; i < 8u; i++) {
                result.hash[i] = hash[i];
            }
        }
    }
}