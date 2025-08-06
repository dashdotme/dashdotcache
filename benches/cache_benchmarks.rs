use criterion::{Criterion, criterion_group, criterion_main};
use dashdotcache::cache::{Cache, Config, SetOptions, Value};
use std::hint::black_box;

fn cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cache Performance");

    for &size in &[1_000, 10_000, 100_000] {
        group.bench_function(format!("set_{}", size), |b| {
            b.iter(|| {
                let cache = Cache::new(Config::default());
                for i in 0..size {
                    cache
                        .set(
                            format!("key_{}", i),
                            Value::String(format!("value_{}", i)),
                            SetOptions::default(),
                        )
                        .unwrap();
                }
            });
        });

        group.bench_function(format!("get_random_{}", size), |b| {
            let cache = Cache::new(Config::default());
            for i in 0..size {
                cache
                    .set(
                        format!("key_{}", i),
                        Value::String(format!("value_{}", i)),
                        SetOptions::default(),
                    )
                    .unwrap();
            }

            b.iter(|| {
                for i in (0..1000).map(|x| (x * 17) % size) {
                    black_box(cache.get(&format!("key_{}", i)));
                }
            });
        });
    }

    group.bench_function("children_scan_10k_items", |b| {
        let cache = Cache::new(Config::default());
        cache
            .set(
                "parent".to_string(),
                Value::String("p".to_string()),
                SetOptions::default(),
            )
            .unwrap();

        for i in 0..10_000 {
            cache
                .set(
                    format!("key_{}", i),
                    Value::String("value".to_string()),
                    if i % 10 == 0 {
                        SetOptions {
                            parent: Some("parent".to_string()),
                            ..Default::default()
                        }
                    } else {
                        SetOptions::default()
                    },
                )
                .unwrap();
        }

        b.iter(|| black_box(cache.children("parent").len()));
    });

    group.finish();
}

criterion_group!(benches, cache_performance);
criterion_main!(benches);
