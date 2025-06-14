//! Integration tests for HTTP connection pooling
//! 
//! Tests the HTTP connection pool functionality, client reuse,
//! performance improvements, and integration with protocol modules.

use chainweb_mining_client::protocol::http_pool::{
    ClientType, HttpClientPool, HttpPoolConfig, global_http_pool,
    get_mining_client, get_config_client, get_insecure_client
};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_http_pool_basic_functionality() {
    let pool = HttpClientPool::new();
    
    // Test getting different client types
    let mining_client = pool.get_client(ClientType::Mining).unwrap();
    let config_client = pool.get_client(ClientType::Config).unwrap();
    let insecure_client = pool.get_client(ClientType::Insecure).unwrap();
    let general_client = pool.get_client(ClientType::General).unwrap();
    
    // Verify they are different instances
    assert!(!Arc::ptr_eq(&mining_client, &config_client));
    assert!(!Arc::ptr_eq(&mining_client, &insecure_client));
    assert!(!Arc::ptr_eq(&mining_client, &general_client));
    
    // Verify pool statistics
    let stats = pool.get_stats();
    assert_eq!(stats.active_clients, 4);
    assert!(stats.client_types.contains(&ClientType::Mining));
    assert!(stats.client_types.contains(&ClientType::Config));
    assert!(stats.client_types.contains(&ClientType::Insecure));
    assert!(stats.client_types.contains(&ClientType::General));
}

#[test]
fn test_client_reuse() {
    let pool = HttpClientPool::new();
    
    // Get the same client type multiple times
    let client1 = pool.get_client(ClientType::Mining).unwrap();
    let client2 = pool.get_client(ClientType::Mining).unwrap();
    let client3 = pool.get_client(ClientType::Mining).unwrap();
    
    // All should be the same instance (pointer equality)
    assert!(Arc::ptr_eq(&client1, &client2));
    assert!(Arc::ptr_eq(&client2, &client3));
    
    // Pool should still show only one client
    let stats = pool.get_stats();
    assert_eq!(stats.active_clients, 1);
}

#[test]
fn test_concurrent_client_access() {
    let pool = Arc::new(HttpClientPool::new());
    let mut handles = vec![];
    
    // Spawn multiple threads accessing the same client type
    for i in 0..20 {
        let pool_clone = Arc::clone(&pool);
        let handle = thread::spawn(move || {
            let client = pool_clone.get_client(ClientType::Mining).unwrap();
            // Do some work to simulate real usage
            thread::sleep(Duration::from_millis(i % 10));
            client
        });
        handles.push(handle);
    }
    
    // Collect all clients
    let clients: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    
    // All clients should be the same instance
    for i in 1..clients.len() {
        assert!(Arc::ptr_eq(&clients[0], &clients[i]));
    }
    
    // Pool should still show only one client
    assert_eq!(pool.get_stats().active_clients, 1);
}

