/// Placeholder - TODO after http & cache optimization
use crate::executor::{Command, CommandExecutor};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
pub struct RespServer {
    executor: Arc<CommandExecutor>,
}

impl RespServer {
    pub fn new(executor: Arc<CommandExecutor>) -> Self {
        Self { executor }
    }

    pub async fn run(&self, addr: &str) -> Result<(), std::io::Error> {
        // TODO: Implement full RESP protocol parsing
        let listener = TcpListener::bind(addr).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            let executor = self.executor.clone();

            tokio::spawn(async move {
                handle_connection(stream, executor).await;
            });
        }
    }
}

async fn handle_connection(stream: TcpStream, executor: Arc<CommandExecutor>) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
        // TODO: Implement RESP protocol parsing
        // parse -> Command
        // executor.execute(command);
        executor.execute(Command::Ping {
            message: (Some("TODO".to_string())),
        });
        writer.write_all(b"TODO: RESP parsing\r\n").await.ok();
        line.clear();
    }
}
