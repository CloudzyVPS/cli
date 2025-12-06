use axum::{
    extract::{State, Request},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::CookieJar;

use crate::models::AppState;
use crate::handlers::helpers::current_username_from_jar;

pub async fn auth_middleware(
    State(state): State<AppState>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    if current_username_from_jar(&state, &jar).is_some() {
        next.run(request).await
    } else {
        Redirect::to("/login").into_response()
    }
}
