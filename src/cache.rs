use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tracing::debug;

use crate::cache_errors::CacheError;

/// Configuration for the cache
#[derive(Debug, Clone)]
pub struct Config {
    pub max_memory: Option<usize>,
    pub max_keys: Option<usize>,
    pub enable_dependencies: bool,
    pub ttl_cleanup_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_memory: None,
            max_keys: None,
            enable_dependencies: true,
            ttl_cleanup_interval: Duration::from_secs(60),
        }
    }
}

/// All possible value types that can be stored in the cache
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Bytes(Vec<u8>),
    Hash(HashMap<String, Value>),
    List(Vec<Value>),
    Set(HashSet<String>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "{}", s),
            Value::Integer(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::Bytes(b) => write!(f, "{} bytes", b.len()),
            Value::Hash(h) => write!(f, "hash with {} fields", h.len()),
            Value::List(l) => write!(f, "list with {} items", l.len()),
            Value::Set(s) => write!(f, "set with {} members", s.len()),
        }
    }
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Bytes(_) => "bytes",
            Value::Hash(_) => "hash",
            Value::List(_) => "list",
            Value::Set(_) => "set",
        }
    }

    pub fn memory_usage(&self) -> usize {
        match self {
            Value::String(s) => s.capacity(),
            Value::Integer(_) => std::mem::size_of::<i64>(),
            Value::Float(_) => std::mem::size_of::<f64>(),
            Value::Bytes(b) => b.capacity(),
            Value::Hash(h) => {
                // Start with the base size of the HashMap itself
                let mut size = std::mem::size_of_val(h);
                // Add the capacity of all keys and the usage of all values
                size += h.iter().map(|(k, v)| k.capacity() + v.memory_usage()).sum::<usize>();
                size
            },
            Value::List(l) => {
                let mut size = std::mem::size_of_val(l);
                size += l.iter().map(|v| v.memory_usage()).sum::<usize>();
                size
            },
            Value::Set(s) => {
                let mut size = std::mem::size_of_val(s);
                size += s.iter().map(|v| v.capacity()).sum::<usize>();
                size
            },
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bytes(b) => format!("<{} bytes>", b.len()),
            Value::Hash(h) => format!("<hash with {} fields>", h.len()),
            Value::List(l) => format!("<list with {} items>", l.len()),
            Value::Set(s) => format!("<set with {} members>", s.len()),
        }
    }
}

/// TTL information
#[derive(Debug, Clone)]
pub struct Ttl {
    pub expires_at: Instant,
    pub sliding: bool, // Toggle: Reset expiry on access
    pub duration: Duration,
}

impl Ttl {
    pub fn new(duration: Duration) -> Self {
        Self {
            expires_at: Instant::now() + duration,
            sliding: false,
            duration,
        }
    }

    pub fn sliding(duration: Duration) -> Self {
        Self {
            expires_at: Instant::now() + duration,
            sliding: true,
            duration,
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    pub fn reset(&mut self) {
        if self.sliding {
            self.expires_at = Instant::now() + self.duration;
        }
    }

    pub fn remaining(&self) -> Option<Duration> {
        let now = Instant::now();
        if now >= self.expires_at {
            None
        } else {
            Some(self.expires_at - now)
        }
    }
}

/// Cache Data Entry
#[derive(Debug, Clone)]
pub struct Entry {
    pub value: Value,
    pub ttl: Option<Ttl>,
    pub parent: Option<String>,
    pub access_count: u64,
    pub last_accessed: Instant,
    pub created_at: Instant,
}

impl Entry {
    pub fn new(value: Value) -> Self {
        let now = Instant::now();
        Self {
            value,
            ttl: None,
            parent: None,
            access_count: 0,
            last_accessed: now,
            created_at: now,
        }
    }

    pub fn with_ttl(value: Value, ttl: Ttl) -> Self {
        let now = Instant::now();
        Self {
            value,
            ttl: Some(ttl),
            parent: None,
            access_count: 0,
            last_accessed: now,
            created_at: now,
        }
    }

    pub fn with_parent(value: Value, parent: String) -> Self {
        let now = Instant::now();
        Self {
            value,
            ttl: None,
            parent: Some(parent),
            access_count: 0,
            last_accessed: now,
            created_at: now,
        }
    }

    pub fn is_valid(&self, cache: &DashMap<String, Entry>) -> bool {
        if let Some(ttl) = &self.ttl {
            if ttl.is_expired() {
                return false;
            }
        }

        if let Some(parent_key) = &self.parent {
            match cache.get(parent_key) {
                Some(parent_entry) => parent_entry.is_valid(cache), // <-- Recursion
                None => false, // Parent doesn't exist = this entry is invalid
            }
        } else {
            true // No parent dependency = valid
        }
    }

    pub fn mark_accessed(&mut self) {
        self.access_count += 1;
        self.last_accessed = Instant::now();
        if let Some(ttl) = &mut self.ttl {
            ttl.reset();
        }
    }

    pub fn memory_usage(&self) -> usize {
        // 1. Get the stack size of the entire Entry struct.
        // This automatically includes all fixed-size fields and the size of the `Option` wrappers.
        let mut size = std::mem::size_of_val(self);

        // 2. Add the heap-allocated size of the value itself.
        size += self.value.memory_usage();

        // 3. Add the heap-allocated size of the parent string, if it exists.
        if let Some(parent) = &self.parent {
            // `size_of_val` already accounted for the `String` struct itself (pointer, len, capacity).
            // We only need to add the size of the characters on the heap.
            size += parent.capacity();
        }

        size
    }
}

/// Cache statistics
#[derive(Debug, Default)]
pub struct Stats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub sets: AtomicU64,
    pub deletes: AtomicU64,
    pub memory_usage: AtomicUsize,
}

pub struct Cache {
    data: DashMap<String, Entry>,
    config: Config,
    stats: Arc<Stats>,
    cleanup_shard_index: AtomicUsize,
    dependency_lock: RwLock<()>
}

#[derive(Default)]
pub struct SetOptionals {
    pub ttl: Option<Duration>,
    pub parent: Option<String>,
}

impl Cache {
    pub fn new(config: Config) -> Self {
        Self {
            data: DashMap::new(),
            config,
            stats: Arc::new(Stats::default()),
            cleanup_shard_index: AtomicUsize::new(0),
            dependency_lock: RwLock::new(())
        }
    }

