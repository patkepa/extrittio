use axum::http::HeaderMap;
use axum::http::Method;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use opentelemetry::global;
use opentelemetry::propagation::Extractor;
use std::sync::Arc;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::auth::validate_token;
use crate::domains::audit::types::NewAuditEventRecord;
use crate::error::AppError;
use crate::services::user_service;
use crate::state::AppState;
use crate::tenancy::{DEFAULT_TENANT_ID, TenantId};

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
        })
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    request
        .extensions_mut()
        .insert(RequestId(request_id.clone()));

    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %request.method(),
        uri = %request.uri(),
    );
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(request.headers()))
    });
    let _ = span.set_parent(parent_context);
    let mut response =
        crate::error::scope_request_id(request_id.clone(), next.run(request).instrument(span))
            .await;
    if let Ok(value) = request_id.parse() {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

struct HeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(axum::http::HeaderName::as_str).collect()
    }
}

pub async fn audit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let should_audit = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    if !should_audit {
        return next.run(request).await;
    }

    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let context = request
        .extensions()
        .get::<crate::auth::context::RequestContext>()
        .cloned();
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .map_or_else(crate::error::current_request_id, |id| id.0.clone());
    let (resource_type, resource_id) = audit_resource(&path);
    let response = next.run(request).await;
    let status = response.status();

    let tenant_id = context
        .as_ref()
        .map(|ctx| ctx.tenant_id().clone())
        .unwrap_or_else(|| {
            TenantId::new(DEFAULT_TENANT_ID).expect("default tenant ID must be valid")
        });
    let actor_id = context.as_ref().map(|ctx| ctx.user_id.to_string());
    let actor_type = if context.is_some() {
        "user"
    } else {
        "anonymous"
    };
    let action = format!("{}.{}", resource_type, method.as_str().to_ascii_lowercase());
    let event = NewAuditEventRecord {
        id: uuid::Uuid::new_v4().to_string(),
        actor_type: actor_type.to_string(),
        actor_id,
        action,
        resource_type,
        resource_id,
        outcome: if status.is_success() {
            "success"
        } else {
            "failure"
        }
        .to_string(),
        request_id,
        metadata: serde_json::json!({
            "method": method.as_str(),
            "path": path,
            "status": status.as_u16(),
        }),
    };
    if let Err(error) =
        crate::services::audit_service::record(state.persistence.audit.as_ref(), &tenant_id, event)
            .await
    {
        tracing::error!(%error, "failed to persist audit event");
    }

    response
}

fn audit_resource(path: &str) -> (String, Option<String>) {
    let segments = path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let resource_type = segments.get(2).copied().unwrap_or("unknown").to_string();
    let resource_id = (segments.len() > 3).then(|| segments[3..].join("/"));
    (resource_type, resource_id)
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let path = request.uri().path();
    if path == "/api/v1/auth/login"
        || path == "/api/v1/auth/logout"
        || path == "/health"
        || path == "/ready"
        || path == "/api/v1/firmware-updates/ci"
    {
        return Ok(next.run(request).await);
    }

    if !path.starts_with("/api/") {
        return Ok(next.run(request).await);
    }

    let token = bearer_token(&request).or_else(|| session_cookie(&request));
    let Some(token) = token else {
        tracing::warn!(path, "security.authentication_missing");
        return Err(AppError::Unauthorized);
    };

    let claims = validate_token(&token, &state.jwt_secret).map_err(|error| {
        tracing::warn!(path, %error, "security.authentication_rejected");
        AppError::Unauthorized
    })?;
    let ctx =
        user_service::context_from_claims(state.persistence.users.as_ref(), claims.clone()).await?;

    request.extensions_mut().insert(ctx);
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

fn bearer_token(request: &Request) -> Option<String> {
    request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .map(str::to_string)
}

fn session_cookie(request: &Request) -> Option<String> {
    request
        .headers()
        .get("cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == "extrittio_session").then(|| value.to_string())
            })
        })
}
