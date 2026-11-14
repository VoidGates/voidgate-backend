use std::{collections::HashSet, sync::Arc};

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

const MAX_BODY_BYTES: usize = 4096;

#[derive(Clone)]
pub struct AppState {
    api_key: Arc<str>,
    decoy_paths: Arc<HashSet<String>>,
}

impl AppState {
    pub fn new(api_key: String, decoy_paths: Vec<String>) -> Result<Self, String> {
        if api_key.len() < 32 {
            return Err("VOIDGATE_API_KEY must be at least 32 bytes".into());
        }

        let mut paths = HashSet::new();
        for path in decoy_paths {
            if !valid_path(&path) || path == "/" {
                return Err(format!("invalid decoy path: {path}"));
            }
            paths.insert(path);
        }

        Ok(Self {
            api_key: api_key.into(),
            decoy_paths: Arc::new(paths),
        })
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/live", get(health))
        .route("/ready", get(health))
        .route("/v1/decisions/http", post(decide_http))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpDecisionRequest {
    method: String,
    path: String,
}

#[derive(Serialize)]
struct HttpDecisionResponse {
    action: &'static str,
    reason_code: &'static str,
}

async fn decide_http(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<HttpDecisionRequest>,
) -> Result<Json<HttpDecisionResponse>, StatusCode> {
    let supplied_key = headers
        .get("x-voidgate-key")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");

    if !bool::from(state.api_key.as_bytes().ct_eq(supplied_key.as_bytes())) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if !valid_method(&input.method) || !valid_path(&input.path) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let (action, reason_code) = if state.decoy_paths.contains(&input.path) {
        ("decoy", "registered_decoy_path")
    } else {
        ("allow", "no_matching_rule")
    };

    Ok(Json(HttpDecisionResponse {
        action,
        reason_code,
    }))
}

fn valid_method(method: &str) -> bool {
    matches!(
        method,
        "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
    )
}

fn valid_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 2048
        && !path
            .chars()
            .any(|ch| matches!(ch, '?' | '#' | '\\') || ch.is_control())
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::{app, AppState};

    fn test_app() -> axum::Router {
        app(AppState::new("a".repeat(32), vec!["/internal/backup".into()]).unwrap())
    }

    async fn decision(path: &str, key: Option<&str>) -> (StatusCode, String) {
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/decisions/http")
            .header("content-type", "application/json");
        if let Some(key) = key {
            request = request.header("x-voidgate-key", key);
        }
        let request = request
            .body(Body::from(format!(r#"{{"method":"GET","path":"{path}"}}"#)))
            .unwrap();
        let response = test_app().oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn health_is_public() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn decision_requires_key() {
        assert_eq!(
            decision("/internal/backup", None).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn registered_path_is_decoy_and_other_paths_are_allowed() {
        let key = "a".repeat(32);
        let (status, body) = decision("/internal/backup", Some(&key)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("\"action\":\"decoy\""));

        let (status, body) = decision("/public", Some(&key)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("\"action\":\"allow\""));
    }

    #[tokio::test]
    async fn rejects_invalid_path() {
        let key = "a".repeat(32);
        assert_eq!(
            decision("relative", Some(&key)).await.0,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn rejects_oversized_body() {
        let request = Request::builder()
            .method("POST")
            .uri("/v1/decisions/http")
            .header("content-type", "application/json")
            .header("x-voidgate-key", "a".repeat(32))
            .body(Body::from("x".repeat(4097)))
            .unwrap();
        let response = test_app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[test]
    fn rejects_weak_key_and_invalid_decoy_path() {
        assert!(AppState::new("short".into(), vec![]).is_err());
        assert!(AppState::new("a".repeat(32), vec!["relative".into()]).is_err());
    }
}
