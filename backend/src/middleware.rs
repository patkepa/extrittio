use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::auth::validate_token;
use crate::error::AppError;
use crate::services::user_service;
use crate::state::{AppState, run_db};

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
        || path == "/api/v1/system/version"
        || path == "/api/v1/firmware-updates/ci"
    {
        return Ok(next.run(request).await);
    }

    if !path.starts_with("/api/") {
        return Ok(next.run(request).await);
    }

    let token = bearer_token(&request).or_else(|| session_cookie(&request));
    let Some(token) = token else {
        return Err(AppError::Unauthorized);
    };

    let claims = validate_token(&token, &state.jwt_secret).map_err(|_| AppError::Unauthorized)?;
    let claims_for_context = claims.clone();
    let ctx = run_db(&state.db_pool, move |conn| {
        user_service::context_from_claims(conn, claims_for_context)
    })
    .await?;

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
