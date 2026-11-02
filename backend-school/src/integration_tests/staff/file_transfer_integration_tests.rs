use crate::{
    integration_tests::auth::session_http_tests::protected_router::ProtectedFixture,
    middleware::session::session_middleware,
};
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
    middleware::from_fn_with_state,
    Router,
};
use futures::StreamExt;
use school_auth::session_crypto::RawSessionToken;
use school_staff::file_transfer::{
    http::{routes, FileTransferHttpState},
    runtime::FileTransferRuntime,
};
use school_test_db::{create_named_test_pool, run_test_migrations};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

fn request(
    method: Method,
    path: &str,
    token: Option<&RawSessionToken>,
    csrf: Option<&str>,
    payload: Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ORIGIN, "https://tenant-a.schoolorbit.test")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        builder = builder.header(
            header::COOKIE,
            format!(
                "{}={}",
                school_auth::config::SESSION_COOKIE_NAME,
                token.encode().expose_for_cookie()
            ),
        );
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    builder.body(Body::from(payload.to_string())).unwrap()
}
async fn json_response(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap()
}
#[tokio::test]
async fn file_transfer_http_rejects_unauthenticated_nonstaff_wrong_session_and_cross_tenant_and_closes_revoked_stream(
) {
    use tower::ServiceExt;
    let pool = create_named_test_pool("file_transfer_http").await;
    run_test_migrations(&pool).await;
    let fixture = ProtectedFixture::with_prepared_tenant_pool("file_transfer_http", pool).await;
    let staff = fixture
        .insert_user(&fixture.tenant_a_pool, "transfer-staff", "active")
        .await;
    let other = fixture
        .insert_user(&fixture.tenant_a_pool, "transfer-other", "active")
        .await;
    let student = fixture
        .insert_user(&fixture.tenant_a_pool, "transfer-student", "active")
        .await;
    sqlx::query("UPDATE users SET user_type='student' WHERE id=$1")
        .bind(student)
        .execute(&fixture.tenant_a_pool)
        .await
        .unwrap();
    let (staff_session, staff_token) = fixture
        .insert_session(&fixture.tenant_a_pool, staff, 81, false)
        .await;
    let (other_session, other_token) = fixture
        .insert_session(&fixture.tenant_a_pool, other, 82, false)
        .await;
    let (student_session, student_token) = fixture
        .insert_session(&fixture.tenant_a_pool, student, 83, false)
        .await;
    let runtime = Arc::new(FileTransferRuntime::default());
    let app = Router::new()
        .nest(
            "/api",
            routes().with_state(FileTransferHttpState {
                auth_runtime: fixture.runtime.clone(),
                runtime: runtime.clone(),
            }),
        )
        .route_layer(from_fn_with_state(
            fixture.runtime.clone(),
            session_middleware,
        ));
    let path = "/api/staff/file-transfer/devices";
    for (token, csrf, status) in [
        (None, None, StatusCode::UNAUTHORIZED),
        (Some(&staff_token), None, StatusCode::FORBIDDEN),
        (
            Some(&student_token),
            Some(fixture.csrf(student_session)),
            StatusCode::FORBIDDEN,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                Method::POST,
                path,
                token,
                csrf.as_deref(),
                json!({"label":"Laptop","protocolVersion":3}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    for raw in ["{", r#"{"label":"Laptop","unexpected":"secret-sentinel"}"#] {
        let mut invalid = request(
            Method::POST,
            path,
            Some(&staff_token),
            Some(&fixture.csrf(staff_session)),
            json!({}),
        );
        *invalid.body_mut() = Body::from(raw);
        let response = app.clone().oneshot(invalid).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        let error = json_response(response).await;
        assert_eq!(error["success"], false);
        assert_eq!(error["error"], "invalid_request_body");
        assert!(!error.to_string().contains("secret-sentinel"));
    }
    for protocol in [None, Some(1), Some(2)] {
        let payload = match protocol {
            Some(version) => json!({"label":"Laptop","protocolVersion":version}),
            None => json!({"label":"Laptop"}),
        };
        let response = app
            .clone()
            .oneshot(request(
                Method::POST,
                path,
                Some(&staff_token),
                Some(&fixture.csrf(staff_session)),
                payload,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error = json_response(response).await;
        assert_eq!(error["success"], false);
        assert!(error["error"].as_str().unwrap().contains("รีเฟรช"));
    }
    let malformed_path = app
        .clone()
        .oneshot(request(
            Method::GET,
            "/api/staff/file-transfer/devices/invalid-uuid",
            Some(&staff_token),
            None,
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(malformed_path.status(), StatusCode::BAD_REQUEST);
    let error = json_response(malformed_path).await;
    assert_eq!(error["success"], false);
    assert_eq!(error["error"], "invalid_resource_id");
    let response = app
        .clone()
        .oneshot(request(
            Method::POST,
            path,
            Some(&staff_token),
            Some(&fixture.csrf(staff_session)),
            json!({"label":"Laptop","protocolVersion":3}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let snapshot = json_response(response).await;
    let device = snapshot["data"]["device"]["id"].as_str().unwrap();
    assert_eq!(snapshot["data"]["protocolVersion"], 3);
    assert_eq!(
        snapshot["data"]["limits"],
        json!({"maxOutgoingTransfers":6})
    );
    for batch in [
        json!({"fileCount":0,"totalBytes":0}),
        json!({"fileCount":9007199254740992_u64,"totalBytes":0}),
        json!({"fileCount":1,"totalBytes":9007199254740992_u64}),
        json!({"fileCount":1,"totalBytes":0,"name":"secret-sentinel"}),
        json!({"fileCount":-1,"totalBytes":0}),
        json!({"fileCount":1.5,"totalBytes":0}),
    ] {
        let response=app.clone().oneshot(request(Method::POST,"/api/staff/file-transfer/pairs",Some(&staff_token),Some(&fixture.csrf(staff_session)),json!({"deviceId":device,"target":{"type":"device","deviceId":device},"batch":batch,"clientRequestId":uuid::Uuid::new_v4()}))).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error = json_response(response).await;
        assert_eq!(error["success"], false);
        assert!(!error.to_string().contains("secret-sentinel"));
    }
    let legacy = app.clone().oneshot(request(Method::POST,"/api/staff/file-transfer/pairs",Some(&staff_token),Some(&fixture.csrf(staff_session)),json!({"deviceId":device,"target":{"type":"device","deviceId":device},"files":[{"name":"secret-sentinel","size":0,"mimeType":""}],"clientRequestId":uuid::Uuid::new_v4()}))).await.unwrap();
    assert_eq!(legacy.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_response(legacy).await["error"], "invalid_request_body");

    let own_path = format!("{path}/{device}");
    let response = app
        .clone()
        .oneshot(request(
            Method::GET,
            &own_path,
            Some(&other_token),
            None,
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let mut cross = request(Method::GET, &own_path, Some(&staff_token), None, json!({}));
    cross.headers_mut().insert(
        header::ORIGIN,
        "https://tenant-b.schoolorbit.test".parse().unwrap(),
    );
    assert_eq!(
        app.clone().oneshot(cross).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let mut stream_request = request(
        Method::GET,
        &format!("{own_path}/events?school_subdomain=tenant-a"),
        Some(&staff_token),
        None,
        json!({}),
    );
    stream_request.headers_mut().remove(header::ORIGIN);
    stream_request.headers_mut().insert(
        header::REFERER,
        "http://localhost:5173/staff/tools/file-transfer"
            .parse()
            .unwrap(),
    );
    let response = app.clone().oneshot(stream_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    let mut stream = response.into_body().into_data_stream();
    let ready = stream.next().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&ready).contains("\"type\":\"ready\""));
    // Both completed peers can close the same pair; a receipt is never authorization.
    let peer_identity = school_staff::file_transfer::runtime::FileTransferIdentity {
        tenant_id: fixture.tenant_a_id,
        user_id: other,
        session_id: other_session,
    };
    let peer = runtime
        .register(
            peer_identity,
            "Other".into(),
            school_staff::file_transfer::models::RegisterFileTransferDevice {
                label: "Peer".into(),
                protocol_version: Some(3),
            },
            chrono::Utc::now(),
        )
        .unwrap();
    let _peer_stream = runtime
        .subscribe(peer_identity, peer.device.id, chrono::Utc::now())
        .unwrap();
    let pair_response = app.clone().oneshot(request(Method::POST,"/api/staff/file-transfer/pairs",Some(&staff_token),Some(&fixture.csrf(staff_session)),json!({"deviceId":device,"target":{"type":"device","deviceId":peer.device.id},"batch":{"fileCount":50000,"totalBytes":1000000000000_u64},"clientRequestId":uuid::Uuid::new_v4()}))).await.unwrap();
    assert_eq!(pair_response.status(), StatusCode::OK);
    let pair = json_response(pair_response).await;
    assert!(pair["data"].get("files").is_none());
    assert_eq!(
        pair["data"]["batch"],
        json!({"fileCount":50000,"totalBytes":1000000000000_u64})
    );
    let pair_id = pair["data"]["id"].as_str().unwrap();
    let close_path = format!("/api/staff/file-transfer/pairs/{pair_id}/close");
    for (token, session_id, member) in [
        (&staff_token, staff_session, device.to_owned()),
        (&other_token, other_session, peer.device.id.to_string()),
        (&staff_token, staff_session, device.to_owned()),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                Method::POST,
                &close_path,
                Some(token),
                Some(&fixture.csrf(session_id)),
                json!({"deviceId":member}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_response(response).await["success"], true);
    }
    for (close_id, member) in [
        (pair_id.to_owned(), device.to_owned()),
        (uuid::Uuid::new_v4().to_string(), peer.device.id.to_string()),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/staff/file-transfer/pairs/{close_id}/close"),
                Some(&other_token),
                Some(&fixture.csrf(other_session)),
                json!({"deviceId":member}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let error = json_response(response).await;
        assert_eq!(error["success"], false);
        assert!(error["error"].as_str().unwrap().contains("กรุณา"));
    }
    fixture
        .runtime
        .session_events
        .send(school_auth::events::SessionRevocationEvent::session(
            "tenant-a",
            staff,
            staff_session,
        ))
        .unwrap();
    let invalid = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&invalid).contains("session_invalid"));
    assert!(runtime
        .snapshot(
            school_staff::file_transfer::runtime::FileTransferIdentity {
                tenant_id: fixture.tenant_a_id,
                user_id: staff,
                session_id: staff_session
            },
            device.parse().unwrap(),
            chrono::Utc::now()
        )
        .is_err());
    assert!(stream.next().await.is_none());
    let response = app
        .oneshot(request(
            Method::POST,
            path,
            Some(&other_token),
            Some(&fixture.csrf(other_session)),
            json!({"label":"x".repeat(70000),"protocolVersion":3}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    let error = json_response(response).await;
    assert_eq!(error["success"], false);
    assert!(error["error"].is_string());
    assert!(!error.to_string().contains(&"x".repeat(100)));
}
