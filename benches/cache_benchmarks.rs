use criterion::{criterion_group, criterion_main, Criterion};
use dashdotcache::cache::{Cache, Config, SetOptionals, Value};
use std::hint::black_box;
use std::collections::{HashMap, HashSet};

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} bytes", bytes)
    }
}

fn create_test_cases() -> Vec<(&'static str, Value)> {
    vec![
        // Representative test cases covering all types and sizes
        ("String_Small", Value::String("hello world".to_string())),
        ("String_Large", Value::String("a".repeat(1_000))),
        ("String_XLarge", Value::String("a".repeat(10_000))),

        ("Integer", Value::Integer(12345)),
        ("Float", Value::Float(123.45)),

        ("Bytes_Small", Value::Bytes(vec![0; 100])),
        ("Bytes_Large", Value::Bytes(vec![0; 10_000])),

        ("Hash_Small", Value::Hash({
            let mut map = HashMap::new();
            map.insert("key1".to_string(), Value::String("value1".to_string()));
            map
        })),
        ("Hash_Large", Value::Hash({
            let mut map = HashMap::new();
            for i in 0..100 {
                map.insert(format!("key{}", i), Value::String(format!("value{}", i)));
            }
            map
        })),

        ("List_Small", Value::List(vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)])),
        ("List_Large", Value::List((0..100).map(|i| Value::Integer(i)).collect())),

        ("Set_Small", Value::Set({
            let mut set = HashSet::new();
            for i in 0..5 {
                set.insert(format!("member{}", i));
            }
            set
        })),
        ("Set_Large", Value::Set({
            let mut set = HashSet::new();
            for i in 0..100 {
                set.insert(format!("member{}", i));
            }
            set
        })),
    ]
}

// 1. ESSENTIAL: Individual value memory analysis
fn value_memory_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("Value Memory Analysis");
    let test_cases = create_test_cases();

    println!("\n=== VALUE MEMORY USAGE ===");
    println!("{:<20} {:<15} {:<15} {}", "Type", "Memory Usage", "Category", "Description");
    println!("{}", "-".repeat(65));

    for (name, value) in &test_cases {
        let memory = value.memory_usage();
        let type_name = value.type_name();
        let category = match type_name {
            "string" => "Text",
            "integer" | "float" => "Numeric",
            "bytes" => "Binary",
            "hash" => "Key-Value",
            "list" => "Sequential",
            "set" => "Unique",
            _ => "Other"
        };

        println!("{:<20} {:<15} {:<15} {}",
            name,
            format_bytes(memory),
            category,
            match memory {
                0..=100 => "Minimal",
                101..=1000 => "Small",
                1001..=10000 => "Medium",
                10001..=100000 => "Large",
                _ => "Very large"
            }
        );

        group.bench_function(*name, |b| {
            b.iter(|| black_box(value.memory_usage()));
        });
    }

    group.finish();
}

// 2. ESSENTIAL: Cache scaling analysis - how memory grows with item count
fn cache_scaling_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cache Scaling Analysis");

    println!("\n=== CACHE SCALING ANALYSIS ===");
    println!("{:<15} {:<10} {:<15} {:<15} {}", "Value Type", "Items", "Total Memory", "Per Item", "Overhead");
    println!("{}", "-".repeat(70));

    let key_cases = vec![
        ("String", Value::String("test_value".to_string())),
        ("Integer", Value::Integer(12345)),
        ("Hash", Value::Hash({
            let mut map = HashMap::new();
            map.insert("key".to_string(), Value::String("value".to_string()));
            map
        })),
    ];

    for (name, value) in &key_cases {
        for &num_items in &[100, 1000, 5000] {
            group.bench_function(&format!("{}_{}_items", name, num_items), |b| {
                b.iter(|| {
                    let cache = Cache::new(Config::default());

                    for i in 0..num_items {
                        let key = format!("key_{:06}", i);
                        cache.set(key, value.clone(), SetOptionals::default()).unwrap();
                    }

                    let total_memory = cache.memory_usage();
                    let single_value_memory = value.memory_usage();
                    let per_item_memory = total_memory / num_items;
                    let overhead = per_item_memory.saturating_sub(single_value_memory);

                    if num_items == 1000 {
                        println!("{:<15} {:<10} {:<15} {:<15} {}",
                            name,
                            num_items,
                            format_bytes(total_memory),
                            format_bytes(per_item_memory),
                            format_bytes(overhead)
                        );
                    }

                    black_box(total_memory)
                });
            });
        }
    }

    group.finish();
}

