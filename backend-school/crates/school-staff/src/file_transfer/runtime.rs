use super::models::*;
use chrono::{DateTime, Duration, Utc};
use school_errors::AppError;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u8 = 2;
pub const MAX_OUTGOING_TRANSFERS: u8 = 3;
pub const MAX_FILES_PER_BATCH: u8 = 20;
pub const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;
pub const MAX_BATCH_BYTES: u64 = 128 * 1024 * 1024;
const DEVICE_SECONDS: i64 = 90;
const MAX_DEVICES: usize = 256;
const QUEUE_CAPACITY: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileTransferIdentity {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
}

struct Device {
    identity: FileTransferIdentity,
    public: FileTransferDevice,
    code: String,
    expires_at: DateTime<Utc>,
    stream: Option<DeviceStream>,
}
struct DeviceStream {
    generation: Uuid,
    sender: mpsc::Sender<FileTransferEvent>,
    cancelled: watch::Sender<bool>,
    presence: watch::Sender<u64>,
}
struct Pair {
    tenant_id: Uuid,
    public: FileTransferPair,
    offered: bool,
    answered: bool,
}
#[derive(Default)]
struct Registry {
    devices: HashMap<Uuid, Device>,
    pairs: HashMap<Uuid, Pair>,
    attempts: VecDeque<(FileTransferIdentity, DateTime<Utc>)>,
}
#[derive(Default)]
pub struct FileTransferRuntime {
    registry: Mutex<Registry>,
}

pub struct FileTransferSubscription {
    pub receiver: mpsc::Receiver<FileTransferEvent>,
    pub cancelled: watch::Receiver<bool>,
    pub presence: watch::Receiver<u64>,
    pub guard: FileTransferStreamGuard,
}
pub struct FileTransferStreamGuard {
    runtime: Arc<FileTransferRuntime>,
    pub(super) device_id: Uuid,
    generation: Uuid,
}
impl FileTransferStreamGuard {
    pub fn disconnect(&self) {
        if let Ok(mut registry) = self.runtime.registry.lock() {
            if registry
                .devices
                .get(&self.device_id)
                .and_then(|d| d.stream.as_ref())
                .is_some_and(|s| s.generation == self.generation)
            {
                registry.remove_device(self.device_id);
            }
        }
    }
}
impl Drop for FileTransferStreamGuard {
    fn drop(&mut self) {
        self.disconnect();
    }
}
fn missing() -> AppError {
    AppError::NotFound("File transfer device or pair unavailable".into())
}
fn conflict() -> AppError {
    AppError::Conflict("File transfer state changed; reconnect and try again".into())
}
fn invalid(message: &str) -> AppError {
    AppError::ValidationError(message.into())
}

