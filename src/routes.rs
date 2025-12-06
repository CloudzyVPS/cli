use axum::{
    routing::{get, post},
    Router,
};
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::header::CACHE_CONTROL;
use axum::http::HeaderValue;

use crate::models::AppState;
use crate::handlers;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::root_get))
        .route("/login", get(handlers::login_get).post(handlers::login_post))
        .route("/logout", post(handlers::logout_post))
        .route("/users", get(handlers::users_list).post(handlers::users_create))
        .route("/users/:username/reset-password", post(handlers::reset_password))
        .route("/users/:username/role", post(handlers::update_role))
        .route("/users/:username/delete", post(handlers::delete_user))
        .route("/regions", get(handlers::regions_get))
        .route("/products", get(handlers::products_get))
        .route("/os", get(handlers::os_get))
        .route("/applications", get(handlers::applications_get))
        // Note: remaining routes (access, ssh-keys, instances, wizard, instance actions) 
        // will be added as we refactor those handlers
        // Serve static files with cache-control header
        .nest_service(
            "/static",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(ServeDir::new("static")),
        )
        .with_state(state)
}
