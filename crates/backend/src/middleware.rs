#[cfg(feature = "otlp")]
use axum::http::HeaderMap;
use axum::http::Method;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
#[cfg(feature = "otlp")]
use opentelemetry::global;
#[cfg(feature = "otlp")]
use opentelemetry::propagation::Extractor;
use std::sync::Arc;
use tracing::Instrument;
#[cfg(feature = "otlp")]
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::auth::context::{MappedUserClaims, RequestContext, map_validated_user_claims};
use crate::auth::policy::Permission;
use crate::auth::validate_token;
use crate::domains::audit::types::NewAuditEventRecord;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub(crate) fn redacted_request_uri(uri: &axum::http::Uri) -> String {
    if uri.path().starts_with("/api/v1/ota-downloads/") {
        "/api/v1/ota-downloads/[redacted]".into()
    } else {
        uri.to_string()
    }
}

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
        uri = %redacted_request_uri(request.uri()),
    );
    #[cfg(feature = "otlp")]
    {
        let parent_context = global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderExtractor(request.headers()))
        });
        let _ = span.set_parent(parent_context);
    }
    let mut response =
        crate::error::scope_request_id(request_id.clone(), next.run(request).instrument(span))
            .await;
    if let Ok(value) = request_id.parse() {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

#[cfg(feature = "otlp")]
struct HeaderExtractor<'a>(&'a HeaderMap);

#[cfg(feature = "otlp")]
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
    let path = if request.uri().path().starts_with("/api/v1/ota-downloads/") {
        redacted_request_uri(request.uri())
    } else {
        request.uri().path().to_string()
    };
    let context = request.extensions().get::<RequestContext>().cloned();
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .map_or_else(crate::error::current_request_id, |id| id.0.clone());
    let (resource_type, resource_id) = audit_resource(&path);
    let response = next.run(request).await;
    let status = response.status();

    let outcome = if status.is_success() {
        "success"
    } else {
        "failure"
    };
    let Some((tenant_id, actor_id)) = authenticated_audit_subject(context.as_ref()) else {
        tracing::info!(
            event = "audit.unauthenticated_mutation",
            %request_id,
            method = %method,
            path = %path,
            status = status.as_u16(),
            outcome,
            resource_type = %resource_type,
            resource_id = ?resource_id,
            "unauthenticated mutation was not written to tenant audit storage"
        );
        return response;
    };

    let action = format!("{}.{}", resource_type, method.as_str().to_ascii_lowercase());
    let event = NewAuditEventRecord {
        id: uuid::Uuid::new_v4().to_string(),
        actor_type: "user".to_string(),
        actor_id: Some(actor_id),
        action,
        resource_type,
        resource_id,
        outcome: outcome.to_string(),
        request_id,
        metadata: serde_json::json!({
            "method": method.as_str(),
            "path": path,
            "status": status.as_u16(),
        }),
    };
    if let Err(error) =
        crate::services::audit_service::record(state.persistence.audit.as_ref(), tenant_id, event)
            .await
    {
        tracing::error!(%error, "failed to persist audit event");
    }

    response
}

fn authenticated_audit_subject(
    context: Option<&RequestContext>,
) -> Option<(&crate::tenancy::TenantId, String)> {
    context.map(|context| (context.tenant_id(), context.user_id.to_string()))
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
        // This route validates a purpose-specific, expiring firmware grant itself.
        || path.starts_with("/api/v1/ota-downloads/")
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

    let mapped_claims = match request.extensions().get::<MappedUserClaims>().cloned() {
        Some(mapped_claims) => mapped_claims,
        None => {
            let claims = validate_token(&token, &state.jwt_secret).map_err(|error| {
                tracing::warn!(path, %error, "security.authentication_rejected");
                AppError::Unauthorized
            })?;
            map_validated_user_claims(claims).map_err(|error| {
                tracing::warn!(path, %error, "security.authentication_rejected");
                AppError::Unauthorized
            })?
        }
    };
    let claims = mapped_claims.claims().clone();
    let auth_epoch = claims
        .auth_epoch
        .as_deref()
        .filter(|value| !value.is_empty());
    let Some(auth_epoch) = auth_epoch else {
        tracing::warn!(
            path,
            user_id = claims.sub,
            "security.authentication_epoch_missing"
        );
        return Err(AppError::Unauthorized);
    };
    let user = state
        .application()
        .users()
        .resolve_session(
            mapped_claims.tenant_id(),
            claims.sub,
            claims.permission_version,
            auth_epoch,
        )
        .await?;
    let permissions = Permission::from_keys(&user.permissions);
    let ctx = RequestContext::authenticated(
        user.id,
        user.username,
        user.role,
        user.tenant_id,
        user.permissions,
        permissions,
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Claims;

    fn context(tenant_id: &str) -> RequestContext {
        RequestContext::from_claims(Claims {
            sub: 17,
            username: "operator".to_string(),
            role: "viewer".to_string(),
            tenant_id: Some(tenant_id.to_string()),
            scopes: vec!["devices:read".to_string()],
            permission_version: 1,
            auth_epoch: Some("test-auth-epoch".to_string()),
            exp: usize::MAX,
        })
        .unwrap()
    }

    #[test]
    fn unauthenticated_audit_has_no_tenant_storage_subject() {
        assert!(authenticated_audit_subject(None).is_none());
    }

    #[test]
    fn authenticated_audit_uses_the_mapped_tenant_and_actor() {
        let context = context("tenant-a");
        let (tenant, actor) = authenticated_audit_subject(Some(&context)).unwrap();

        assert_eq!(tenant.as_str(), "tenant-a");
        assert_eq!(actor, "17");
    }
}
