use dashdotcache::cache::{Cache, Config};
use dashdotcache::executor::CommandExecutor;
use dashdotcache::http_api::HttpApiServer;
use dashdotcache::resp_api::RespServer;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Dashdotcache!");

    let cache = Arc::new(Cache::new(Config::default()));
    let executor = Arc::new(CommandExecutor::new(cache));

    println!(
        "Cache initialized. Memory usage: {}",
        executor.cache.memory_usage()
    );

    let http_executor = executor.clone();
    let resp_executor = executor.clone();

    let http_server = tokio::spawn(async move {
        println!("Starting HTTP API server on http://127.0.0.1:8080");
        HttpApiServer::run(http_executor, "127.0.0.1:8080").await
    });

    let resp_server = tokio::spawn(async move {
        println!("Starting RESP server on 127.0.0.1:6379");
        let server = RespServer::new(resp_executor);
        server.run("127.0.0.1:6379").await
    });

    tokio::select! {
        result = http_server => {
            match result {
                Ok(Ok(())) => println!("HTTP server exited successfully"),
                Ok(Err(e)) => eprintln!("HTTP server error: {}", e),
                Err(e) => eprintln!("HTTP server task error: {}", e),
            }
        }
        result = resp_server => {
            match result {
                Ok(Ok(())) => println!("RESP server exited successfully"),
                Ok(Err(e)) => eprintln!("RESP server error: {}", e),
                Err(e) => eprintln!("RESP server task error: {}", e),
            }
        }
    }

    Ok(())
}
