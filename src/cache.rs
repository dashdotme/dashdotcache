use dashmap::DashMap;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Write;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::debug;

use crate::cache_errors::CacheError;

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
                let mut size = std::mem::size_of_val(h);
                size += h
                    .iter()
                    .map(|(k, v)| k.capacity() + v.memory_usage())
                    .sum::<usize>();
                size
            }
            Value::List(l) => {
                let mut size = std::mem::size_of_val(l);
                size += l.iter().map(|v| v.memory_usage()).sum::<usize>();
                size
            }
            Value::Set(s) => {
                let mut size = std::mem::size_of_val(s);
                size += s.iter().map(|v| v.capacity()).sum::<usize>();
                size
            }
        }
    }
}

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
                Some(parent_entry) => parent_entry.is_valid(cache),
                None => false,
            }
        } else {
            true
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
        let mut size = std::mem::size_of_val(self);

        size += self.value.memory_usage();

        if let Some(parent) = &self.parent {
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

impl Stats {
    /// Prints all metrics in prometheus format
    pub fn render(&self) -> String {
        let mut s = String::with_capacity(256);

        macro_rules! write_metric {
            ($buffer:expr, $name:expr, $help:expr, $type:expr, $value:expr) => {
                writeln!($buffer, "# HELP {} {}", $name, $help).unwrap();
                writeln!($buffer, "# TYPE {} {}", $name, $type).unwrap();
                writeln!($buffer, "{} {}", $name, $value).unwrap();
            };
        }

        write_metric!(
            &mut s,
            "cache_hits_total",
            "Total number of cache hits",
            "counter",
            self.hits.load(Ordering::Relaxed)
        );
        write_metric!(
            &mut s,
            "cache_misses_total",
            "Total number of cache misses",
            "counter",
            self.misses.load(Ordering::Relaxed)
        );
        write_metric!(
            &mut s,
            "cache_sets_total",
            "Total number of SET operations",
            "counter",
            self.sets.load(Ordering::Relaxed)
        );
        write_metric!(
            &mut s,
            "cache_deletes_total",
            "Total number of DELETE operations",
            "counter",
            self.deletes.load(Ordering::Relaxed)
        );
        write_metric!(
            &mut s,
            "cache_memory_usage_bytes",
            "Current estimated memory usage in bytes",
            "gauge",
            self.memory_usage.load(Ordering::Relaxed)
        );

        s
    }
}

pub struct Cache {
    data: DashMap<String, Entry>,
    config: Config,
    stats: Arc<Stats>,
    cleanup_shard_index: AtomicUsize,
    dependency_lock: RwLock<()>,
}

#[derive(Clone, Debug, Default)]
pub struct SetOptions {
    pub ttl: Option<Duration>,
    pub parent: Option<String>,
    pub nx: bool, // not exists: flag for 'set', to set only if key is new
    pub xx: bool, // exists: flag for 'set', to update only if key pre-exists
}

impl Cache {
    pub fn new(config: Config) -> Self {
        let cache = Self {
            data: DashMap::new(),
            config,
            stats: Arc::new(Stats::default()),
            cleanup_shard_index: AtomicUsize::new(0),
            dependency_lock: RwLock::new(()),
        };

        let base_memory =
            std::mem::size_of::<Cache>() + std::mem::size_of::<DashMap<String, Entry>>();
        cache
            .stats
            .memory_usage
            .store(base_memory, Ordering::Relaxed);
        cache
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

    pub fn ttl(&self, key: &str) -> i64 {
        let Some(entry) = self.data.get(key) else {
            return -2;
        };

        let Some(ttl) = &entry.ttl else {
            return -1;
        };

        ttl.remaining().map(|r| r.as_secs() as i64).unwrap_or(-2)
    }

    /// Sets an entry, with synchronous writes for parent refs to avoid cycles
    pub fn set(&self, key: String, value: Value, options: SetOptions) -> Result<bool, CacheError> {
        // Branch: parent refs require validation under a dependency_lock to avoid inserting cycles
        let _dependency_guard = if options.parent.is_some() || options.nx || options.xx {
            Some(self.dependency_lock.write().unwrap())
        } else {
            None
        };

        let exists = self.data.contains_key(&key);
        if options.nx && exists {
            return Ok(false);
        }
        if options.xx && !exists {
            return Ok(false);
        }

        if let Some(ref parent_key) = options.parent {
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
            ttl: options.ttl.map(Ttl::new),
            parent: options.parent,
            access_count: 0,
            last_accessed: Instant::now(),
            created_at: Instant::now(),
        };

        debug!("Inserted key {}", key);
        self.insert_entry(key, entry)?;
        Ok(true)
    }

    pub fn expire(&self, key: &str, seconds: u64) -> i64 {
        let _guard = self.dependency_lock.write().unwrap();

        match self.data.get_mut(key) {
            Some(mut entry) => {
                entry.ttl = Some(Ttl::new(Duration::from_secs(seconds)));
                1
            }
            None => 0,
        }
    }

    pub fn persist(&self, key: &str) -> i64 {
        let _guard = self.dependency_lock.write().unwrap();

        match self.data.get_mut(key) {
            Some(mut entry) => {
                entry.ttl = None;
                1
            }
            None => 0,
        }
    }

    pub fn del(&self, keys: &[&str]) -> usize {
        let mut deleted_count: usize = 0;
        let mut total_memory_freed = 0;

        {
            let _guard = self.dependency_lock.write().unwrap();

            for &key in keys {
                if let Some((removed_key, entry)) = self.data.remove(key) {
                    deleted_count += 1;
                    total_memory_freed += removed_key.capacity() + entry.memory_usage();
                }
            }
        }

        self.stats
            .deletes
            .fetch_add(deleted_count as u64, Ordering::Relaxed);
        self.stats
            .memory_usage
            .fetch_sub(total_memory_freed, Ordering::Relaxed);
        deleted_count
    }

    pub fn delete(&self, key: &str) -> bool {
        self.del(&[key]) == 1
    }

    pub fn exists(&self, key: &str) -> bool {
        match self.data.get(key) {
            Some(entry) => entry.is_valid(&self.data),
            None => false,
        }
    }

    pub fn exists_multi(&self, keys: &[&str]) -> usize {
        keys.iter()
            .map(|key| if self.exists(key) { 1 } else { 0 })
            .sum()
    }

    // Slow, avoid
    pub fn keys(&self, pattern: &str, limit: usize) -> Vec<String> {
        self.data
            .iter()
            .filter(|item| matches_pattern(item.key(), pattern))
            .take(limit)
            .map(|item| item.key().clone())
            .collect()
    }

    pub fn parent(&self, key: &str) -> Option<String> {
        self.data.get(key).and_then(|entry| entry.parent.clone())
    }

    pub fn set_parent(&self, key: &str, parent: String) -> Result<i64, CacheError> {
        let _guard = self.dependency_lock.write().unwrap();

        if !self.data.contains_key(&parent) {
            return Err(CacheError::ParentNotFound(parent.clone()));
        }

        if self.would_create_cycle(key, &parent) {
            return Err(CacheError::DependencyCycle(key.to_string(), parent.clone()));
        }

        match self.data.get_mut(key) {
            Some(mut entry) => {
                entry.parent = Some(parent);
                Ok(1)
            }
            None => Ok(0),
        }
    }

    pub fn children_recursive(&self, parent_key: &str, max_depth: usize) -> Vec<(String, u64)> {
        let mut result = Vec::new();
        let mut current_parents: HashSet<String> = [parent_key.to_string()].into();

        for depth in 1..=max_depth {
            if current_parents.is_empty() {
                break;
            }

            let mut next_parents = HashSet::new();

            for entry in self.data.iter() {
                if let Some(parent) = &entry.parent {
                    if current_parents.contains(parent) {
                        let child = entry.key().clone();
                        result.push((child.clone(), depth as u64));
                        next_parents.insert(child);
                    }
                }
            }

            current_parents = next_parents;
        }
        result
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

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn memory_usage(&self) -> usize {
        self.stats.memory_usage.load(Ordering::Relaxed)
    }

    /// Probabilistic cleanup: iterates over underlying shards in the DashMap,
    /// taking random samples in a round robin.
    pub fn cleanup_expired(&self) -> usize {
        const NUM_SAMPLES: usize = 20;

        let shards = self.data.shards();
        if shards.is_empty() {
            return 0;
        }

        let shard_counter = self.cleanup_shard_index.fetch_add(1, Ordering::Relaxed);
        let shard_index = shard_counter % shards.len();
        let shard = &shards[shard_index];

        let keys_to_delete: Vec<_> = unsafe {
            let shard_guard = shard.read();
            let shard_size = shard_guard.len();

            if shard_size == 0 {
                return 0;
            }

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

        self.del(
            &keys_to_delete
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        )
    }

    /// Used to check for cycles before adding a parent dependency
    /// Access under the dependency_lock, or you might allow cycles
    fn would_create_cycle(&self, key: &str, parent: &str) -> bool {
        if key == parent {
            return true;
        }

        let mut visited = HashSet::new();
        let mut current_option = Some(parent.to_string());

        while let Some(current_key) = current_option {
            if current_key == key {
                return true;
            }

            if !visited.insert(current_key.clone()) {
                return true;
            }

            current_option = self.data.get(&current_key).and_then(|e| e.parent.clone());
        }

        false
    }

    fn insert_entry(&self, key: String, entry: Entry) -> Result<(), CacheError> {
        let memory_delta = key.capacity() + entry.memory_usage();

        if let Some(max_memory) = self.config.max_memory {
            let current_memory = self.memory_usage();
            if current_memory + memory_delta > max_memory {
                return Err(CacheError::MemoryLimitExceeded);
            }
        }

        if let Some(max_keys) = self.config.max_keys {
            if self.data.len() >= max_keys && !self.data.contains_key(&key) {
                return Err(CacheError::KeyLimitExceeded);
            }
        }

        self.data.insert(key, entry);
        self.stats.sets.fetch_add(1, Ordering::Relaxed);
        self.stats
            .memory_usage
            .fetch_add(memory_delta, Ordering::Relaxed);

        Ok(())
    }
}

fn matches_pattern(key: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
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
                SetOptions::default(),
            )
            .unwrap();
        assert_eq!(cache.get("key1"), Some(Value::String("value1".to_string())));

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
                SetOptions {
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
                SetOptions::default(),
            )
            .unwrap();

        // Set child with dependency
        cache
            .set(
                "child".to_string(),
                Value::String("child_value".to_string()),
                SetOptions {
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
                SetOptions::default(),
            )
            .unwrap();
        cache
            .set(
                "b".to_string(),
                Value::String("b".to_string()),
                SetOptions::default(),
            )
            .unwrap();

        // a -> b
        cache
            .set(
                "a".to_string(),
                Value::String("a2".to_string()),
                SetOptions {
                    parent: Some("b".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        // b -> a should fail (would create cycle)
        let result = cache.set(
            "b".to_string(),
            Value::String("b2".to_string()),
            SetOptions {
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

        let result = cache.set(key, value, SetOptions::default());

        if let Err(e) = result {
            panic!(
                "Setting a small key failed unexpectedly! Error: {:?}. Expected size: {}, Max memory: {}",
                e,
                expected_size,
                config.max_memory.unwrap()
            );
        }

        let large_value = Value::String("x".repeat(200));
        let result = cache.set("large".to_string(), large_value, SetOptions::default());

        assert!(matches!(result, Err(CacheError::MemoryLimitExceeded)));
    }

    #[test]
    fn test_cleanup_expired() {
        let cache = Cache::new(Config::default());

        // Create enough entries to populate multiple shards across CPU cores
        let num_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4); // fallback to 4 if detection fails

        let entries_per_core = 5;
        let total_expired = num_cores * entries_per_core;

        // Add expired entries distributed across shards
        for i in 0..total_expired {
            cache
                .set(
                    format!("exp_{}", i),
                    Value::String(i.to_string()),
                    SetOptions {
                        ttl: Some(Duration::from_millis(1)),
                        ..Default::default()
                    },
                )
                .unwrap();
        }

        // Add some persistent entries
        for i in 0..5 {
            cache
                .set(
                    format!("keep_{}", i),
                    Value::String(format!("keep_{}", i)),
                    SetOptions::default(),
                )
                .unwrap();
        }

        let initial_count = cache.len();
        assert_eq!(initial_count, total_expired + 5);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(10));

        // Run cleanup enough times to hit all shards multiple times
        let shard_count = cache.data.shards().len();
        let cleanup_rounds = shard_count * 5;

        let mut cleaned_count = 0;
        for _ in 0..cleanup_rounds {
            cleaned_count += cache.cleanup_expired();
        }

        // Verify cleanup worked
        assert!(
            cleaned_count > 0,
            "Should have cleaned some expired entries"
        );
        assert_eq!(cache.len(), 5, "Should only have persistent entries left");

        // Verify specific entries
        for i in 0..5 {
            assert!(
                cache.get(&format!("keep_{}", i)).is_some(),
                "Persistent entry should exist"
            );
        }

        // Verify expired entries are gone (test a few samples)
        for i in 0..std::cmp::min(10, total_expired) {
            assert!(
                cache.get(&format!("exp_{}", i)).is_none(),
                "Expired entry should be gone"
            );
        }
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
                SetOptions::default(),
            )
            .unwrap();
        cache
            .set(
                "c".to_string(),
                Value::String("c".to_string()),
                SetOptions {
                    parent: Some("d".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        cache
            .set(
                "b".to_string(),
                Value::String("b".to_string()),
                SetOptions {
                    parent: Some("c".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        cache
            .set(
                "a".to_string(),
                Value::String("a".to_string()),
                SetOptions {
                    parent: Some("b".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        // Now try to create a cycle: d -> a (should fail)
        let result = cache.set(
            "d".to_string(),
            Value::String("d2".to_string()),
            SetOptions {
                parent: Some("a".to_string()),
                ..Default::default()
            },
        );
        assert!(matches!(result, Err(CacheError::DependencyCycle(..))));

        // Try intermediate cycle: c -> a (should also fail)
        let result = cache.set(
            "c".to_string(),
            Value::String("c2".to_string()),
            SetOptions {
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
