use super::session_cache::SessionCache;
use super::session_crypto::{RawSessionToken, TokenHash};
use super::session_repository::{
    MaintainedSession, PresentedTokenKind, SessionMaintenanceMode, SessionValidity,
};
use chrono::{Duration, Utc};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;

fn snapshot(now: chrono::DateTime<Utc>, user_id: Uuid) -> MaintainedSession {
    MaintainedSession {
        session_id: Uuid::nil(),
        user_id,
        username: "test".into(),
        user_type: "staff".into(),
        presented_as: PresentedTokenKind::Current,
        remember_me: false,
        rotated_at: now,
        last_seen_at: now,
        idle_expires_at: now + Duration::hours(2),
        absolute_expires_at: now + Duration::hours(12),
        replacement: None,
    }
}

fn separate_auth_stripe(user: Uuid) -> TokenHash {
    use std::hash::{Hash, Hasher};
    fn stripe(key: &impl Hash) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish() % 256
    }
    let validation = stripe(&("a".to_string(), Uuid::nil(), user));
    (0..=255)
        .map(|byte| RawSessionToken::from_bytes([byte; 32]).token_hash())
        .find(|hash| stripe(&("a".to_string(), *hash.as_bytes())) != validation)
        .unwrap()
}

#[tokio::test]
async fn concurrent_authentication_fetches_once_and_tenants_remain_isolated() {
    let cache = Arc::new(SessionCache::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let now = Utc::now();
    let hash = RawSessionToken::from_bytes([11; 32]).token_hash();
    let user = Uuid::new_v4();
    let mut tasks = Vec::new();
    for _ in 0..20 {
        let cache = cache.clone();
        let calls = calls.clone();
        tasks.push(tokio::spawn(async move {
            cache
                .authenticate(
                    "a",
                    hash,
                    now,
                    SessionMaintenanceMode::RotateAndTouch,
                    |_| async {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::task::yield_now().await;
                        Ok(Some(snapshot(now, user)))
                    },
                )
                .await
                .unwrap()
                .unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    cache
        .authenticate(
            "b",
            hash,
            now,
            SessionMaintenanceMode::RotateAndTouch,
            |_| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(Some(snapshot(now, user)))
            },
        )
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn invalidation_rejects_in_flight_fill_and_cached_identity() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    let hash = RawSessionToken::from_bytes([12; 32]).token_hash();
    let result = cache
        .authenticate(
            "a",
            hash,
            now,
            SessionMaintenanceMode::RotateAndTouch,
            |_| async {
                cache.invalidate_identity_user("a", user);
                Ok(Some(snapshot(now, user)))
            },
        )
        .await;
    assert!(result.is_err());
    let result = cache
        .authenticate(
            "a",
            hash,
            now,
            SessionMaintenanceMode::RotateAndTouch,
            |_| async { Ok(None) },
        )
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn cache_never_defers_touch_rotation_or_session_expiry() {
    let now = Utc::now();
    let hash = RawSessionToken::from_bytes([13; 32]).token_hash();
    for boundary in ["touch", "rotation", "idle", "absolute"] {
        let cache = SessionCache::new();
        cache
            .authenticate(
                "a",
                hash,
                now,
                SessionMaintenanceMode::TouchOnly,
                |_| async {
                    let mut row = snapshot(now, Uuid::new_v4());
                    match boundary {
                        "touch" => {
                            row.last_seen_at = now - Duration::minutes(5) + Duration::seconds(1)
                        }
                        "rotation" => {
                            row.rotated_at = now - Duration::minutes(15) + Duration::seconds(1)
                        }
                        "idle" => row.idle_expires_at = now + Duration::seconds(1),
                        _ => row.absolute_expires_at = now + Duration::seconds(1),
                    }
                    Ok(Some(row))
                },
            )
            .await
            .unwrap();
        let result = cache
            .authenticate(
                "a",
                hash,
                now + Duration::seconds(1),
                SessionMaintenanceMode::RotateAndTouch,
                |_| async { Ok(None) },
            )
            .await
            .unwrap();
        assert!(result.is_none(), "boundary: {boundary}");
    }
}

#[tokio::test]
async fn realtime_reuses_api_validation_without_extending_idle_expiry() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    let hash = RawSessionToken::from_bytes([14; 32]).token_hash();
    cache
        .authenticate(
            "a",
            hash,
            now,
            SessionMaintenanceMode::RotateAndTouch,
            |_| async {
                let mut row = snapshot(now, user);
                row.idle_expires_at = now + Duration::seconds(45);
                Ok(Some(row))
            },
        )
        .await
        .unwrap();
    assert!(cache
        .revalidate(
            "a",
            Uuid::nil(),
            user,
            now + Duration::seconds(30),
            || async { panic!("API validation must satisfy realtime") }
        )
        .await
        .unwrap());
    assert!(!cache
        .revalidate(
            "a",
            Uuid::nil(),
            user,
            now + Duration::seconds(45),
            || async { Ok(None) }
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn realtime_connections_share_refresh_and_invalidation() {
    let cache = Arc::new(SessionCache::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let now = Utc::now();
    let user = Uuid::new_v4();
    let mut tasks = Vec::new();
    for _ in 0..20 {
        let cache = cache.clone();
        let calls = calls.clone();
        tasks.push(tokio::spawn(async move {
            assert!(cache
                .revalidate("a", Uuid::nil(), user, now, || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    Ok(Some(SessionValidity {
                        idle_expires_at: now + Duration::hours(1),
                        absolute_expires_at: now + Duration::hours(2),
                    }))
                })
                .await
                .unwrap());
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(cache
        .revalidate(
            "a",
            Uuid::nil(),
            user,
            now + Duration::seconds(299),
            || async { panic!("realtime cache should still be valid") }
        )
        .await
        .unwrap());
    assert!(!cache
        .revalidate(
            "a",
            Uuid::nil(),
            user,
            now + Duration::seconds(300),
            || async { Ok(None) }
        )
        .await
        .unwrap());
    cache.invalidate_identity_user("a", user);
    assert!(!cache
        .revalidate("a", Uuid::nil(), user, now, || async { Ok(None) })
        .await
        .unwrap());
}

#[tokio::test]
async fn previous_tokens_and_replacement_credentials_are_never_cached() {
    let now = Utc::now();
    let hash = RawSessionToken::from_bytes([15; 32]).token_hash();
    for previous in [true, false] {
        let cache = SessionCache::new();
        cache
            .authenticate(
                "a",
                hash,
                now,
                SessionMaintenanceMode::RotateAndTouch,
                |_| async {
                    let mut row = snapshot(now, Uuid::new_v4());
                    if previous {
                        row.presented_as = PresentedTokenKind::Previous;
                    } else {
                        row.replacement = Some(RawSessionToken::from_bytes([16; 32]));
                    }
                    Ok(Some(row))
                },
            )
            .await
            .unwrap();
        assert!(cache
            .authenticate(
                "a",
                hash,
                now,
                SessionMaintenanceMode::RotateAndTouch,
                |_| async { Ok(None) }
            )
            .await
            .unwrap()
            .is_none());
    }
}

#[tokio::test]
async fn unrelated_invalidation_does_not_discard_committed_rotation() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    let hash = RawSessionToken::from_bytes([17; 32]).token_hash();
    let result = cache
        .authenticate(
            "a",
            hash,
            now,
            SessionMaintenanceMode::RotateAndTouch,
            |_| async {
                cache.invalidate_identity_user("a", Uuid::new_v4());
                cache.invalidate_identity_tenant("b");
                let mut row = snapshot(now, user);
                row.replacement = Some(RawSessionToken::from_bytes([18; 32]));
                Ok(Some(row))
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert!(result.replacement.is_some());
}

#[tokio::test]
async fn permission_invalidation_also_evicts_session_identity() {
    let permissions = crate::db::permission_cache::PermissionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    let hash = RawSessionToken::from_bytes([19; 32]).token_hash();
    for tenant_wide in [false, true] {
        permissions
            .session_cache
            .authenticate(
                "a",
                hash,
                now,
                SessionMaintenanceMode::RotateAndTouch,
                |_| async { Ok(Some(snapshot(now, user))) },
            )
            .await
            .unwrap();
        if tenant_wide {
            permissions.invalidate_tenant("a");
        } else {
            permissions.invalidate_user("a", user);
        }
        assert!(permissions
            .session_cache
            .authenticate(
                "a",
                hash,
                now,
                SessionMaintenanceMode::RotateAndTouch,
                |_| async { Ok(None) }
            )
            .await
            .unwrap()
            .is_none());
        assert!(!permissions
            .session_cache
            .revalidate("a", Uuid::nil(), user, now, || async { Ok(None) })
            .await
            .unwrap());
    }
}

#[tokio::test]
async fn realtime_invalidation_during_fetch_fails_closed() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    assert!(cache
        .revalidate("a", Uuid::nil(), user, now, || async {
            cache.invalidate_identity_user("a", user);
            Ok(Some(SessionValidity {
                idle_expires_at: now + Duration::hours(1),
                absolute_expires_at: now + Duration::hours(2),
            }))
        })
        .await
        .is_err());
}

#[tokio::test]
async fn invalidation_journal_overflow_rejects_an_unverifiable_fill() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    let hash = RawSessionToken::from_bytes([20; 32]).token_hash();
    let result = cache
        .authenticate(
            "a",
            hash,
            now,
            SessionMaintenanceMode::RotateAndTouch,
            |_| async {
                for _ in 0..8193 {
                    cache.invalidate_identity_tenant("b");
                }
                Ok(Some(snapshot(now, user)))
            },
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn affected_permission_change_preserves_verified_rotation_cookie() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    let calls = AtomicUsize::new(0);
    let hash = RawSessionToken::from_bytes([21; 32]).token_hash();
    let replacement_hash = RawSessionToken::from_bytes([22; 32]).token_hash();
    let row = cache
        .authenticate(
            "a",
            hash,
            now,
            SessionMaintenanceMode::RotateAndTouch,
            |recovery_hash| {
                let first = calls.fetch_add(1, Ordering::SeqCst) == 0;
                assert_eq!(
                    recovery_hash.map(|hash| *hash.as_bytes()),
                    if first {
                        None
                    } else {
                        Some(*replacement_hash.as_bytes())
                    }
                );
                let cache = &cache;
                async move {
                    let mut row = snapshot(now, user);
                    if first {
                        cache.invalidate_identity_tenant("a");
                        row.replacement = Some(RawSessionToken::from_bytes([22; 32]));
                    } else {
                        row.username = "refreshed".into();
                    }
                    Ok(Some(row))
                }
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(row.username, "refreshed");
    assert_eq!(
        row.replacement.unwrap().token_hash().as_bytes(),
        replacement_hash.as_bytes()
    );
}

#[tokio::test]
async fn late_realtime_fill_cannot_replace_a_newer_http_touch() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    let hash = separate_auth_stripe(user);
    assert!(cache
        .revalidate("a", Uuid::nil(), user, now, || async {
            cache
                .authenticate(
                    "a",
                    hash,
                    now + Duration::seconds(1),
                    SessionMaintenanceMode::TouchOnly,
                    |_| async { Ok(Some(snapshot(now + Duration::seconds(1), user))) },
                )
                .await?;
            Ok(Some(SessionValidity {
                idle_expires_at: now + Duration::seconds(2),
                absolute_expires_at: now + Duration::hours(12),
            }))
        })
        .await
        .unwrap());
    assert!(cache
        .revalidate(
            "a",
            Uuid::nil(),
            user,
            now + Duration::seconds(3),
            || async { panic!("newer HTTP touch should still be cached") }
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn cached_idle_expiry_checks_for_a_committed_touch() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    assert!(cache
        .revalidate("a", Uuid::nil(), user, now, || async {
            Ok(Some(SessionValidity {
                idle_expires_at: now + Duration::seconds(1),
                absolute_expires_at: now + Duration::hours(12),
            }))
        })
        .await
        .unwrap());
    assert!(cache
        .revalidate(
            "a",
            Uuid::nil(),
            user,
            now + Duration::seconds(1),
            || async {
                Ok(Some(SessionValidity {
                    idle_expires_at: now + Duration::hours(2),
                    absolute_expires_at: now + Duration::hours(12),
                }))
            }
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn rotation_recovery_rejects_revoked_replaced_expired_or_racing_identity() {
    for failure in [
        "revoked",
        "previous",
        "session",
        "user",
        "idle",
        "absolute",
        "invalidation",
        "database",
    ] {
        let cache = SessionCache::new();
        let now = Utc::now();
        let user = Uuid::new_v4();
        let calls = AtomicUsize::new(0);
        let hash = RawSessionToken::from_bytes([24; 32]).token_hash();
        let result = cache
            .authenticate(
                "a",
                hash,
                now,
                SessionMaintenanceMode::RotateAndTouch,
                |recovery| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    let cache = &cache;
                    async move {
                        let mut row = snapshot(now, user);
                        if recovery.is_none() {
                            cache.invalidate_identity_user("a", user);
                            row.replacement = Some(RawSessionToken::from_bytes([25; 32]));
                        } else {
                            match failure {
                                "revoked" => return Ok(None),
                                "previous" => row.presented_as = PresentedTokenKind::Previous,
                                "session" => row.session_id = Uuid::new_v4(),
                                "user" => row.user_id = Uuid::new_v4(),
                                "idle" => row.idle_expires_at = now,
                                "absolute" => row.absolute_expires_at = now,
                                "invalidation" => cache.invalidate_identity_user("a", user),
                                _ => {
                                    return Err(crate::error::AppError::ServiceUnavailable(
                                        "database".into(),
                                    ))
                                }
                            }
                        }
                        Ok(Some(row))
                    }
                },
            )
            .await;
        assert!(result.is_err(), "{failure}");
        assert_eq!(calls.load(Ordering::SeqCst), 2, "{failure}");
        assert!(
            cache
                .authenticate(
                    "a",
                    hash,
                    now,
                    SessionMaintenanceMode::TouchOnly,
                    |_| async { Ok(None) }
                )
                .await
                .unwrap()
                .is_none(),
            "{failure}"
        );
    }
}

#[tokio::test]
async fn late_authentication_fill_cannot_replace_a_newer_realtime_read() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    let hash = separate_auth_stripe(user);
    cache
        .authenticate(
            "a",
            hash,
            now,
            SessionMaintenanceMode::TouchOnly,
            |_| async {
                assert!(
                    cache
                        .revalidate(
                            "a",
                            Uuid::nil(),
                            user,
                            now + Duration::seconds(1),
                            || async {
                                Ok(Some(SessionValidity {
                                    idle_expires_at: now + Duration::hours(2),
                                    absolute_expires_at: now + Duration::hours(12),
                                }))
                            }
                        )
                        .await?
                );
                let mut row = snapshot(now, user);
                row.idle_expires_at = now + Duration::seconds(2);
                Ok(Some(row))
            },
        )
        .await
        .unwrap();
    assert!(cache
        .revalidate(
            "a",
            Uuid::nil(),
            user,
            now + Duration::seconds(3),
            || async { panic!("late auth read cannot shorten cached idle expiry") }
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn absolute_expiry_remains_a_local_hard_stop() {
    let cache = SessionCache::new();
    let now = Utc::now();
    let user = Uuid::new_v4();
    assert!(cache
        .revalidate("a", Uuid::nil(), user, now, || async {
            Ok(Some(SessionValidity {
                idle_expires_at: now + Duration::seconds(1),
                absolute_expires_at: now + Duration::seconds(1),
            }))
        })
        .await
        .unwrap());
    assert!(!cache
        .revalidate(
            "a",
            Uuid::nil(),
            user,
            now + Duration::seconds(1),
            || async { panic!("absolute expiry cannot be extended") }
        )
        .await
        .unwrap());
}
