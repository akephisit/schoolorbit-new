use super::*;

fn owner(tenant: Uuid) -> FileTransferIdentity {
    FileTransferIdentity {
        tenant_id: tenant,
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    }
}
fn file() -> FileTransferFile {
    FileTransferFile {
        name: "เอกสาร.pdf".into(),
        size: 0,
        mime_type: "application/pdf".into(),
    }
}
const SDP: &str = "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=ice-ufrag:abcd\r\na=ice-pwd:abcdefghijklmnopqrstuv\r\na=fingerprint:sha-256 AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA:AA\r\na=setup:actpass\r\na=sctp-port:5000\r\na=candidate:1 1 udp 2122260223 device.local 55555 typ host generation 0\r\n";
fn online(
    runtime: &Arc<FileTransferRuntime>,
    identity: FileTransferIdentity,
    now: DateTime<Utc>,
) -> (FileTransferSnapshot, FileTransferSubscription) {
    let snapshot = runtime
        .register(identity, "Staff member".into(), "Laptop".into(), now)
        .unwrap();
    let subscription = runtime
        .subscribe(identity, snapshot.device.id, now)
        .unwrap();
    (snapshot, subscription)
}
fn request(device: Uuid, target: Uuid) -> CreateFileTransferPair {
    CreateFileTransferPair {
        device_id: device,
        target: FileTransferTarget::Device { device_id: target },
        file: file(),
    }
}
fn signal(device_id: Uuid, signal_type: FileTransferSignalType) -> SendFileTransferSignal {
    SendFileTransferSignal {
        device_id,
        signal_type,
        sdp: SDP.into(),
    }
}

