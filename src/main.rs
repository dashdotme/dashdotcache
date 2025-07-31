use dashdotcache::cache::{Cache, Config, Value, SetOptionals};

fn main() {
    println!("Dashdotcache!");

    let cache = Cache::new(Config::default());
    cache.set("hello".to_string(), Value::String("world".to_string()), SetOptionals::default()).unwrap();

    if let Some(value) = cache.get("hello") {
        println!("Cache working! Got: {}", value.to_string());
    }
}
