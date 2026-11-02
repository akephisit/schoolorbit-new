use super::*;

fn owner(tenant: Uuid) -> FileTransferIdentity {
    FileTransferIdentity {
        tenant_id: tenant,
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
    }
}
fn registration(label: &str) -> RegisterFileTransferDevice {
    RegisterFileTransferDevice {
        label: label.into(),
        protocol_version: Some(PROTOCOL_VERSION),
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
        .register(identity, "Staff member".into(), registration("Laptop"), now)
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
        files: vec![file()],
        client_request_id: Uuid::new_v4(),
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
                files: vec![file()],
                client_request_id: Uuid::new_v4()
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
    assert!(pair.files == vec![file()]);
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
    assert!(accepted.files == pair.files);
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
                    files: vec![file()],
                    client_request_id: Uuid::new_v4()
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
                files: vec![file()],
                client_request_id: Uuid::new_v4()
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
                files: vec![file()],
                client_request_id: Uuid::new_v4()
            },
            now + Duration::seconds(61)
        )
        .is_ok());
    runtime
        .register(
            a,
            "Staff".into(),
            registration("Second"),
            now + Duration::seconds(61),
        )
        .unwrap();
    runtime
        .register(
            a,
            "Staff".into(),
            registration("Third"),
            now + Duration::seconds(61),
        )
        .unwrap();
    assert!(matches!(
        runtime.register(
            a,
            "Staff".into(),
            registration("Fourth"),
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
                .register(owner(tenant), "Staff".into(), registration("Laptop"), now)
                .unwrap();
        }
        assert!(matches!(
            runtime.register(owner(tenant), "Staff".into(), registration("Overflow"), now),
            Err(AppError::RateLimited { .. })
        ));
    }
    assert!(matches!(
        runtime.register(
            owner(Uuid::new_v4()),
            "Staff".into(),
            registration("Global overflow"),
            now
        ),
        Err(AppError::RateLimited { .. })
    ));
    assert!(runtime
        .register(
            owner(Uuid::new_v4()),
            "Staff".into(),
            registration("After expiry"),
            now + Duration::seconds(91)
        )
        .is_ok());
}

#[test]
fn file_transfer_protocol_and_batch_limits_are_authoritative() {
    let runtime = FileTransferRuntime::default();
    let now = Utc::now();
    let actor = owner(Uuid::new_v4());
    for version in [None, Some(0), Some(1), Some(3), Some(255)] {
        assert!(runtime
            .register(
                actor,
                "Staff".into(),
                RegisterFileTransferDevice {
                    label: "Laptop".into(),
                    protocol_version: version
                },
                now
            )
            .is_err());
    }
    let snapshot = runtime
        .register(actor, "Staff".into(), registration("Laptop"), now)
        .unwrap();
    let wire = serde_json::to_value(snapshot).unwrap();
    assert_eq!(wire["protocolVersion"], 2);
    assert_eq!(
        wire["limits"],
        serde_json::json!({"maxOutgoingTransfers":3,"maxFilesPerBatch":20,"maxFileBytes":67108864,"maxBatchBytes":134217728})
    );
    assert!(validate_batch(&[]).is_err());
    assert!(validate_batch(&vec![file(); 20]).is_ok());
    assert!(validate_batch(&vec![file(); 21]).is_err());
    let mut maximum = file();
    maximum.size = MAX_FILE_SIZE;
    assert!(validate_batch(&[maximum.clone(), maximum.clone()]).is_ok());
    assert!(validate_batch(&[maximum.clone(), maximum.clone(), maximum]).is_err());
    let mut overflow = file();
    overflow.size = u64::MAX;
    assert!(validate_batch(&[overflow]).is_err());
    let mut unsafe_name = file();
    unsafe_name.name = "../private".into();
    assert!(validate_batch(&[file(), unsafe_name]).is_err());
}

