use crate::{handlers, state::CertificateHttpState};
use axum::{
    extract::{DefaultBodyLimit, Request},
    http::{header::CACHE_CONTROL, HeaderName, HeaderValue},
    middleware::{from_fn, Next},
    response::Response,
    routing::{delete, get, post, put},
    Router,
};

pub const PUBLIC_CERTIFICATE_JSON_BODY_LIMIT: usize = 16 * 1024;

// Compile handler futures and response encoding in the owning HTTP crate.
#[inline(never)]
pub fn public_routes() -> Router<CertificateHttpState> {
    Router::new()
        .route(
            "/public/certificates/verify/manual",
            post(handlers::verify_certificate_manually),
        )
        .route(
            "/public/certificates/verify/qr",
            post(handlers::verify_certificate_by_qr),
        )
        .route(
            "/public/certificates/render-manifest",
            post(handlers::create_public_certificate_render_manifest),
        )
        .route_layer(from_fn(public_certificate_security_headers))
        .layer(DefaultBodyLimit::max(PUBLIC_CERTIFICATE_JSON_BODY_LIMIT))
}

// Compile handler futures and response encoding in the owning HTTP crate.
#[inline(never)]
pub fn protected_routes() -> Router<CertificateHttpState> {
    Router::new()
        .route(
            "/certificates/campaigns",
            get(handlers::list_certificate_campaigns).post(handlers::create_certificate_campaign),
        )
        .route(
            "/certificates/campaigns/{campaign_id}",
            get(handlers::get_certificate_campaign).put(handlers::update_certificate_campaign),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/status",
            put(handlers::change_certificate_campaign_status),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/purge-impact",
            get(handlers::get_certificate_campaign_purge_impact),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/purge",
            post(handlers::start_certificate_campaign_purge),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/purge-status",
            get(handlers::get_certificate_campaign_purge_status),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/purge/retry",
            post(handlers::retry_certificate_campaign_purge),
        )
        .route(
            "/certificates/owner-options",
            get(handlers::list_certificate_owner_options),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/templates",
            get(handlers::list_certificate_templates).post(handlers::create_certificate_template),
        )
        .route(
            "/certificates/templates/{template_id}",
            get(handlers::get_certificate_template)
                .put(handlers::update_certificate_template)
                .delete(handlers::delete_certificate_template),
        )
        .route(
            "/certificates/templates/{template_id}/background",
            put(handlers::attach_certificate_template_background),
        )
        .route(
            "/certificates/templates/{template_id}/assets",
            post(handlers::attach_certificate_template_asset),
        )
        .route(
            "/certificates/templates/{template_id}/fonts",
            get(handlers::list_certificate_school_fonts),
        )
        .route(
            "/certificates/templates/{template_id}/fonts/inspect",
            post(handlers::inspect_certificate_font_uploads),
        )
        .route(
            "/certificates/templates/{template_id}/fonts/batch",
            post(handlers::attach_certificate_font_batch),
        )
        .route(
            "/certificates/templates/{template_id}/assets/{asset_id}",
            delete(handlers::delete_certificate_template_asset),
        )
        .route(
            "/certificates/templates/{template_id}/variables",
            get(handlers::get_certificate_template_variable_catalog),
        )
        .route(
            "/certificates/templates/{template_id}/preview-manifest",
            post(handlers::create_certificate_template_preview_manifest),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/candidates",
            get(handlers::list_certificate_candidates),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/candidates/import",
            post(handlers::import_certificate_candidates),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/candidates/manual",
            post(handlers::create_manual_certificate_candidate),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/candidates/account-search",
            get(handlers::search_certificate_candidate_accounts)
                .post(handlers::create_account_certificate_candidate),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/candidates/bulk",
            post(handlers::bulk_update_certificate_candidates),
        )
        .route(
            "/certificates/candidates/{candidate_id}",
            get(handlers::get_certificate_candidate)
                .put(handlers::update_certificate_candidate)
                .delete(handlers::delete_certificate_candidate),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/issue-requests",
            get(handlers::list_certificate_campaign_issue_requests)
                .post(handlers::submit_certificate_issue_request),
        )
        .route(
            "/certificates/issue-requests",
            get(handlers::list_certificate_issue_requests),
        )
        .route(
            "/certificates/issue-requests/{request_id}",
            get(handlers::get_certificate_issue_request),
        )
        .route(
            "/certificates/issue-requests/{request_id}/withdraw",
            post(handlers::withdraw_certificate_issue_request),
        )
        .route(
            "/certificates/issue-requests/{request_id}/review",
            post(handlers::start_certificate_issue_request_review),
        )
        .route(
            "/certificates/issue-requests/{request_id}/return",
            post(handlers::return_certificate_issue_request),
        )
        .route(
            "/certificates/issue-requests/{request_id}/issue",
            post(handlers::issue_certificates),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/issued",
            get(handlers::list_issued_certificates),
        )
        .route(
            "/certificates/campaigns/{campaign_id}/render-manifests",
            post(handlers::create_issued_certificate_render_manifests),
        )
        .route(
            "/certificates/{certificate_id}",
            get(handlers::get_issued_certificate),
        )
        .route(
            "/certificates/{certificate_id}/revoke",
            post(handlers::revoke_issued_certificate),
        )
        .route(
            "/certificates/{certificate_id}/render-manifest",
            post(handlers::create_issued_certificate_render_manifest),
        )
        .route("/me/certificates", get(handlers::list_own_certificates))
        .route(
            "/me/certificates/{certificate_id}",
            get(handlers::get_own_certificate),
        )
        .route(
            "/me/certificates/{certificate_id}/render-manifest",
            post(handlers::create_own_certificate_render_manifest),
        )
}

async fn public_certificate_security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    response
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        routing::post,
        Router,
    };
    use tower::ServiceExt;

    use super::public_certificate_security_headers;

    #[tokio::test]
    async fn public_certificate_route_headers_cover_success_and_method_rejection() {
        let app = Router::new()
            .route("/certificate", post(|| async { StatusCode::OK }))
            .route_layer(axum::middleware::from_fn(
                public_certificate_security_headers,
            ));

        for method in [Method::POST, Method::GET] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method.clone())
                        .uri("/certificate")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            assert_eq!(
                response.headers().get("referrer-policy").unwrap(),
                "no-referrer"
            );
            assert_eq!(
                response.status(),
                if method == Method::POST {
                    StatusCode::OK
                } else {
                    StatusCode::METHOD_NOT_ALLOWED
                }
            );
        }
    }
}