    /// Checks key liveness on access
    pub fn get(&self, key: &str) -> Option<Value> {
        match self.data.get_mut(key) {
            Some(mut entry) => {
                if !entry.is_valid(&self.data) {
                    self.stats.misses.fetch_add(1, Ordering::Relaxed);
                    drop(entry);
                    self.data.remove(key);
                    return None;
                }

                entry.mark_accessed();
                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                Some(entry.value.clone())
            }
            None => {
                self.stats.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    pub fn set(&self, key: String, value: Value, optionals: SetOptionals) -> Result<(), CacheError> {
        if let Some(ref parent_key) = optionals.parent {
            // stop race conditions from creating dependency cycles
            let _lock = self.dependency_lock.write().unwrap();

            if !self.config.enable_dependencies {
                return Err(CacheError::DependenciesDisabled);
            }

            if !self.data.contains_key(parent_key) {
                return Err(CacheError::ParentNotFound(parent_key.clone()));
            }

            if self.would_create_cycle(&key, parent_key) {
                return Err(CacheError::DependencyCycle(key, parent_key.clone()));
            }
        }

        let entry = Entry {
            value,
            ttl: optionals.ttl.map(Ttl::new),
            parent: optionals.parent,
            access_count: 0,
            last_accessed: Instant::now(),
            created_at: Instant::now(),
        };

        self.insert_entry(key, entry)
    }

    pub fn delete(&self, key: &str) -> bool {
        match self.data.remove(key) {
            Some((removed_key, entry)) => {
                let memory_delta = removed_key.capacity() + entry.memory_usage();

                self.stats.deletes.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .memory_usage
                    .fetch_sub(memory_delta, Ordering::Relaxed);
                debug!("Deleted key: {}", key);
                true
            }
            None => false,
        }
    }

    pub fn exists(&self, key: &str) -> bool {
        match self.data.get(key) {
            Some(entry) => entry.is_valid(&self.data),
            None => false,
        }
    }

    /// Get all keys (expensive - use sparingly)
    pub fn keys(&self, pattern: &str) -> Vec<String> {
        self.data
            .iter()
            .filter_map(|item| {
                let key = item.key();
                if item.value().is_valid(&self.data) && matches_pattern(key, pattern) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn flush_all(&self) {
        self.data.clear();
        self.stats.memory_usage.store(0, Ordering::Relaxed);
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Note: estimated for now
    pub fn memory_usage(&self) -> usize {
        self.stats.memory_usage.load(Ordering::Relaxed)
    }


    /// Probabilistic cleanup: iterates over underlying shards in the DashMap,
    /// taking random samples in a round robin.
    pub fn cleanup_expired(&self) -> usize {
        const NUM_SAMPLES: usize = 20;

        let shards = self.data.shards();
        if shards.is_empty() {
            return 0
        }

        let shard_counter = self.cleanup_shard_index.fetch_add(1, Ordering::Relaxed);
        let shard_index = shard_counter % shards.len();
        let shard = &shards[shard_index];

        let keys_to_delete: Vec<_> = unsafe {
            let shard_guard = shard.read();
            let shard_size = shard_guard.len();

            if shard_size == 0  { return 0; }

            let (skip, take) = if shard_size < NUM_SAMPLES {
                (0, shard_size)
            } else {
                let offset = shard_counter * 7 % (shard_size - NUM_SAMPLES + 1);
                (offset, NUM_SAMPLES)
            };

            shard_guard
                .iter()
                .skip(skip)
                .take(take)
                .filter_map(|bucket| {
                    let (key, value) = bucket.as_ref();
                    if value.get().ttl.as_ref()?.is_expired() {
                        Some(key.clone())
                    } else {
                        None
                    }
                })
                .collect()
        }; // lock released

        keys_to_delete
            .iter()
            .filter(|key| self.delete(key))
            .count()
   }

    /// Used to check for cycles before adding a parent dependency.
    /// Note: race condition here - must be accessed and updated atomically
    fn would_create_cycle(&self, key: &str, parent: &str) -> bool {
        if key == parent {
            return true;
        }

        let mut visited = HashSet::new();
        let mut current_option = Some(parent.to_string());

        while let Some(current_key) = current_option {
            // expected case, trying to add a cycle - disallow this action
            if current_key == key {
                return true;
            }

            // worst case: pre-existing cycle in graph
            if !visited.insert(current_key.clone()) {
                return true;
            }

            // traverse up
            current_option = self.data.get(&current_key).and_then(|e| e.parent.clone());
        }

        false
    }

    /// Internal: Insert entry with validation
    fn insert_entry(&self, key: String, entry: Entry) -> Result<(), CacheError> {
        let memory_delta = key.capacity() + entry.memory_usage();

        // Check memory limit
        if let Some(max_memory) = self.config.max_memory {
            let current_memory = self.memory_usage();
            if current_memory + memory_delta > max_memory {
                return Err(CacheError::MemoryLimitExceeded);
            }
        }

        // Check key count limit
        if let Some(max_keys) = self.config.max_keys {
            if self.data.len() >= max_keys && !self.data.contains_key(&key) {
                return Err(CacheError::KeyLimitExceeded);
            }
        }

        // Insert the entry
        self.data.insert(key, entry);
        self.stats.sets.fetch_add(1, Ordering::Relaxed);
        self.stats
            .memory_usage
            .fetch_add(memory_delta, Ordering::Relaxed);

        Ok(())
    }
}

/// Simple glob pattern matching
fn matches_pattern(key: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    // Simple implementation - just handle * at the end
    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        key.starts_with(prefix)
    } else {
        key == pattern
    }
}

#[cfg(test)]
mod tests {
    use crate::cache_errors::CacheError;

    use super::*;
    use std::time::Duration;

    #[test]
    fn test_basic_operations() {
        let cache = Cache::new(Config::default());

        // Test set and get
        cache
            .set(
                "key1".to_string(),
                Value::String("value1".to_string()),
                SetOptionals::default(),
            )
            .unwrap();
        assert_eq!(
            cache.get("key1"),
            Some(Value::String("value1".to_string()))
        );

        // Test get().is_some() for existence
        assert!(cache.get("key1").is_some());
        assert!(cache.get("nonexistent").is_none());

        // Test delete
        cache.delete("key1");
        assert!(cache.get("key1").is_none());
    }

    #[test]
    fn test_ttl_expiration() {
        let cache = Cache::new(Config::default());

        // Set with a very short TTL
        cache
            .set(
                "temp".to_string(),
                Value::String("temp_value".to_string()),
                SetOptionals {
                    ttl: Some(Duration::from_millis(1)),
                    ..Default::default()
                },
            )
            .unwrap();

        // Should exist immediately
        assert!(cache.get("temp").is_some());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(10));

        // Should be expired now
        assert!(cache.get("temp").is_none());
    }

    #[test]
    fn test_dependencies() {
        // Dependencies must be enabled in the config
        let config = Config {
            enable_dependencies: true,
            ..Default::default()
        };
        let cache = Cache::new(config);

        // Set parent
        cache
            .set(
                "parent".to_string(),
                Value::String("parent_value".to_string()),
                SetOptionals::default(),
            )
            .unwrap();

        // Set child with dependency
        cache
            .set(
                "child".to_string(),
                Value::String("child_value".to_string()),
                SetOptionals {
                    parent: Some("parent".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        // Both should exist
        assert!(cache.get("parent").is_some());
        assert!(cache.get("child").is_some());

        // Delete parent
        cache.delete("parent");

        // Child should now be invalid due to missing parent
        assert!(cache.get("child").is_none());
    }

    #[test]
    fn test_cycle_detection() {
        let config = Config {
            enable_dependencies: true,
            ..Default::default()
        };
        let cache = Cache::new(config);

        cache
            .set(
                "a".to_string(),
                Value::String("a".to_string()),
                SetOptionals::default(),
            )
            .unwrap();
        cache
            .set(
                "b".to_string(),
                Value::String("b".to_string()),
                SetOptionals::default(),
            )
            .unwrap();

        // a -> b
        cache
            .set(
                "a".to_string(),
                Value::String("a2".to_string()),
                SetOptionals {
                    parent: Some("b".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        // b -> a should fail (would create cycle)
        let result = cache.set(
            "b".to_string(),
            Value::String("b2".to_string()),
            SetOptionals {
                parent: Some("a".to_string()),
                ..Default::default()
            },
        );

        // Assert that we get the specific cycle error
        assert!(matches!(result, Err(CacheError::DependencyCycle(..))));
    }

    #[test]
    fn test_memory_limit() {
        let config = Config {
            max_memory: Some(512),
            ..Default::default()
        };
        let cache = Cache::new(config.clone());

        let key = "small".to_string();
        let value = Value::String("ok".to_string());

        let entry = Entry::new(value.clone());
        let expected_size = key.len() + entry.memory_usage();

        let result = cache.set(key, value, SetOptionals::default());

        if let Err(e) = result {
            panic!(
                "Setting a small key failed unexpectedly! Error: {:?}. Expected size: {}, Max memory: {}",
                e,
                expected_size,
                config.max_memory.unwrap()
            );
        }

        let large_value = Value::String("x".repeat(200));
        let result = cache.set("large".to_string(), large_value, SetOptionals::default());

        assert!(matches!(result, Err(CacheError::MemoryLimitExceeded)));
    }

    #[test]
    fn test_cleanup_expired() {
        let cache = Cache::new(Config::default());

        // Add some entries with very short TTL
        cache
            .set(
                "exp1".to_string(),
                Value::String("1".to_string()),
                SetOptionals {
                    ttl: Some(Duration::from_millis(1)),
                    ..Default::default()
                },
            )
            .unwrap();
        cache
            .set(
                "exp2".to_string(),
                Value::String("2".to_string()),
                SetOptionals {
                    ttl: Some(Duration::from_millis(1)),
                    ..Default::default()
                },
            )
            .unwrap();
        cache
            .set(
                "keep".to_string(),
                Value::String("keep".to_string()),
                SetOptionals::default(),
            )
            .unwrap();

        assert_eq!(cache.len(), 3);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(10));

        // Cleanup - run for each possible shard, to avoid flakiness
        for _ in 0..cache.data.shards().len() * 3 {
            cache.cleanup_expired();
        }
        assert_eq!(cache.len(), 1);
        assert!(cache.get("keep").is_some());
        assert!(cache.get("exp1").is_none());
    }

    #[test]
    fn test_complex_cycle_detection() {
        let config = Config {
            enable_dependencies: true,
            ..Default::default()
        };
        let cache = Cache::new(config);

        // Set up a complex chain: a -> b -> c -> d
        cache
            .set(
                "d".to_string(),
                Value::String("d".to_string()),
                SetOptionals::default(),
            )
            .unwrap();
        cache
            .set(
                "c".to_string(),
                Value::String("c".to_string()),
                SetOptionals {
                    parent: Some("d".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        cache
            .set(
                "b".to_string(),
                Value::String("b".to_string()),
                SetOptionals {
                    parent: Some("c".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        cache
            .set(
                "a".to_string(),
                Value::String("a".to_string()),
                SetOptionals {
                    parent: Some("b".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        // Now try to create a cycle: d -> a (should fail)
        let result = cache.set(
            "d".to_string(),
            Value::String("d2".to_string()),
            SetOptionals {
                parent: Some("a".to_string()),
                ..Default::default()
            },
        );
        assert!(matches!(result, Err(CacheError::DependencyCycle(..))));

        // Try intermediate cycle: c -> a (should also fail)
        let result = cache.set(
            "c".to_string(),
            Value::String("c2".to_string()),
            SetOptionals {
                parent: Some("a".to_string()),
                ..Default::default()
            },
        );
        assert!(matches!(result, Err(CacheError::DependencyCycle(..))));
    }

    // This test is for a helper function and does not need changes
    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("hello", "*"));
        assert!(matches_pattern("hello", "hello"));
        assert!(matches_pattern("hello_world", "hello*"));
        assert!(!matches_pattern("world_hello", "hello*"));
    }
}