#[tokio::test]
async fn file_transfer_three_recipient_burst_preserves_ordered_batch_consent_and_correlation() {
    let runtime = Arc::new(FileTransferRuntime::default());
    let now = Utc::now();
    let sender = owner(Uuid::new_v4());
    let (source, mut source_stream) = online(&runtime, sender, now);
    let mut receivers = Vec::new();
    let mut pairs = Vec::new();
    for index in 0..3 {
        let actor = owner(sender.tenant_id);
        let (device, stream) = online(&runtime, actor, now);
        let mut intent = request(source.device.id, device.device.id);
        let mut second = file();
        second.name = format!("second-{index}.txt");
        intent.files.push(second);
        let correlation = intent.client_request_id;
        let pair = runtime.create_pair(sender, intent, now).unwrap();
        assert_eq!(pair.client_request_id, correlation);
        assert_eq!(pair.files.len(), 2);
        assert_eq!(pair.files[1].name, format!("second-{index}.txt"));
        receivers.push((actor, device, stream));
        pairs.push(pair);
    }
    let extra = owner(sender.tenant_id);
    let (extra_device, _extra_stream) = online(&runtime, extra, now);
    assert!(runtime
        .create_pair(
            sender,
            request(source.device.id, extra_device.device.id),
            now
        )
        .is_err());
    let accepted = runtime
        .accept(receivers[0].0, receivers[0].1.device.id, pairs[0].id, now)
        .unwrap();
    assert!(accepted.files == pairs[0].files);
    assert!(runtime
        .signal(
            sender,
            pairs[1].id,
            signal(source.device.id, FileTransferSignalType::Offer),
            now
        )
        .is_err());
    for index in 1..3 {
        runtime
            .accept(
                receivers[index].0,
                receivers[index].1.device.id,
                pairs[index].id,
                now,
            )
            .unwrap();
    }
    for index in 0..3 {
        runtime
            .signal(
                sender,
                pairs[index].id,
                signal(source.device.id, FileTransferSignalType::Offer),
                now,
            )
            .unwrap();
        runtime
            .signal(
                receivers[index].0,
                pairs[index].id,
                signal(receivers[index].1.device.id, FileTransferSignalType::Answer),
                now,
            )
            .unwrap();
    }
    // Ready + 3 pending + 3 accepted + 3 answers must survive one undrained burst.
    let mut answers = Vec::new();
    let mut events = 0;
    while let Ok(event) = source_stream.receiver.try_recv() {
        events += 1;
        if let FileTransferEvent::Signal(value) = event {
            answers.push(value.pair_id);
        }
    }
    assert_eq!(events, 10);
    assert_eq!(answers.len(), 3);
    assert_eq!(
        runtime
            .snapshot(sender, source.device.id, now)
            .unwrap()
            .pairs
            .len(),
        3
    );
    for (index, (_, _, stream)) in receivers.iter_mut().enumerate() {
        let mut offers = Vec::new();
        while let Ok(event) = stream.receiver.try_recv() {
            if let FileTransferEvent::Signal(value) = event {
                offers.push(value.pair_id);
            }
        }
        assert_eq!(offers, vec![pairs[index].id]);
    }
    runtime
        .close(receivers[1].0, receivers[1].1.device.id, pairs[1].id, now)
        .unwrap();
    assert_eq!(
        runtime
            .snapshot(sender, source.device.id, now)
            .unwrap()
            .pairs
            .len(),
        2
    );
    let replacement = runtime
        .create_pair(
            sender,
            request(source.device.id, extra_device.device.id),
            now + Duration::seconds(30),
        )
        .unwrap();
    assert_eq!(replacement.expires_at, now + Duration::seconds(150));
    assert!(runtime
        .snapshot(sender, source.device.id, now)
        .unwrap()
        .pairs
        .iter()
        .any(|pair| pair.id == pairs[0].id && pair.status == FileTransferPairStatus::Accepted));
}

