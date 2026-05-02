use axum::{
    extract::{Request, State},
    http::Method,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::auth::context::RequestContext;
use crate::auth::validate_token;
use crate::error::AppError;
use crate::state::AppState;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let path = request.uri().path();
    if path == "/api/v1/auth/login"
        || path == "/health"
        || path == "/ready"
        || path == "/api/v1/firmware-updates/ci"
        || (request.method() == Method::GET
            && path.starts_with("/api/v1/firmware-updates/")
            && path.ends_with("/download"))
    {
        return Ok(next.run(request).await);
    }

    if !path.starts_with("/api/") {
        return Ok(next.run(request).await);
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => return Err(AppError::Unauthorized),
    };

    let claims = validate_token(token, &state.jwt_secret).map_err(|_| AppError::Unauthorized)?;
    let ctx = RequestContext::from_claims(claims.clone());

    request.extensions_mut().insert(ctx);
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}
