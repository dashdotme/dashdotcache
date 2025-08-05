use crate::cache::SetOptions;
use crate::executor::{Command, CommandExecutor, CommandResponse};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::io::Error;
use std::sync::Arc;
use std::time::Duration;

#[derive(Deserialize)]
pub struct ExpireRequest {
    pub seconds: u64,
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn format_http_response(response: CommandResponse) -> ApiResponse<String> {
    match response {
        CommandResponse::Ok => ApiResponse {
            success: true,
            data: Some("OK".to_string()),
            error: None,
        },
        CommandResponse::Value(v) => ApiResponse {
            success: true,
            data: Some(v),
            error: None,
        },
        CommandResponse::Integer(i) => ApiResponse {
            success: true,
            data: Some(i.to_string()),
            error: None,
        },
        CommandResponse::Null => ApiResponse {
            success: false,
            data: None,
            error: Some("Not found".to_string()),
        },
        CommandResponse::Error(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(e),
        },
        CommandResponse::Array(arr) => ApiResponse {
            success: true,
            data: Some(arr.join(",")), // Simple comma-separated format
            error: None,
        },
        CommandResponse::ArrayWithDepth(arr) => ApiResponse {
            success: true,
            data: None,
            // data: Some(json!(arr)),
            error: None,
        },
        CommandResponse::KeyInfo(info) => ApiResponse {
            success: true,
            data: None,
            // data: Some(json!(info)),
            error: None,
        },
    }
}

async fn get_metrics(State(executor): State<Arc<CommandExecutor>>) -> String {
    let stats = executor.cache.stats();
    stats.render()
}

async fn get_dashboard(State(executor): State<Arc<CommandExecutor>>) -> Json<ApiResponse<String>> {
    Json(ApiResponse {
        success: true,
        data: Some("TODO".to_string()),
        error: None,
    })
}

async fn get_key(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> Json<ApiResponse<String>> {
    let command = Command::Get { key };
    let response = executor.execute(command);
    Json(format_http_response(response))
}

#[derive(Deserialize)]
pub struct MultiKeyRequest {
    pub keys: Vec<String>,
}

async fn delete_key(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> Json<ApiResponse<String>> {
    let command = Command::Del { keys: vec![key] };
    let response = executor.execute(command);
    Json(format_http_response(response))
}

async fn get_ttl(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> Json<ApiResponse<String>> {
    let command = Command::Ttl { key };
    let response = executor.execute(command);
    Json(format_http_response(response))
}

#[derive(Serialize)]
pub struct KeyInfoResponse {
    pub key: String,
    pub exists: bool,
    pub ttl: i64,
    pub value: Option<String>,
}

// TODO: decide if worth using CommandResponse and adding to RESP
async fn get_key_full(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> Json<ApiResponse<KeyInfoResponse>> {
    let get_command = Command::Get { key: key.clone() };
    let value_response = executor.execute(get_command);

    let ttl_command = Command::Ttl { key: key.clone() };
    let ttl_response = executor.execute(ttl_command);

    let exists_command = Command::Exists {
        keys: vec![key.clone()],
    };
    let exists_response = executor.execute(exists_command);

    let info = KeyInfoResponse {
        key,
        exists: matches!(exists_response, CommandResponse::Integer(1)),
        ttl: match ttl_response {
            CommandResponse::Integer(ttl) => ttl,
            _ => -1,
        },
        value: match value_response {
            CommandResponse::Value(v) => Some(v),
            _ => None,
        },
    };

    Json(ApiResponse {
        success: true,
        data: Some(info),
        error: None,
    })
}

async fn set_expire(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<ExpireRequest>,
) -> Json<ApiResponse<String>> {
    let command = Command::Expire {
        key,
        seconds: req.seconds,
    };
    let response = executor.execute(command);
    Json(format_http_response(response))
}

async fn persist_key(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> Json<ApiResponse<String>> {
    let command = Command::Persist { key };
    let response = executor.execute(command);
    Json(format_http_response(response))
}

#[derive(Deserialize)]
pub struct SetParentRequest {
    pub parent: String,
}

async fn set_parent(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<SetParentRequest>,
) -> Json<ApiResponse<String>> {
    // TODO
    Json(ApiResponse {
        success: false,
        data: None,
        error: Some("SetParent command not implemented yet".to_string()),
    })
}

async fn get_children(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> Json<ApiResponse<String>> {
    // TODO
    Json(ApiResponse {
        success: false,
        data: None,
        error: Some("GetChildren command not implemented yet".to_string()),
    })
}

// Bulk operations
#[derive(Deserialize)]
pub struct ListKeysQuery {
    pub pattern: Option<String>,
    pub limit: Option<usize>,
}

async fn list_keys(
    Query(params): Query<ListKeysQuery>,
    State(executor): State<Arc<CommandExecutor>>,
) -> Json<ApiResponse<String>> {
    // TODO
    Json(ApiResponse {
        success: false,
        data: None,
        error: Some("ListKeys command not implemented yet".to_string()),
    })
}

async fn delete_multiple(
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<MultiKeyRequest>,
) -> Json<ApiResponse<String>> {
    let command = Command::Del { keys: req.keys };
    let response = executor.execute(command);
    Json(format_http_response(response))
}

// Raw/admin actions
async fn get_stats(State(executor): State<Arc<CommandExecutor>>) -> Json<ApiResponse<String>> {
    let stats = executor.cache.stats();
    Json(ApiResponse {
        success: true,
        data: Some(stats.render()),
        error: None,
    })
}

async fn flush_all(State(executor): State<Arc<CommandExecutor>>) -> Json<ApiResponse<String>> {
    // TODO
    Json(ApiResponse {
        success: false,
        data: None,
        error: Some("FlushAll command not implemented yet".to_string()),
    })
}

#[derive(Deserialize)]
pub struct PingRequest {
    pub message: Option<String>,
}

async fn ping(
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<Option<PingRequest>>,
) -> Json<ApiResponse<String>> {
    let message = req.and_then(|r| r.message);
    let command = Command::Ping { message };
    let response = executor.execute(command);
    Json(format_http_response(response))
}

async fn check_exists(
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<MultiKeyRequest>,
) -> Json<ApiResponse<String>> {
    let command = Command::Exists { keys: req.keys };
    let response = executor.execute(command);
    Json(format_http_response(response))
}

pub struct HttpApiServer {}

impl HttpApiServer {
    pub fn create_router(executor: Arc<CommandExecutor>) -> Router {
        Router::new()
            // stats, react dashboard
            .route("/metrics", get(get_metrics))
            .route("/dash", get(get_dashboard))
            // basics
            .route("/keys/{key}", get(get_key).post(set_key).delete(delete_key))
            // advanced
            .route("/keys/{key}/ttl", get(get_ttl))
            .route("/keys/{key}/info", get(get_key_full))
            .route("/keys/{key}/expire", post(set_expire))
            .route("/keys/{key}/persist", post(persist_key))
            // custom
            .route("/keys/{key}/parent", post(set_parent))
            .route("/keys/{key}/children", get(get_children))
            // bulk
            .route("/keys", get(list_keys).delete(delete_multiple))
            .route("/keys/exists", post(check_exists))
            .route("/ping", post(ping))
            // raw/admin actions
            .route("/stats", get(get_stats))
            .route("/flush", post(flush_all))
            .with_state(executor)
    }

    pub async fn run(executor: Arc<CommandExecutor>, addr: &str) -> Result<(), Error> {
        let app = Self::create_router(executor);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct SetKeyRequest {
    pub value: String,
    #[serde(default)]
    pub ttl: Option<u64>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub nx: bool,
    #[serde(default)]
    pub xx: bool,
}

async fn set_key(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<SetKeyRequest>,
) -> Json<ApiResponse<String>> {
    let options = SetOptions {
        ttl: req.ttl.map(Duration::from_secs),
        parent: req.parent,
        nx: req.nx,
        xx: req.xx,
    };

    let command = Command::Set {
        key,
        value: req.value,
        options,
    };
    let response = executor.execute(command);
    Json(format_http_response(response))
}