// 3. ESSENTIAL: Growth pattern analysis - how memory changes over time
fn memory_growth_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Growth Patterns");

    println!("\n=== MEMORY GROWTH PATTERNS ===");

    let patterns = vec![
        ("Linear_Growth", (0..500).map(|i| Value::String(format!("value_{}", i))).collect::<Vec<_>>()),
        ("Exponential_Growth", (0..10).map(|i| Value::Bytes(vec![0; 2_usize.pow(i)])).collect::<Vec<_>>()),
    ];

    for (pattern_name, values) in patterns {
        group.bench_function(pattern_name, |b| {
            b.iter(|| {
                let cache = Cache::new(Config::default());
                let mut previous_memory = 0;

                for (i, value) in values.iter().enumerate() {
                    let key = format!("key_{}", i);
                    cache.set(key, value.clone(), SetOptionals::default()).unwrap();

                    if i < 5 || i % 100 == 0 {
                        let current_memory = cache.memory_usage();
                        let growth = current_memory.saturating_sub(previous_memory);

                        if i < 5 {
                            println!("{} step {}: {} -> {} (growth: {})",
                                pattern_name, i,
                                format_bytes(previous_memory),
                                format_bytes(current_memory),
                                format_bytes(growth)
                            );
                        }

                        previous_memory = current_memory;
                    }
                }

                black_box(cache.memory_usage())
            });
        });
    }

    group.finish();
}

// 4. ESSENTIAL: Cache configuration limits
fn cache_limits_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cache Limits Analysis");

    println!("\n=== CACHE LIMITS ANALYSIS ===");
    println!("{:<15} {:<10} {:<15} {:<15}", "Config", "Max Items", "Memory Used", "Efficiency");
    println!("{}", "-".repeat(60));

    let configs = vec![
        ("Unlimited", Config::default()),
        ("MemLimit_1MB", Config { max_memory: Some(1024 * 1024), ..Default::default() }),
        ("KeyLimit_500", Config { max_keys: Some(500), ..Default::default() }),
        ("Both_Limited", Config {
            max_memory: Some(512 * 1024),
            max_keys: Some(300),
            ..Default::default()
        }),
    ];

    for (config_name, config) in configs {
        group.bench_function(config_name, |b| {
            b.iter(|| {
                let cache = Cache::new(config.clone());
                let mut successful_sets = 0;

                // Try to fill cache until we hit limits
                for i in 0..2000 {
                    let key = format!("key_{}", i);
                    let value = Value::String(format!("value_string_data_{}", i));

                    if cache.set(key, value, SetOptionals::default()).is_ok() {
                        successful_sets += 1;
                    } else {
                        break;
                    }
                }

                let memory_used = cache.memory_usage();
                let efficiency = if successful_sets > 0 {
                    memory_used as f64 / successful_sets as f64
                } else { 0.0 };

                println!("{:<15} {:<10} {:<15} {:<13.1} bytes/item",
                    config_name,
                    successful_sets,
                    format_bytes(memory_used),
                    efficiency
                );

                black_box((successful_sets, memory_used))
            });
        });
    }

    group.finish();
}

// 5. ESSENTIAL: Basic performance benchmarks
fn cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cache Performance");

    // Test basic set/get operations
    group.bench_function("set_get_operations", |b| {
        b.iter(|| {
            let cache = Cache::new(Config::default());

            // Set operations
            for i in 0..100 {
                let key = format!("key_{}", i);
                let value = Value::String(format!("value_{}", i));
                cache.set(key, value, SetOptionals::default()).unwrap();
            }

            // Get operations
            for i in 0..100 {
                let key = format!("key_{}", i);
                black_box(cache.get(&key));
            }

            black_box(cache.memory_usage())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    value_memory_analysis,
    cache_scaling_analysis,
    memory_growth_patterns,
    cache_limits_analysis,
    cache_performance
);

criterion_main!(benches);