impl Registry {
    fn owned(&self, identity: FileTransferIdentity, id: Uuid) -> Result<&Device, AppError> {
        self.devices
            .get(&id)
            .filter(|d| d.identity == identity)
            .ok_or_else(missing)
    }
    fn online(&self, id: Uuid) -> bool {
        self.devices.get(&id).is_some_and(|d| d.stream.is_some())
    }
    fn pair_member(
        &self,
        identity: FileTransferIdentity,
        device: Uuid,
        pair: Uuid,
    ) -> Result<&Pair, AppError> {
        self.owned(identity, device)?;
        self.pairs
            .get(&pair)
            .filter(|p| {
                p.tenant_id == identity.tenant_id
                    && (p.public.initiator.id == device || p.public.recipient.id == device)
            })
            .ok_or_else(missing)
    }
    fn snapshot(
        &self,
        identity: FileTransferIdentity,
        id: Uuid,
    ) -> Result<FileTransferSnapshot, AppError> {
        let device = self.owned(identity, id)?;
        let mut peers: Vec<_> = self
            .devices
            .values()
            .filter(|d| {
                d.identity.tenant_id == identity.tenant_id
                    && d.public.id != id
                    && d.stream.is_some()
            })
            .map(|d| d.public.clone())
            .collect();
        peers.sort_by_key(|d| d.id);
        let mut pairs: Vec<_> = self
            .pairs
            .values()
            .filter(|p| {
                p.tenant_id == identity.tenant_id
                    && (p.public.initiator.id == id || p.public.recipient.id == id)
            })
            .map(|p| p.public.clone())
            .collect();
        pairs.sort_by_key(|p| p.id);
        Ok(FileTransferSnapshot {
            protocol_version: PROTOCOL_VERSION,
            limits: limits(),
            device: device.public.clone(),
            pairing_code: device.code.clone(),
            expires_at: device.expires_at,
            peers,
            pairs,
        })
    }
    // Queue overflow invalidates the binding. The cancellation watch takes priority over
    // buffered events, so stale signaling cannot continue after bounded delivery fails.
    fn dispatch(&mut self, messages: Vec<(Uuid, FileTransferEvent)>) {
        let mut pending: VecDeque<_> = messages.into();
        while let Some((id, event)) = pending.pop_front() {
            if matches!(event, FileTransferEvent::PresenceChanged(_)) {
                if let Some(stream) = self.devices.get(&id).and_then(|d| d.stream.as_ref()) {
                    stream
                        .presence
                        .send_modify(|version| *version = version.wrapping_add(1));
                }
                continue;
            }
            let failed = self
                .devices
                .get(&id)
                .and_then(|d| d.stream.as_ref())
                .is_none_or(|s| s.sender.try_send(event).is_err());
            if failed {
                pending.extend(self.remove_device_messages(id));
            }
        }
    }
    fn presence_messages(&self, tenant: Uuid) -> Vec<(Uuid, FileTransferEvent)> {
        self.devices
            .values()
            .filter(|d| d.identity.tenant_id == tenant && d.stream.is_some())
            .map(|d| {
                (
                    d.public.id,
                    FileTransferEvent::PresenceChanged(FileTransferEmpty {}),
                )
            })
            .collect()
    }
    fn remove_device_messages(&mut self, id: Uuid) -> Vec<(Uuid, FileTransferEvent)> {
        let Some(device) = self.devices.remove(&id) else {
            return Vec::new();
        };
        if let Some(stream) = device.stream {
            let _ = stream.cancelled.send(true);
        }
        let pair_ids: Vec<_> = self
            .pairs
            .iter()
            .filter(|(_, p)| p.public.initiator.id == id || p.public.recipient.id == id)
            .map(|(id, _)| *id)
            .collect();
        let mut messages = self.presence_messages(device.identity.tenant_id);
        for pair_id in pair_ids {
            if let Some(pair) = self.pairs.remove(&pair_id) {
                let peer = if pair.public.initiator.id == id {
                    pair.public.recipient.id
                } else {
                    pair.public.initiator.id
                };
                messages.push((
                    peer,
                    FileTransferEvent::PairClosed(FileTransferPairClosed {
                        pair_id,
                        reason: FileTransferCloseReason::Disconnected,
                    }),
                ));
            }
        }
        messages
    }
    fn remove_device(&mut self, id: Uuid) {
        let messages = self.remove_device_messages(id);
        self.dispatch(messages);
    }
    fn sweep(&mut self, now: DateTime<Utc>) {
        let expired_devices: Vec<_> = self
            .devices
            .iter()
            .filter(|(_, d)| d.expires_at <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in expired_devices {
            self.remove_device(id);
        }
        let expired_pairs: Vec<_> = self
            .pairs
            .iter()
            .filter(|(_, p)| {
                p.public.status == FileTransferPairStatus::Pending && p.public.expires_at <= now
            })
            .map(|(id, _)| *id)
            .collect();
        for id in expired_pairs {
            self.close_pair(id, FileTransferCloseReason::Expired);
        }
        self.attempts
            .retain(|(_, at)| *at > now - Duration::minutes(1));
    }
    fn close_pair(&mut self, id: Uuid, reason: FileTransferCloseReason) {
        if let Some(pair) = self.pairs.remove(&id) {
            let event = FileTransferEvent::PairClosed(FileTransferPairClosed {
                pair_id: id,
                reason,
            });
            self.dispatch(vec![
                (pair.public.initiator.id, event.clone()),
                (pair.public.recipient.id, event),
            ]);
        }
    }
}

impl FileTransferRuntime {
    fn lock(&self, now: DateTime<Utc>) -> Result<MutexGuard<'_, Registry>, AppError> {
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| AppError::ServiceUnavailable("File transfer unavailable".into()))?;
        registry.sweep(now);
        Ok(registry)
    }
    pub fn register(
        &self,
        identity: FileTransferIdentity,
        display_name: String,
        request: RegisterFileTransferDevice,
        now: DateTime<Utc>,
    ) -> Result<FileTransferSnapshot, AppError> {
        validate_protocol(request.protocol_version)?;
        let label = request.label.trim().to_string();
        if label.is_empty() || label.chars().count() > 60 || label.chars().any(char::is_control) {
            return Err(invalid("Device label must contain 1–60 characters"));
        }
        let mut registry = self.lock(now)?;
        if registry.devices.len() >= MAX_DEVICES
            || registry
                .devices
                .values()
                .filter(|d| d.identity.tenant_id == identity.tenant_id)
                .count()
                >= 64
            || registry
                .devices
                .values()
                .filter(|d| d.identity == identity)
                .count()
                >= 3
        {
            return Err(AppError::RateLimited {
                retry_after_seconds: 90,
            });
        }
        let id = Uuid::now_v7();
        let code = loop {
            let code: String = Uuid::new_v4()
                .simple()
                .to_string()
                .chars()
                .take(10)
                .map(|c| c.to_ascii_uppercase())
                .collect();
            if !registry.devices.values().any(|d| d.code == code) {
                break code;
            }
        };
        registry.devices.insert(
            id,
            Device {
                identity,
                public: FileTransferDevice {
                    id,
                    user_id: identity.user_id,
                    display_name,
                    label,
                },
                code,
                expires_at: now + Duration::seconds(DEVICE_SECONDS),
                stream: None,
            },
        );
        registry.snapshot(identity, id)
    }
    pub fn snapshot(
        &self,
        identity: FileTransferIdentity,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<FileTransferSnapshot, AppError> {
        self.lock(now)?.snapshot(identity, id)
    }
    pub fn subscribe(
        self: &Arc<Self>,
        identity: FileTransferIdentity,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<FileTransferSubscription, AppError> {
        let mut registry = self.lock(now)?;
        if registry.owned(identity, id)?.stream.is_some() {
            return Err(conflict());
        }
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let (cancelled, cancelled_rx) = watch::channel(false);
        let (presence, presence_rx) = watch::channel(0);
        let generation = Uuid::new_v4();
        let device = registry.devices.get_mut(&id).ok_or_else(missing)?;
        device.stream = Some(DeviceStream {
            generation,
            sender,
            cancelled,
            presence,
        });
        device.expires_at = now + Duration::seconds(DEVICE_SECONDS);
        let snapshot = registry.snapshot(identity, id)?;
        registry.dispatch(vec![(id, FileTransferEvent::Ready(snapshot))]);
        let messages = registry
            .presence_messages(identity.tenant_id)
            .into_iter()
            .filter(|(peer, _)| *peer != id)
            .collect();
        registry.dispatch(messages);
        Ok(FileTransferSubscription {
            receiver,
            cancelled: cancelled_rx,
            presence: presence_rx,
            guard: FileTransferStreamGuard {
                runtime: self.clone(),
                device_id: id,
                generation,
            },
        })
    }
    pub fn refresh(
        &self,
        identity: FileTransferIdentity,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let mut registry = self.lock(now)?;
        registry.owned(identity, id)?;
        registry
            .devices
            .get_mut(&id)
            .ok_or_else(missing)?
            .expires_at = now + Duration::seconds(DEVICE_SECONDS);
        Ok(())
    }
    pub fn remove(
        &self,
        identity: FileTransferIdentity,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let mut registry = self.lock(now)?;
        registry.owned(identity, id)?;
        registry.remove_device(id);
        Ok(())
    }
    pub fn create_pair(
        &self,
        identity: FileTransferIdentity,
        request: CreateFileTransferPair,
        now: DateTime<Utc>,
    ) -> Result<FileTransferPair, AppError> {
        validate_batch(&request.files)?;
        let mut registry = self.lock(now)?;
        registry.owned(identity, request.device_id)?;
        let target_id = match request.target {
            FileTransferTarget::Device { device_id } => device_id,
            FileTransferTarget::Code { code } => {
                if registry.attempts.len() >= MAX_DEVICES * 5 {
                    return Err(AppError::RateLimited {
                        retry_after_seconds: 60,
                    });
                }
                if registry
                    .attempts
                    .iter()
                    .filter(|(owner, _)| *owner == identity)
                    .count()
                    >= 5
                {
                    return Err(AppError::RateLimited {
                        retry_after_seconds: 60,
                    });
                }
                registry.attempts.push_back((identity, now));
                if code.len() != 10 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(missing());
                }
                registry
                    .devices
                    .values()
                    .find(|d| {
                        d.identity.tenant_id == identity.tenant_id
                            && d.code.eq_ignore_ascii_case(&code)
                    })
                    .map(|d| d.public.id)
                    .ok_or_else(missing)?
            }
        };
        let target = registry
            .devices
            .get(&target_id)
            .filter(|d| d.identity.tenant_id == identity.tenant_id)
            .ok_or_else(missing)?;
        if target_id == request.device_id
            || !registry.online(target_id)
            || !registry.online(request.device_id)
        {
            return Err(conflict());
        }
        let outgoing = registry
            .pairs
            .values()
            .filter(|pair| pair.public.initiator.id == request.device_id)
            .count();
        let incoming = registry
            .pairs
            .values()
            .any(|pair| pair.public.recipient.id == request.device_id);
        let recipient_busy = registry.pairs.values().any(|pair| {
            pair.public.initiator.id == target_id || pair.public.recipient.id == target_id
        });
        if incoming || outgoing >= usize::from(MAX_OUTGOING_TRANSFERS) || recipient_busy {
            return Err(AppError::Conflict(
                "อุปกรณ์กำลังรับไฟล์หรือมีรายการส่งครบจำนวนแล้ว".into(),
            ));
        }
        // Correlation is scoped to active pairs of this exact initiating device;
        // request IDs never replace authenticated ownership or become replay receipts.
        if registry.pairs.values().any(|pair| {
            pair.public.initiator.id == request.device_id
                && pair.public.client_request_id == request.client_request_id
        }) {
            return Err(AppError::Conflict(
                "คำขอนี้กำลังดำเนินการ กรุณาตรวจสอบรายการปัจจุบัน".into(),
            ));
        }
        let pair = FileTransferPair {
            id: Uuid::now_v7(),
            initiator: registry.owned(identity, request.device_id)?.public.clone(),
            recipient: target.public.clone(),
            files: request.files,
            client_request_id: request.client_request_id,
            status: FileTransferPairStatus::Pending,
            expires_at: now + Duration::seconds(120),
        };
        registry.pairs.insert(
            pair.id,
            Pair {
                tenant_id: identity.tenant_id,
                public: pair.clone(),
                offered: false,
                answered: false,
            },
        );
        registry.dispatch(vec![
            (
                pair.initiator.id,
                FileTransferEvent::PairChanged(pair.clone()),
            ),
            (target_id, FileTransferEvent::PairChanged(pair.clone())),
        ]);
        if !registry.pairs.contains_key(&pair.id) {
            return Err(conflict());
        }
        Ok(pair)
    }
    pub fn accept(
        &self,
        identity: FileTransferIdentity,
        device: Uuid,
        pair_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<FileTransferPair, AppError> {
        let mut registry = self.lock(now)?;
        let pair = registry.pair_member(identity, device, pair_id)?;
        if pair.public.recipient.id != device {
            return Err(AppError::Forbidden("Only the recipient can accept".into()));
        }
        if pair.public.status == FileTransferPairStatus::Accepted {
            return Ok(pair.public.clone());
        }
        let pair = registry.pairs.get_mut(&pair_id).ok_or_else(missing)?;
        pair.public.status = FileTransferPairStatus::Accepted;
        pair.public.expires_at = now + Duration::minutes(10);
        let result = pair.public.clone();
        registry.dispatch(vec![
            (
                result.initiator.id,
                FileTransferEvent::PairChanged(result.clone()),
            ),
            (device, FileTransferEvent::PairChanged(result.clone())),
        ]);
        if !registry.pairs.contains_key(&pair_id) {
            return Err(conflict());
        }
        Ok(result)
    }
    pub fn close(
        &self,
        identity: FileTransferIdentity,
        device: Uuid,
        pair_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let mut registry = self.lock(now)?;
        let pair = registry.pair_member(identity, device, pair_id)?;
        let reason = if pair.public.status == FileTransferPairStatus::Pending
            && pair.public.recipient.id == device
        {
            FileTransferCloseReason::Declined
        } else {
            FileTransferCloseReason::Cancelled
        };
        registry.close_pair(pair_id, reason);
        Ok(())
    }
    pub fn signal(
        &self,
        identity: FileTransferIdentity,
        pair_id: Uuid,
        request: SendFileTransferSignal,
        now: DateTime<Utc>,
    ) -> Result<(), AppError> {
        validate_sdp(&request.sdp)?;
        let mut registry = self.lock(now)?;
        let pair = registry.pair_member(identity, request.device_id, pair_id)?;
        if pair.public.status != FileTransferPairStatus::Accepted || pair.public.expires_at <= now {
            return Err(conflict());
        }
        let target = match request.signal_type {
            FileTransferSignalType::Offer
                if pair.public.initiator.id == request.device_id && !pair.offered =>
            {
                pair.public.recipient.id
            }
            FileTransferSignalType::Answer
                if pair.public.recipient.id == request.device_id
                    && pair.offered
                    && !pair.answered =>
            {
                pair.public.initiator.id
            }
            _ => return Err(conflict()),
        };
        let pair = registry.pairs.get_mut(&pair_id).ok_or_else(missing)?;
        match request.signal_type {
            FileTransferSignalType::Offer => pair.offered = true,
            FileTransferSignalType::Answer => pair.answered = true,
        }
        registry.dispatch(vec![(
            target,
            FileTransferEvent::Signal(FileTransferSignal {
                pair_id,
                signal_type: request.signal_type,
                sdp: request.sdp,
            }),
        )]);
        if !registry.pairs.contains_key(&pair_id) {
            return Err(conflict());
        }
        Ok(())
    }
}

pub fn limits() -> FileTransferLimits {
    FileTransferLimits {
        max_outgoing_transfers: MAX_OUTGOING_TRANSFERS,
        max_files_per_batch: MAX_FILES_PER_BATCH,
        max_file_bytes: MAX_FILE_SIZE,
        max_batch_bytes: MAX_BATCH_BYTES,
    }
}

pub fn validate_protocol(version: Option<u8>) -> Result<(), AppError> {
    if version != Some(PROTOCOL_VERSION) {
        return Err(invalid(
            "กรุณารีเฟรชหน้าเว็บทั้งอุปกรณ์ส่งและอุปกรณ์รับ เพื่อใช้การรับส่งไฟล์รุ่นล่าสุด",
        ));
    }
    Ok(())
}

pub fn validate_batch(files: &[FileTransferFile]) -> Result<(), AppError> {
    if files.is_empty() || files.len() > usize::from(MAX_FILES_PER_BATCH) {
        return Err(invalid("กรุณาเลือกไฟล์ตั้งแต่ 1 ถึง 20 ไฟล์"));
    }
    files.iter().try_fold(0_u64, |total, file| {
        validate_file(file)?;
        total
            .checked_add(file.size)
            .filter(|sum| *sum <= MAX_BATCH_BYTES)
            .ok_or_else(|| invalid("ไฟล์รวมในชุดต้องไม่เกิน 128 MiB"))
    })?;
    Ok(())
}

pub fn validate_file(file: &FileTransferFile) -> Result<(), AppError> {
    if file.size > MAX_FILE_SIZE
        || file.name.trim().is_empty()
        || file.name.chars().count() > 255
        || file
            .name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control())
        || file.name == "."
        || file.name == ".."
        || file.mime_type.chars().count() > 128
        || file.mime_type.chars().any(char::is_control)
    {
        return Err(invalid("Invalid file metadata or file exceeds 64 MiB"));
    }
    Ok(())
}

