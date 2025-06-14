//! HTTP connection pooling for improved performance and resource management
//!
//! This module provides a centralized HTTP client pool that reuses connections,
//! manages connection limits, and provides optimized client configurations for
//! different use cases (mining, configuration loading, etc.).

use crate::error::{Error, Result};
use parking_lot::RwLock;
use reqwest::{Client, ClientBuilder};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// HTTP client pool configuration
#[derive(Debug, Clone)]
pub struct HttpPoolConfig {
    /// Maximum number of connections per host
    pub max_connections_per_host: usize,
    /// Maximum idle connections per host
    pub max_idle_per_host: usize,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Keep-alive timeout
    pub keep_alive_timeout: Option<Duration>,
    /// User agent string
    pub user_agent: String,
    /// Whether to enable gzip compression
    pub gzip: bool,
    /// Maximum number of redirects to follow
    pub max_redirects: usize,
}

impl Default for HttpPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_host: 20,
            max_idle_per_host: 5,
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            keep_alive_timeout: Some(Duration::from_secs(60)),
            user_agent: format!("chainweb-mining-client/{}", env!("CARGO_PKG_VERSION")),
            gzip: true,
            max_redirects: 3,
        }
    }
}

/// Client type for specialized configurations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClientType {
    /// Mining operations (high-performance, frequent requests)
    Mining,
    /// Configuration loading (occasional, large responses)
    Config,
    /// General purpose HTTP client
    General,
    /// Insecure client (accepts self-signed certificates)
    Insecure,
}

/// HTTP client pool for managing multiple specialized clients
#[derive(Debug)]
pub struct HttpClientPool {
    /// Pool configuration
    config: HttpPoolConfig,
    /// Cached clients by type
    clients: RwLock<HashMap<ClientType, Arc<Client>>>,
}

impl HttpClientPool {
    /// Create a new HTTP client pool with default configuration
    pub fn new() -> Self {
        Self::with_config(HttpPoolConfig::default())
    }

    /// Create a new HTTP client pool with custom configuration
    pub fn with_config(config: HttpPoolConfig) -> Self {
        Self {
            config,
            clients: RwLock::new(HashMap::new()),
        }
    }

    /// Get a client for the specified type, creating it if necessary
    pub fn get_client(&self, client_type: ClientType) -> Result<Arc<Client>> {
        // Check if client already exists
        {
            let clients = self.clients.read();
            if let Some(client) = clients.get(&client_type) {
                return Ok(Arc::clone(client));
            }
        }

        // Create new client
        let client = self.create_client(&client_type)?;
        let client_arc = Arc::new(client);

        // Store in cache
        {
            let mut clients = self.clients.write();
            // Double-check in case another thread created it
            if let Some(existing) = clients.get(&client_type) {
                return Ok(Arc::clone(existing));
            }
            clients.insert(client_type.clone(), Arc::clone(&client_arc));
        }

        info!("Created new HTTP client for type: {:?}", client_type);
        Ok(client_arc)
    }

    /// Create a specialized client for the given type
    fn create_client(&self, client_type: &ClientType) -> Result<Client> {
        let mut builder = ClientBuilder::new()
            .user_agent(&self.config.user_agent)
            .connect_timeout(self.config.connect_timeout)
            .timeout(self.config.request_timeout)
            .redirect(reqwest::redirect::Policy::limited(self.config.max_redirects))
            .pool_max_idle_per_host(self.config.max_idle_per_host)
            .pool_idle_timeout(self.config.keep_alive_timeout);

        // Note: reqwest enables gzip compression by default in most builds
        // The 'gzip' feature is controlled at the crate level via Cargo.toml features

        // Apply type-specific configurations
        match client_type {
            ClientType::Mining => {
                // Mining clients need high performance and frequent reuse
                builder = builder
                    .pool_max_idle_per_host(self.config.max_idle_per_host * 2) // More idle connections
                    .pool_idle_timeout(Some(Duration::from_secs(300))) // Longer keep-alive
                    .tcp_keepalive(Some(Duration::from_secs(60))) // TCP keep-alive
                    .http2_keep_alive_timeout(Duration::from_secs(90))
                    .http2_keep_alive_interval(Some(Duration::from_secs(30)));
            }
            ClientType::Config => {
                // Config clients are used less frequently but may handle larger responses
                builder = builder
                    .pool_max_idle_per_host(2) // Fewer idle connections needed
                    .pool_idle_timeout(Some(Duration::from_secs(60))) // Shorter keep-alive
                    .timeout(Duration::from_secs(60)); // Longer timeout for large configs
            }
            ClientType::Insecure => {
                // Insecure clients accept self-signed certificates
                builder = builder
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true);
            }
            ClientType::General => {
                // Use default settings
            }
        }

        let client = builder
            .build()
            .map_err(|e| Error::network(format!("Failed to create HTTP client: {}", e)))?;

        debug!("Created HTTP client for type: {:?}", client_type);
        Ok(client)
    }

    /// Get statistics about the client pool
    pub fn get_stats(&self) -> HttpPoolStats {
        let clients = self.clients.read();
        HttpPoolStats {
            active_clients: clients.len(),
            client_types: clients.keys().cloned().collect(),
        }
    }

    /// Clear all cached clients (forces recreation on next use)
    pub fn clear_cache(&self) {
        let mut clients = self.clients.write();
        clients.clear();
        info!("HTTP client pool cache cleared");
    }

    /// Update pool configuration (clears cache to apply new settings)
    pub fn update_config(&mut self, config: HttpPoolConfig) {
        self.config = config;
        self.clear_cache();
        info!("HTTP client pool configuration updated");
    }
}

