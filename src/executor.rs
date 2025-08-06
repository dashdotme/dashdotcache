use crate::cache::{Cache, SetOptions, Value};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Command {
    // Redis
    Get {
        key: String,
    },
    Set {
        key: String,
        value: String,
        options: SetOptions,
    },
    Del {
        keys: Vec<String>,
    },
    Expire {
        key: String,
        seconds: u64,
    },
    Ttl {
        key: String,
    },
    Persist {
        key: String,
    },
    Exists {
        keys: Vec<String>,
    },
    Ping {
        message: Option<String>,
    },
    ListKeys {
        pattern: String,
        limit: Option<u64>,
    },
    FlushAll {},
    // custom
    SetParent {
        key: String,
        parent: String,
    },
    GetParent {
        key: String,
    },
    GetChildren {
        parent: String,
        depth: Option<u64>,
    },
    GetInfo {
        key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    pub key: String,
    pub exists: bool,
    pub ttl: i64,
    pub value: Option<String>,
    pub parent: Option<String>,
    pub children_count: usize,
}

#[derive(Debug, Clone)]
pub enum CommandResponse {
    Ok,
    Value(String),
    Integer(i64),
    Array(Vec<String>),
    ArrayWithDepth(Vec<(String, u64)>),
    KeyInfo(KeyInfo),
    Null,
    Error(String),
}

pub struct CommandExecutor {
    pub cache: Arc<Cache>,
}

impl CommandExecutor {
    pub fn new(cache: Arc<Cache>) -> Self {
        Self { cache }
    }

    pub fn execute(&self, cmd: Command) -> CommandResponse {
        match cmd {
            Command::Get { key } => match self.cache.get(&key) {
                Some(value) => CommandResponse::Value(value.to_string()),
                None => CommandResponse::Null,
            },

            Command::Set {
                key,
                value,
                options,
            } => match self.cache.set(key, Value::String(value), options) {
                Ok(true) => CommandResponse::Ok,
                Ok(false) => CommandResponse::Null,
                Err(_) => CommandResponse::Error("SET failed".to_string()),
            },

            Command::Del { keys } => {
                let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
                let deleted = self.cache.del(&key_refs);
                CommandResponse::Integer(deleted as i64)
            }

            Command::Exists { keys } => {
                let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
                let count = self.cache.exists_multi(&key_refs);
                CommandResponse::Integer(count as i64)
            }

            Command::Ping { message } => match message {
                Some(msg) => CommandResponse::Value(msg),
                None => CommandResponse::Value("PONG".to_string()),
            },

            Command::Ttl { key } => {
                let ttl = self.cache.ttl(&key);
                CommandResponse::Integer(ttl)
            }

            Command::Expire { key, seconds } => {
                let result = self.cache.expire(&key, seconds);
                CommandResponse::Integer(result)
            }

            Command::Persist { key } => {
                let result = self.cache.persist(&key);
                CommandResponse::Integer(result)
            }

            Command::SetParent { key, parent } => match self.cache.set_parent(&key, parent) {
                Ok(i) => CommandResponse::Integer(i),
                Err(e) => CommandResponse::Error(e.to_string()),
            },

            Command::GetParent { key } => match self.cache.parent(&key) {
                Some(key) => CommandResponse::Value(key),
                None => CommandResponse::Null,
            },

            Command::GetChildren { parent, depth } => {
                let depth_usize = depth.and_then(|l| usize::try_from(l).ok()).unwrap_or(1);

                let children = self.cache.children_recursive(&parent, depth_usize);

                CommandResponse::ArrayWithDepth(children)
            }

            Command::ListKeys { pattern, limit } => {
                let limit_usize = limit
                    .and_then(|l| usize::try_from(l).ok())
                    .unwrap_or(usize::MAX);

                let keys = self.cache.keys(&pattern, limit_usize);
                CommandResponse::Array(keys)
            }

            Command::GetInfo { key } => {
                let exists = self.cache.exists(&key);
                let ttl = self.cache.ttl(&key);
                let value = self.cache.get(&key).map(|v| v.to_string());
                let parent = self.cache.parent(&key);
                let children_count = self.cache.children_recursive(&key, usize::MAX).len();

                CommandResponse::KeyInfo(KeyInfo {
                    key,
                    exists,
                    ttl,
                    value,
                    parent,
                    children_count,
                })
            }

            Command::FlushAll {} => {
                self.cache.flush_all();
                CommandResponse::Ok
            }
        }
    }
}