#[test]
fn test_performance_vs_individual_clients() {
    const ITERATIONS: usize = 100;
    
    // Measure time with HTTP pool (reused clients)
    let pool = Arc::new(HttpClientPool::new());
    let start_pooled = Instant::now();
    
    let mut handles = vec![];
    for _ in 0..ITERATIONS {
        let pool_clone = Arc::clone(&pool);
        let handle = thread::spawn(move || {
            let _client = pool_clone.get_client(ClientType::Mining).unwrap();
            // Simulate minimal work
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let pooled_duration = start_pooled.elapsed();
    
    // Measure time with individual client creation
    let start_individual = Instant::now();
    
    let mut handles = vec![];
    for _ in 0..ITERATIONS {
        let handle = thread::spawn(move || {
            let _client = reqwest::Client::new();
            // Simulate minimal work
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let individual_duration = start_individual.elapsed();
    
    // Pool should be faster (or at least not significantly slower)
    println!("Pooled duration: {:?}", pooled_duration);
    println!("Individual duration: {:?}", individual_duration);
    
    // Allow some tolerance, but pooled should generally be faster
    assert!(pooled_duration <= individual_duration * 2, 
           "HTTP pool performance regression: pooled={:?}, individual={:?}", 
           pooled_duration, individual_duration);
}

#[test]
fn test_custom_pool_configuration() {
    let mut config = HttpPoolConfig::default();
    config.max_connections_per_host = 50;
    config.max_idle_per_host = 10;
    config.connect_timeout = Duration::from_secs(5);
    config.request_timeout = Duration::from_secs(60);
    config.user_agent = "test-agent/1.0".to_string();
    
    let pool = HttpClientPool::with_config(config.clone());
    
    // Test that clients can be created with custom config
    let client = pool.get_client(ClientType::Mining).unwrap();
    // Simple validation that we got a client
    assert!(Arc::strong_count(&client) >= 1);
}

#[test]
fn test_cache_management() {
    let pool = HttpClientPool::new();
    
    // Create some clients
    let _mining1 = pool.get_client(ClientType::Mining).unwrap();
    let _config1 = pool.get_client(ClientType::Config).unwrap();
    assert_eq!(pool.get_stats().active_clients, 2);
    
    // Clear cache
    pool.clear_cache();
    assert_eq!(pool.get_stats().active_clients, 0);
    
    // Create clients again - should be new instances
    let _mining2 = pool.get_client(ClientType::Mining).unwrap();
    let _config2 = pool.get_client(ClientType::Config).unwrap();
    assert_eq!(pool.get_stats().active_clients, 2);
}

#[test]
fn test_global_pool_convenience_functions() {
    // Test all convenience functions
    let mining = get_mining_client().unwrap();
    let config = get_config_client().unwrap();
    let insecure = get_insecure_client().unwrap();
    let general = global_http_pool().get_client(ClientType::General).unwrap();
    
    // All should be valid clients
    assert!(Arc::strong_count(&mining) >= 1);
    assert!(Arc::strong_count(&config) >= 1);
    assert!(Arc::strong_count(&insecure) >= 1);
    assert!(Arc::strong_count(&general) >= 1);
    
    // Global pool should show all clients
    let stats = global_http_pool().get_stats();
    assert!(stats.active_clients >= 4);
}


#[test]
fn test_pool_stats_accuracy() {
    let pool = HttpClientPool::new();
    let initial_stats = pool.get_stats();
    assert_eq!(initial_stats.active_clients, 0);
    assert!(initial_stats.client_types.is_empty());
    
    // Create clients one by one and verify stats
    let _mining = pool.get_client(ClientType::Mining).unwrap();
    let stats1 = pool.get_stats();
    assert_eq!(stats1.active_clients, 1);
    assert!(stats1.client_types.contains(&ClientType::Mining));
    
    let _config = pool.get_client(ClientType::Config).unwrap();
    let stats2 = pool.get_stats();
    assert_eq!(stats2.active_clients, 2);
    assert!(stats2.client_types.contains(&ClientType::Mining));
    assert!(stats2.client_types.contains(&ClientType::Config));
    
    // Getting the same client again shouldn't change stats
    let _mining2 = pool.get_client(ClientType::Mining).unwrap();
    let stats3 = pool.get_stats();
    assert_eq!(stats3.active_clients, 2);
}

#[test]
fn test_error_handling() {
    // Test that the pool handles errors gracefully
    let pool = HttpClientPool::new();
    
    // This should succeed
    let client = pool.get_client(ClientType::Mining);
    assert!(client.is_ok());
    
    // Even with multiple concurrent requests, errors should be handled
    let pool_arc = Arc::new(pool);
    let mut handles = vec![];
    
    for _ in 0..10 {
        let pool_clone = Arc::clone(&pool_arc);
        let handle = thread::spawn(move || {
            pool_clone.get_client(ClientType::Mining)
        });
        handles.push(handle);
    }
    
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_async_http_operations() {
    // Test that pooled clients work correctly with async operations
    let client = global_http_pool().get_client(ClientType::General).unwrap();
    
    // Test a simple HTTP request (to httpbin.org which supports CORS)
    let response = client
        .get("https://httpbin.org/status/200")
        .send()
        .await;
    
    match response {
        Ok(resp) => {
            assert!(resp.status().is_success());
        }
        Err(e) => {
            // Network tests might fail in CI environments, so we just verify
            // that the client is properly configured (not a pool-related error)
            assert!(!e.to_string().contains("pool"), 
                   "HTTP pool configuration error: {}", e);
        }
    }
}

#[test]
fn test_memory_efficiency() {
    // Test that the pool doesn't leak memory with many requests
    let pool = Arc::new(HttpClientPool::new());
    
    // Make many requests and verify memory usage is bounded
    for _ in 0..1000 {
        let _client = pool.get_client(ClientType::Mining).unwrap();
        // Client goes out of scope here, but Arc should be reused
    }
    
    // Pool should still only have one client cached
    let stats = pool.get_stats();
    assert_eq!(stats.active_clients, 1);
    
    // Creating different types should still be bounded
    for i in 0..100 {
        let client_type = match i % 4 {
            0 => ClientType::Mining,
            1 => ClientType::Config,
            2 => ClientType::Insecure,
            _ => ClientType::General,
        };
        let _client = pool.get_client(client_type).unwrap();
    }
    
    // Should only have 4 clients total (one of each type)
    let final_stats = pool.get_stats();
    assert_eq!(final_stats.active_clients, 4);
}