impl Default for HttpClientPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the HTTP client pool
#[derive(Debug, Clone)]
pub struct HttpPoolStats {
    /// Number of active client instances
    pub active_clients: usize,
    /// Types of clients currently cached
    pub client_types: Vec<ClientType>,
}

/// Global HTTP client pool instance
static HTTP_POOL: std::sync::OnceLock<HttpClientPool> = std::sync::OnceLock::new();

/// Get the global HTTP client pool instance
pub fn global_http_pool() -> &'static HttpClientPool {
    HTTP_POOL.get_or_init(|| HttpClientPool::new())
}

/// Initialize the global HTTP client pool with custom configuration
pub fn init_global_http_pool(config: HttpPoolConfig) -> Result<()> {
    let pool = HttpClientPool::with_config(config);
    HTTP_POOL.set(pool).map_err(|_| {
        Error::config("Global HTTP pool already initialized".to_string())
    })?;
    info!("Global HTTP client pool initialized");
    Ok(())
}

/// Convenience function to get a mining client
pub fn get_mining_client() -> Result<Arc<Client>> {
    global_http_pool().get_client(ClientType::Mining)
}

/// Convenience function to get a config client
pub fn get_config_client() -> Result<Arc<Client>> {
    global_http_pool().get_client(ClientType::Config)
}

/// Convenience function to get an insecure client
pub fn get_insecure_client() -> Result<Arc<Client>> {
    global_http_pool().get_client(ClientType::Insecure)
}

/// Convenience function to get a general client
pub fn get_general_client() -> Result<Arc<Client>> {
    global_http_pool().get_client(ClientType::General)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_pool_config_default() {
        let config = HttpPoolConfig::default();
        assert_eq!(config.max_connections_per_host, 20);
        assert_eq!(config.max_idle_per_host, 5);
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert!(config.user_agent.contains("chainweb-mining-client"));
        assert!(config.gzip);
    }

    #[test]
    fn test_client_type_equality() {
        assert_eq!(ClientType::Mining, ClientType::Mining);
        assert_ne!(ClientType::Mining, ClientType::Config);
    }

    #[test]
    fn test_http_pool_creation() {
        let pool = HttpClientPool::new();
        let stats = pool.get_stats();
        assert_eq!(stats.active_clients, 0);
        assert!(stats.client_types.is_empty());
    }

    #[test]
    fn test_client_creation() {
        let pool = HttpClientPool::new();
        
        // Create mining client
        let mining_client = pool.get_client(ClientType::Mining).unwrap();
        assert_eq!(pool.get_stats().active_clients, 1);
        
        // Get same client again (should reuse)
        let mining_client2 = pool.get_client(ClientType::Mining).unwrap();
        assert_eq!(pool.get_stats().active_clients, 1);
        assert!(Arc::ptr_eq(&mining_client, &mining_client2));
        
        // Create different client type
        let config_client = pool.get_client(ClientType::Config).unwrap();
        assert_eq!(pool.get_stats().active_clients, 2);
        assert!(!Arc::ptr_eq(&mining_client, &config_client));
    }

    #[test]
    fn test_client_cache_clear() {
        let pool = HttpClientPool::new();
        
        // Create some clients
        let _mining = pool.get_client(ClientType::Mining).unwrap();
        let _config = pool.get_client(ClientType::Config).unwrap();
        assert_eq!(pool.get_stats().active_clients, 2);
        
        // Clear cache
        pool.clear_cache();
        assert_eq!(pool.get_stats().active_clients, 0);
        
        // Create client again (should be new instance)
        let _new_mining = pool.get_client(ClientType::Mining).unwrap();
        assert_eq!(pool.get_stats().active_clients, 1);
    }

    #[test]
    fn test_global_pool_functions() {
        // These functions should not panic and should return valid clients
        let _mining = get_mining_client().unwrap();
        let _config = get_config_client().unwrap();
        let _general = get_general_client().unwrap();
        let _insecure = get_insecure_client().unwrap();
        
        let stats = global_http_pool().get_stats();
        assert!(stats.active_clients > 0);
    }

    #[test]
    fn test_custom_config() {
        let mut config = HttpPoolConfig::default();
        config.max_connections_per_host = 50;
        config.connect_timeout = Duration::from_secs(5);
        
        let pool = HttpClientPool::with_config(config.clone());
        assert_eq!(pool.config.max_connections_per_host, 50);
        assert_eq!(pool.config.connect_timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_concurrent_client_creation() {
        use std::sync::Arc;
        use std::thread;
        
        let pool = Arc::new(HttpClientPool::new());
        let mut handles = vec![];
        
        // Spawn multiple threads trying to create the same client type
        for _ in 0..10 {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                pool_clone.get_client(ClientType::Mining).unwrap()
            });
            handles.push(handle);
        }
        
        // Collect all clients
        let clients: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        
        // All should be the same instance
        for i in 1..clients.len() {
            assert!(Arc::ptr_eq(&clients[0], &clients[i]));
        }
        
        // Only one client should be cached
        assert_eq!(pool.get_stats().active_clients, 1);
    }
}