#[test]
fn file_transfer_metadata_bounds_preserve_zero_and_unicode() {
    assert!(validate_file(&file()).is_ok());
    for name in ["", " ", "../data", "folder\\data", "a\nb", ".", ".."] {
        let mut f = file();
        f.name = name.into();
        assert!(validate_file(&f).is_err());
    }
    let mut f = file();
    f.name = "ก".repeat(255);
    f.size = MAX_FILE_SIZE;
    assert!(validate_file(&f).is_ok());
    f.name.push('ก');
    assert!(validate_file(&f).is_err());
    f = file();
    f.size = MAX_FILE_SIZE + 1;
    assert!(validate_file(&f).is_err());
    f = file();
    f.mime_type = "x".repeat(129);
    assert!(validate_file(&f).is_err());
}
#[test]
fn file_transfer_sdp_allows_only_bounded_complete_datachannel_host_candidates() {
    assert!(validate_sdp(SDP).is_ok());
    for bad in [
        SDP.replace("typ host", "typ relay"),
        SDP.replace("typ host", "typ srflx"),
        SDP.replace("m=application", "m=audio"),
        SDP.replace("a=sctp-port:5000\r\n", ""),
        SDP.replace("a=candidate:", "a=ignored:"),
        format!("{SDP}m=video 9 UDP/TLS/RTP/SAVPF 96\r\n"),
        "x".repeat(32769),
    ] {
        assert!(validate_sdp(&bad).is_err());
    }
}
#[tokio::test]
async fn file_transfer_identity_scope_is_exact_and_discovery_never_crosses_tenants() {
    let now = Utc::now();
    let runtime = Arc::new(FileTransferRuntime::default());
    let a = owner(Uuid::new_v4());
    let b = owner(a.tenant_id);
    let c = owner(Uuid::new_v4());
    let (ad, _as) = online(&runtime, a, now);
    let (bd, _bs) = online(&runtime, b, now);
    let (cd, _cs) = online(&runtime, c, now);
    let snapshot = runtime.snapshot(a, ad.device.id, now).unwrap();
    assert_eq!(snapshot.peers.len(), 1);
    assert_eq!(snapshot.peers[0].id, bd.device.id);
    for foreign in [
        b,
        c,
        FileTransferIdentity {
            session_id: Uuid::new_v4(),
            ..a
        },
    ] {
        assert!(runtime.snapshot(foreign, ad.device.id, now).is_err());
        assert!(runtime.remove(foreign, ad.device.id, now).is_err());
    }
    assert!(runtime
        .create_pair(a, request(ad.device.id, cd.device.id), now)
        .is_err());
    assert!(runtime
        .create_pair(
            a,
            CreateFileTransferPair {
                device_id: ad.device.id,
                target: FileTransferTarget::Code {
                    code: cd.pairing_code
                },
                file: file()
            },
            now
        )
        .is_err());
}
#[tokio::test]
async fn file_transfer_consent_direction_sequence_and_exact_metadata() {
    let now = Utc::now();
    let runtime = Arc::new(FileTransferRuntime::default());
    let a = owner(Uuid::new_v4());
    let b = owner(a.tenant_id);
    let (ad, mut asub) = online(&runtime, a, now);
    let (bd, mut bsub) = online(&runtime, b, now);
    let pair = runtime
        .create_pair(a, request(ad.device.id, bd.device.id), now)
        .unwrap();
    assert!(pair.file == file());
    assert!(runtime
        .signal(
            a,
            pair.id,
            signal(ad.device.id, FileTransferSignalType::Offer),
            now
        )
        .is_err());
    assert!(runtime.accept(a, ad.device.id, pair.id, now).is_err());
    let accepted = runtime.accept(b, bd.device.id, pair.id, now).unwrap();
    assert!(accepted.file == pair.file);
    assert!(accepted.status == FileTransferPairStatus::Accepted);
    assert!(runtime.accept(b, bd.device.id, pair.id, now).is_ok());
    assert!(runtime
        .signal(
            b,
            pair.id,
            signal(bd.device.id, FileTransferSignalType::Answer),
            now
        )
        .is_err());
    assert!(runtime
        .signal(
            b,
            pair.id,
            signal(bd.device.id, FileTransferSignalType::Offer),
            now
        )
        .is_err());
    runtime
        .signal(
            a,
            pair.id,
            signal(ad.device.id, FileTransferSignalType::Offer),
            now,
        )
        .unwrap();
    assert!(runtime
        .signal(
            a,
            pair.id,
            signal(ad.device.id, FileTransferSignalType::Offer),
            now
        )
        .is_err());
    runtime
        .signal(
            b,
            pair.id,
            signal(bd.device.id, FileTransferSignalType::Answer),
            now,
        )
        .unwrap();
    let mut received_offer = false;
    while let Ok(event) = bsub.receiver.try_recv() {
        if let FileTransferEvent::Signal(s) = event {
            assert!(s.signal_type == FileTransferSignalType::Offer);
            assert_eq!(s.sdp, SDP);
            received_offer = true;
        }
    }
    assert!(received_offer);
    let mut received_answer = false;
    while let Ok(event) = asub.receiver.try_recv() {
        if let FileTransferEvent::Signal(s) = event {
            assert!(s.signal_type == FileTransferSignalType::Answer);
            received_answer = true;
        }
    }
    assert!(received_answer);
}
#[tokio::test]
async fn file_transfer_disconnect_immediately_cancels_pairs_and_removes_presence() {
    let now = Utc::now();
    let runtime = Arc::new(FileTransferRuntime::default());
    let a = owner(Uuid::new_v4());
    let b = owner(a.tenant_id);
    let (ad, asub) = online(&runtime, a, now);
    let (bd, _bsub) = online(&runtime, b, now);
    runtime
        .create_pair(a, request(ad.device.id, bd.device.id), now)
        .unwrap();
    drop(asub);
    let snapshot = runtime.snapshot(b, bd.device.id, now).unwrap();
    assert!(snapshot.peers.is_empty());
    assert!(snapshot.pairs.is_empty());
    assert!(runtime.snapshot(a, ad.device.id, now).is_err());
}
#[tokio::test]
async fn file_transfer_ttl_and_signaling_window_do_not_kill_healthy_long_transfers() {
    let now = Utc::now();
    let runtime = Arc::new(FileTransferRuntime::default());
    let a = owner(Uuid::new_v4());
    let b = owner(a.tenant_id);
    let (ad, _asub) = online(&runtime, a, now);
    let (bd, _bsub) = online(&runtime, b, now);
    let pair = runtime
        .create_pair(a, request(ad.device.id, bd.device.id), now)
        .unwrap();
    for delta in [60, 120] {
        runtime
            .refresh(a, ad.device.id, now + Duration::seconds(delta))
            .unwrap();
        runtime
            .refresh(b, bd.device.id, now + Duration::seconds(delta))
            .unwrap();
    }
    assert!(runtime
        .accept(b, bd.device.id, pair.id, now + Duration::seconds(120))
        .is_err());
    let pair = runtime
        .create_pair(
            a,
            request(ad.device.id, bd.device.id),
            now + Duration::seconds(120),
        )
        .unwrap();
    runtime
        .accept(b, bd.device.id, pair.id, now + Duration::seconds(120))
        .unwrap();
    for delta in (180..=780).step_by(60) {
        runtime
            .refresh(a, ad.device.id, now + Duration::seconds(delta))
            .unwrap();
        runtime
            .refresh(b, bd.device.id, now + Duration::seconds(delta))
            .unwrap();
    }
    assert_eq!(
        runtime
            .snapshot(a, ad.device.id, now + Duration::seconds(780))
            .unwrap()
            .pairs
            .len(),
        1
    );
    assert!(runtime
        .signal(
            a,
            pair.id,
            signal(ad.device.id, FileTransferSignalType::Offer),
            now + Duration::seconds(780)
        )
        .is_err());
    assert!(runtime
        .snapshot(a, ad.device.id, now + Duration::seconds(871))
        .is_err());
}
#[tokio::test]
async fn file_transfer_pairing_codes_and_limits_are_bounded_per_session() {
    let now = Utc::now();
    let runtime = Arc::new(FileTransferRuntime::default());
    let a = owner(Uuid::new_v4());
    let b = owner(a.tenant_id);
    let (ad, _asub) = online(&runtime, a, now);
    let (bd, _bsub) = online(&runtime, b, now);
    for _ in 0..5 {
        assert!(runtime
            .create_pair(
                a,
                CreateFileTransferPair {
                    device_id: ad.device.id,
                    target: FileTransferTarget::Code {
                        code: "0000000000".into()
                    },
                    file: file()
                },
                now
            )
            .is_err());
    }
    assert!(matches!(
        runtime.create_pair(
            a,
            CreateFileTransferPair {
                device_id: ad.device.id,
                target: FileTransferTarget::Code {
                    code: bd.pairing_code.clone()
                },
                file: file()
            },
            now
        ),
        Err(AppError::RateLimited { .. })
    ));
    assert!(runtime
        .create_pair(
            a,
            CreateFileTransferPair {
                device_id: ad.device.id,
                target: FileTransferTarget::Code {
                    code: bd.pairing_code
                },
                file: file()
            },
            now + Duration::seconds(61)
        )
        .is_ok());
    runtime
        .register(
            a,
            "Staff".into(),
            "Second".into(),
            now + Duration::seconds(61),
        )
        .unwrap();
    runtime
        .register(
            a,
            "Staff".into(),
            "Third".into(),
            now + Duration::seconds(61),
        )
        .unwrap();
    assert!(matches!(
        runtime.register(
            a,
            "Staff".into(),
            "Fourth".into(),
            now + Duration::seconds(61)
        ),
        Err(AppError::RateLimited { .. })
    ));
}
#[tokio::test]
async fn file_transfer_presence_bursts_coalesce_but_signal_overflow_fails_closed() {
    let now = Utc::now();
    let runtime = Arc::new(FileTransferRuntime::default());
    let a = owner(Uuid::new_v4());
    let (ad, mut asub) = online(&runtime, a, now);
    for _ in 0..100 {
        let mut registry = runtime.lock(now).unwrap();
        let messages = registry.presence_messages(a.tenant_id);
        registry.dispatch(messages);
    }
    assert!(asub.presence.has_changed().unwrap());
    assert!(runtime.snapshot(a, ad.device.id, now).is_ok());
    {
        let mut registry = runtime.lock(now).unwrap();
        for _ in 0..QUEUE_CAPACITY + 1 {
            registry.dispatch(vec![(
                ad.device.id,
                FileTransferEvent::PairClosed(FileTransferPairClosed {
                    pair_id: Uuid::new_v4(),
                    reason: FileTransferCloseReason::Cancelled,
                }),
            )]);
        }
    }
    asub.cancelled.changed().await.unwrap();
    assert!(*asub.cancelled.borrow());
    assert!(runtime.snapshot(a, ad.device.id, now).is_err());
}
#[tokio::test]
async fn file_transfer_pair_reservation_is_atomic_and_foreign_members_cannot_decide() {
    let now = Utc::now();
    let runtime = Arc::new(FileTransferRuntime::default());
    let a = owner(Uuid::new_v4());
    let b = owner(a.tenant_id);
    let c = owner(a.tenant_id);
    let (ad, _as) = online(&runtime, a, now);
    let (bd, _bs) = online(&runtime, b, now);
    let (cd, _cs) = online(&runtime, c, now);
    let pair = runtime
        .create_pair(a, request(ad.device.id, bd.device.id), now)
        .unwrap();
    assert!(runtime
        .create_pair(c, request(cd.device.id, bd.device.id), now)
        .is_err());
    assert!(runtime.accept(c, cd.device.id, pair.id, now).is_err());
    assert!(runtime.close(c, cd.device.id, pair.id, now).is_err());
    runtime.close(b, bd.device.id, pair.id, now).unwrap();
    assert!(runtime
        .create_pair(c, request(cd.device.id, bd.device.id), now)
        .is_ok());
}
#[test]
fn file_transfer_event_wire_shape_keeps_nested_signal_type_distinct() {
    let event = FileTransferEvent::Signal(FileTransferSignal {
        pair_id: Uuid::nil(),
        signal_type: FileTransferSignalType::Offer,
        sdp: SDP.into(),
    });
    let wire = serde_json::to_value(event).unwrap();
    assert_eq!(wire["type"], "signal");
    assert_eq!(wire["data"]["type"], "offer");
    assert_eq!(wire["data"]["pairId"], Uuid::nil().to_string());
}

#[test]
fn file_transfer_registration_is_globally_and_tenant_bounded() {
    let now = Utc::now();
    let runtime = FileTransferRuntime::default();
    for _ in 0..4 {
        let tenant = Uuid::new_v4();
        for _ in 0..64 {
            runtime
                .register(owner(tenant), "Staff".into(), "Laptop".into(), now)
                .unwrap();
        }
        assert!(matches!(
            runtime.register(owner(tenant), "Staff".into(), "Overflow".into(), now),
            Err(AppError::RateLimited { .. })
        ));
    }
    assert!(matches!(
        runtime.register(
            owner(Uuid::new_v4()),
            "Staff".into(),
            "Global overflow".into(),
            now
        ),
        Err(AppError::RateLimited { .. })
    ));
    assert!(runtime
        .register(
            owner(Uuid::new_v4()),
            "Staff".into(),
            "After expiry".into(),
            now + Duration::seconds(91)
        )
        .is_ok());
}
