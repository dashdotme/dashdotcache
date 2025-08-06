use crate::cache::SetOptions;
use crate::executor::{Command, CommandExecutor, CommandResponse, KeyInfo};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::Deserialize;
use std::io::Error;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    InternalError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, message).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Deserialize)]
pub struct ExpireRequest {
    pub seconds: u64,
}

#[derive(Deserialize)]
pub struct MultiKeyRequest {
    pub keys: Vec<String>,
}

#[derive(Deserialize)]
pub struct SetParentRequest {
    pub parent: String,
}

#[derive(Deserialize)]
pub struct GetChildrenRequest {
    #[serde(default)]
    pub depth: Option<u64>,
}

#[derive(Deserialize)]
pub struct ListKeysQuery {
    pub pattern: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Deserialize)]
pub struct PingRequest {
    pub message: Option<String>,
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

async fn get_metrics(State(executor): State<Arc<CommandExecutor>>) -> String {
    let stats = executor.cache.stats();
    stats.render()
}

async fn get_dashboard(State(_executor): State<Arc<CommandExecutor>>) -> &'static str {
    "TODO: React dashboard"
}

async fn get_key(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> ApiResult<Json<String>> {
    let command = Command::Get { key };
    let response = executor.execute(command);
    match response {
        CommandResponse::Value(v) => Ok(Json(v)),
        CommandResponse::Null => Err(ApiError::NotFound("Key not found".to_string())),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn set_key(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<SetKeyRequest>,
) -> ApiResult<String> {
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
    match response {
        CommandResponse::Ok => Ok("OK".to_string()),
        CommandResponse::Null => Ok("Key unchanged".to_string()),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn delete_key(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> ApiResult<String> {
    let command = Command::Del { keys: vec![key] };
    let response = executor.execute(command);
    match response {
        CommandResponse::Integer(0) => Err(ApiError::NotFound("Key not found".to_string())),
        CommandResponse::Integer(count) => Ok(format!("Deleted {} key(s)", count)),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn get_ttl(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> ApiResult<Json<i64>> {
    let command = Command::Ttl { key };
    let response = executor.execute(command);
    match response {
        CommandResponse::Integer(-2) => Err(ApiError::NotFound("Key not found".to_string())),
        CommandResponse::Integer(ttl) => Ok(Json(ttl)),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn get_key_info(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> ApiResult<Json<KeyInfo>> {
    let command = Command::GetInfo { key };
    let response = executor.execute(command);
    match response {
        CommandResponse::KeyInfo(info) => Ok(Json(info)),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn set_expire(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<ExpireRequest>,
) -> ApiResult<String> {
    let command = Command::Expire {
        key,
        seconds: req.seconds,
    };
    let response = executor.execute(command);
    match response {
        CommandResponse::Integer(1) => Ok("Expiry set".to_string()),
        CommandResponse::Integer(0) => Err(ApiError::NotFound("Key not found".to_string())),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn persist_key(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
) -> ApiResult<String> {
    let command = Command::Persist { key };
    let response = executor.execute(command);
    match response {
        CommandResponse::Integer(1) => Ok("Key persisted".to_string()),
        CommandResponse::Integer(0) => Err(ApiError::NotFound("Key not found".to_string())),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn set_parent(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<SetParentRequest>,
) -> ApiResult<String> {
    let command = Command::SetParent {
        key,
        parent: req.parent,
    };
    let response = executor.execute(command);
    match response {
        CommandResponse::Integer(1) => Ok("Parent set".to_string()),
        CommandResponse::Integer(0) => Err(ApiError::NotFound("Key not found".to_string())),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn get_children(
    Path(key): Path<String>,
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<GetChildrenRequest>,
) -> ApiResult<Json<Vec<String>>> {
    let command = Command::GetChildren {
        parent: key,
        depth: req.depth,
    };
    let response = executor.execute(command);
    match response {
        CommandResponse::ArrayWithDepth(children) => {
            let child_keys: Vec<String> = children.into_iter().map(|(key, _)| key).collect();
            Ok(Json(child_keys))
        }

        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn list_keys(
    Query(params): Query<ListKeysQuery>,
    State(executor): State<Arc<CommandExecutor>>,
) -> ApiResult<Json<Vec<String>>> {
    let pattern = params.pattern.unwrap_or_else(|| "*".to_string());
    let command = Command::ListKeys {
        pattern,
        limit: params.limit,
    };
    let response = executor.execute(command);
    match response {
        CommandResponse::Array(keys) => Ok(Json(keys)),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn delete_multiple(
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<MultiKeyRequest>,
) -> ApiResult<String> {
    let command = Command::Del { keys: req.keys };
    let response = executor.execute(command);
    match response {
        CommandResponse::Integer(count) => Ok(format!("Deleted {} key(s)", count)),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn flush_all(State(executor): State<Arc<CommandExecutor>>) -> ApiResult<String> {
    let command = Command::FlushAll {};
    let response = executor.execute(command);
    match response {
        CommandResponse::Ok => Ok("All keys flushed".to_string()),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn ping(
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<Option<PingRequest>>,
) -> ApiResult<String> {
    let message = req.and_then(|r| r.message);
    let command = Command::Ping { message };
    let response = executor.execute(command);
    match response {
        CommandResponse::Value(msg) => Ok(msg),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

async fn check_exists(
    State(executor): State<Arc<CommandExecutor>>,
    Json(req): Json<MultiKeyRequest>,
) -> ApiResult<Json<i64>> {
    let command = Command::Exists { keys: req.keys };
    let response = executor.execute(command);
    match response {
        CommandResponse::Integer(count) => Ok(Json(count)),
        CommandResponse::Error(e) => Err(ApiError::BadRequest(e)),
        _ => Err(ApiError::InternalError("Unexpected response".to_string())),
    }
}

pub struct HttpApiServer {}

impl HttpApiServer {
    pub fn create_router(executor: Arc<CommandExecutor>) -> Router {
        Router::new()
            // Raw endpoints
            .route("/metrics", get(get_metrics))
            .route("/dash", get(get_dashboard))
            // Core operations
            .route("/keys/{key}", get(get_key).post(set_key).delete(delete_key))
            // Key operations
            .route("/keys/{key}/ttl", get(get_ttl))
            .route("/keys/{key}/info", get(get_key_info))
            .route("/keys/{key}/expire", post(set_expire))
            .route("/keys/{key}/persist", post(persist_key))
            // Relationship operations
            .route("/keys/{key}/parent", post(set_parent))
            .route("/keys/{key}/children", get(get_children))
            // Bulk operations
            .route("/keys", get(list_keys).delete(delete_multiple))
            .route("/keys/exists", post(check_exists))
            // Admin operations
            .route("/ping", post(ping))
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
