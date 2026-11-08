//! `GET /health` through the same JSON helper `main` serves.

use axum::body::Body;
use axum::http::Request;
use matrix_mcp_server_r2::health::{health_flags, health_response};
use tower::ServiceExt;

#[tokio::test]
async fn get_health_reports_e2ee_off_when_matrix_init_is_skipped() {
    let (e2ee, cross_signed) = health_flags(true, true, true);
    let app = axum::Router::new().route(
        "/health",
        axum::routing::get(move || async move {
            axum::Json(health_response(
                "@bot:localhost",
                "0.1.1",
                e2ee,
                cross_signed,
                false,
            ))
        }),
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");
    assert_eq!(response.status(), 200);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("health body");
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("health json");
    assert_eq!(body["e2ee"], false);
    assert_eq!(body["crossSigned"], false);
    assert_eq!(body["botEnabled"], false);
    assert_eq!(body["user_id"], "@bot:localhost");
}