pub fn validate_sdp(sdp: &str) -> Result<(), AppError> {
    let fail = || invalid("A complete host-only WebRTC data channel description is required");
    if sdp.len() > 32 * 1024
        || !sdp.starts_with("v=0\r\n")
        || sdp
            .chars()
            .any(|c| c.is_control() && c != '\r' && c != '\n')
    {
        return Err(fail());
    }
    let lines: Vec<_> = sdp.lines().collect();
    let media: Vec<_> = lines.iter().filter(|l| l.starts_with("m=")).collect();
    if media.len() != 1
        || !media[0].starts_with("m=application ")
        || !media[0].ends_with(" UDP/DTLS/SCTP webrtc-datachannel")
        || !lines.iter().any(|l| {
            l.strip_prefix("a=ice-ufrag:")
                .is_some_and(|value| ice_token(value, 4))
        })
        || !lines.iter().any(|l| {
            l.strip_prefix("a=ice-pwd:")
                .is_some_and(|value| ice_token(value, 22))
        })
        || !lines.iter().any(|l| {
            l.strip_prefix("a=fingerprint:sha-256 ")
                .is_some_and(valid_fingerprint)
        })
        || !lines.iter().any(|l| {
            l.strip_prefix("a=sctp-port:")
                .is_some_and(|port| port.parse::<u16>().is_ok_and(|port| port > 0))
        })
        || !lines.iter().any(|line| {
            matches!(
                *line,
                "a=setup:actpass" | "a=setup:active" | "a=setup:passive"
            )
        })
        || lines
            .iter()
            .any(|line| line.starts_with("a=remote-candidates:"))
    {
        return Err(fail());
    }
    let mut candidates = 0;
    for line in lines.iter().filter(|l| l.starts_with("a=candidate:")) {
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        if fields.len() < 8
            || fields[6] != "typ"
            || fields[7] != "host"
            || fields[1] != "1"
            || !matches!(fields[2].to_ascii_lowercase().as_str(), "udp" | "tcp")
            || fields[3].parse::<u32>().is_err()
            || !valid_candidate_address(fields[4])
            || !fields[5].parse::<u16>().is_ok_and(|port| port > 0)
            || fields[8..].contains(&"typ")
        {
            return Err(fail());
        }
        candidates += 1;
    }
    if candidates == 0 || candidates > 64 {
        return Err(fail());
    }
    Ok(())
}

fn ice_token(value: &str, minimum: usize) -> bool {
    (minimum..=256).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/')
}
fn valid_fingerprint(value: &str) -> bool {
    let bytes: Vec<_> = value.split(':').collect();
    bytes.len() == 32
        && bytes
            .iter()
            .all(|byte| byte.len() == 2 && byte.bytes().all(|digit| digit.is_ascii_hexdigit()))
}
fn valid_candidate_address(value: &str) -> bool {
    value.parse::<std::net::IpAddr>().is_ok()
        || (value.len() <= 253
            && value.ends_with(".local")
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.'))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