#[tokio::test]
async fn file_transfer_incoming_role_is_exclusive_and_request_ids_never_authorize() {
    let runtime = Arc::new(FileTransferRuntime::default());
    let now = Utc::now();
    let a = owner(Uuid::new_v4());
    let b = owner(a.tenant_id);
    let c = owner(a.tenant_id);
    let d = owner(a.tenant_id);
    let (ad, _as) = online(&runtime, a, now);
    let (bd, _bs) = online(&runtime, b, now);
    let (cd, _cs) = online(&runtime, c, now);
    let (dd, _ds) = online(&runtime, d, now);
    let initial = request(ad.device.id, bd.device.id);
    let correlation = initial.client_request_id;
    let pair = runtime.create_pair(a, initial, now).unwrap();
    assert!(runtime
        .create_pair(b, request(bd.device.id, cd.device.id), now)
        .is_err());
    assert!(runtime
        .create_pair(c, request(cd.device.id, ad.device.id), now)
        .is_err());
    let mut duplicate = request(ad.device.id, cd.device.id);
    duplicate.client_request_id = correlation;
    assert!(runtime.create_pair(a, duplicate, now).is_err());
    let mut independent = request(dd.device.id, cd.device.id);
    independent.client_request_id = correlation;
    assert!(runtime.create_pair(d, independent, now).is_ok());
    assert!(runtime.accept(c, cd.device.id, pair.id, now).is_err());
    assert!(runtime
        .signal(
            c,
            pair.id,
            signal(cd.device.id, FileTransferSignalType::Answer),
            now
        )
        .is_err());
    let foreign = FileTransferIdentity {
        tenant_id: Uuid::new_v4(),
        ..a
    };
    assert!(runtime.close(foreign, ad.device.id, pair.id, now).is_err());
    runtime.close(b, bd.device.id, pair.id, now).unwrap();
    assert!(runtime.accept(b, bd.device.id, pair.id, now).is_err());
    assert!(runtime
        .signal(
            a,
            pair.id,
            signal(ad.device.id, FileTransferSignalType::Offer),
            now
        )
        .is_err());
}

#[tokio::test]
async fn file_transfer_sender_drop_or_overflow_closes_every_pair_and_receiver_failure_is_isolated()
{
    for failure in ["drop", "overflow", "receiver-overflow"] {
        let runtime = Arc::new(FileTransferRuntime::default());
        let now = Utc::now();
        let sender = owner(Uuid::new_v4());
        let (source, source_stream) = online(&runtime, sender, now);
        let mut source_stream = Some(source_stream);
        let mut receivers = Vec::new();
        for _ in 0..3 {
            let actor = owner(sender.tenant_id);
            let (device, stream) = online(&runtime, actor, now);
            runtime
                .create_pair(sender, request(source.device.id, device.device.id), now)
                .unwrap();
            receivers.push((actor, device, stream));
        }
        match failure {
            "drop" => drop(source_stream.take()),
            _ => {
                let target = if failure == "overflow" {
                    source.device.id
                } else {
                    receivers[0].1.device.id
                };
                let mut registry = runtime.lock(now).unwrap();
                for _ in 0..=QUEUE_CAPACITY {
                    registry.dispatch(vec![(
                        target,
                        FileTransferEvent::PairClosed(FileTransferPairClosed {
                            pair_id: Uuid::new_v4(),
                            reason: FileTransferCloseReason::Cancelled,
                        }),
                    )]);
                }
            }
        }
        if failure == "receiver-overflow" {
            assert_eq!(
                runtime
                    .snapshot(sender, source.device.id, now)
                    .unwrap()
                    .pairs
                    .len(),
                2
            );
            assert!(runtime
                .snapshot(receivers[0].0, receivers[0].1.device.id, now)
                .is_err());
        } else {
            assert!(runtime.snapshot(sender, source.device.id, now).is_err());
        }
        for (index, (actor, device, _)) in receivers.iter().enumerate() {
            if failure == "receiver-overflow" && index == 0 {
                continue;
            }
            let state = runtime.snapshot(*actor, device.device.id, now).unwrap();
            assert_eq!(
                state.pairs.len(),
                usize::from(failure == "receiver-overflow")
            );
        }
    }
}

#[test]
fn file_transfer_concurrent_recipient_admission_never_exceeds_three() {
    let runtime = Arc::new(FileTransferRuntime::default());
    let now = Utc::now();
    let sender = owner(Uuid::new_v4());
    let (source, _source_stream) = online(&runtime, sender, now);
    let mut receivers = Vec::new();
    for _ in 0..4 {
        receivers.push(online(&runtime, owner(sender.tenant_id), now));
    }
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let handles: Vec<_> = receivers
        .iter()
        .map(|(device, _)| {
            let runtime = runtime.clone();
            let barrier = barrier.clone();
            let intent = request(source.device.id, device.device.id);
            std::thread::spawn(move || {
                barrier.wait();
                runtime.create_pair(sender, intent, now).is_ok()
            })
        })
        .collect();
    assert_eq!(
        handles
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum::<usize>(),
        3
    );
    assert_eq!(
        runtime
            .snapshot(sender, source.device.id, now)
            .unwrap()
            .pairs
            .len(),
        3
    );
}
