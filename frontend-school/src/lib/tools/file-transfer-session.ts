import { authAPI } from '#lib/api/auth.js';
import { recoveryDelay } from './file-transfer-recovery.js';
import {
	directoryPickerSupported,
	pickReceiveDirectory,
	destinationSelectionMessage,
	type ReceiveMode,
	type ReceiveDestination
} from './file-transfer-destination.js';
import { AutomaticDownloadPump } from './file-transfer-download-pump.js';
import { ApiClientError } from '#lib/api/client.js';
import { get } from 'svelte/store';
import { authStore } from '#lib/stores/auth.js';
import {
	browserVisibilityDependencies,
	createVisibilityIdle
} from '#lib/realtime/visibility-idle.js';
import {
	parseTransferEvent,
	transferAPI,
	type TransferPair,
	type TransferSnapshot,
	type TransferTarget
} from '#lib/api/file-transfer.js';
import {
	batchSummary,
	fileMetadata,
	sameFiles,
	sameBatch,
	validateBatchSummary,
	validateTransferBatch,
	TRANSFER_PAGE_SIZE,
	type FileMetadata,
	type BatchSummary
} from './file-transfer.js';
import {
	ReceivedBatchStore,
	receivingSupported,
	RECEIVING_UNSUPPORTED,
	storageMessage,
	collectAbandonedTransferStorage,
	checkStorageSpace,
	type StoredMetadata
} from './file-transfer-storage.js';
import { createTransferArchive } from './file-transfer-archive.js';
import {
	FileTransferTransport,
	type TransportPhase,
	type TransportProgress
} from './file-transfer-transport.js';

export type TransferPhase =
	| 'queued'
	| 'requesting'
	| 'waiting'
	| 'consent'
	| TransportPhase
	| 'declined'
	| 'cancelled'
	| 'failed';
export interface TransferJob {
	id: string;
	clientRequestId: string;
	direction: 'outgoing' | 'incoming';
	label: string;
	recipientId: string | null;
	pair: TransferPair | null;
	files: FileMetadata[];
	batch: BatchSummary;
	manifestPage: number;
	manifestPending: boolean;
	storageWarning: string;
	receiveMode: ReceiveMode;
	destinationLabel: string;
	megabytesPerSecond: number;
	phase: TransferPhase;
	bytes: number;
	fileIndex: number;
	completedFiles: number;
	error: string;
	cleanupError: string;
	pending: boolean;
	slotReserved: boolean;
}
export interface ReceivedFile {
	id: string;
	jobId: string;
	index: number;
	name: string;
	size: number;
}
export interface FileTransferState {
	status: 'idle' | 'connecting' | 'online' | 'reconnecting' | 'paused' | 'offline';
	draftEpoch: number;
	authBlocked: boolean;
	snapshot: TransferSnapshot | null;
	jobs: TransferJob[];
	downloads: ReceivedFile[];
	downloadCount: number;
	downloadBytes: number;
	downloadPage: number;
	downloadPending: boolean;
	downloadError: string;
	directoryPickerSupported: boolean;
	saveDestination: { mode: ReceiveMode; label: string } | null;
	automaticDownloadRequests: number;
	automaticDownloadPending: boolean;
	receivingSupported: boolean;
	archive: { pending: boolean; bytes: number; error: string };
	sourceFiles: FileMetadata[];
	error: string;
}
type Entry = {
	view: TransferJob;
	batchId: string;
	target: TransferTarget | null;
	controller: AbortController;
	transport: FileTransferTransport | null;
	consented: boolean;
	closingPairId: string | null;
	store: ReceivedBatchStore | null;
	manifestAttempt: number;
	consentOperation: number;
	intent: {
		operation: number;
		generation: number;
		authEpoch: number;
		deviceId: string;
		pairId: string;
		requestId: string;
		batch: BatchSummary;
		destination: ReceiveDestination;
	} | null;
};
type Batch = { id: string; files: File[]; metadata: FileMetadata[] };
const MAX_TRANSFER_JOBS = 64;
const terminal = new Set<TransferPhase>(['complete', 'declined', 'cancelled', 'failed']);
export function isTransferTerminal(phase: TransferPhase) {
	return terminal.has(phase);
}
export function initialTransferState(): FileTransferState {
	return {
		status: 'idle',
		draftEpoch: 0,
		authBlocked: get(authStore).isUnavailable,
		snapshot: null,
		jobs: [],
		downloads: [],
		downloadCount: 0,
		downloadBytes: 0,
		downloadPage: 0,
		downloadPending: false,
		downloadError: '',
		directoryPickerSupported: directoryPickerSupported(),
		saveDestination: null,
		automaticDownloadRequests: 0,
		automaticDownloadPending: false,
		receivingSupported: receivingSupported(),
		archive: { pending: false, bytes: 0, error: '' },
		sourceFiles: [],
		error: ''
	};
}

/** Owns one discovery identity, the recipient queue, consent and completed download URLs. */
export class FileTransferSession {
	private state = initialTransferState();
	private downloadStore: ReceivedBatchStore | null = null;
	private downloadOwner: string | null = null;
	private downloadAttempt = 0;
	private automaticPump: AutomaticDownloadPump | null = null;
	private automaticStopping: Promise<void> | null = null;
	private archiveController: AbortController | null = null;
	private archiveTask: Promise<void> | null = null;
	private archiveFile: { store: ReceivedBatchStore; count: number; file: File } | null = null;
	private urls = new Map<string, string>();
	private entries = new Map<string, Entry>();
	private batch: Batch | null = null;
	private generation = 0;
	private revision = 0;
	private controller = new AbortController();
	private source: EventSource | null = null;
	private startup: ReturnType<typeof setTimeout> | null = null;
	private disposed = false;
	private authEpoch = authStore.sessionEpoch;
	private stopAuth: () => void;
	private refreshing: { generation: number; promise: Promise<boolean> } | null = null;
	private refreshAgain = false;
	private pumping = false;
	private retiredPairs = new Set<string>();
	private closedPairs = new Set<string>();
	private retiredRequests = new Set<string>();
	private enabled = false;
	private label = 'อุปกรณ์ของฉัน';
	private manualRequired = false;
	private pendingDeviceId: string | null = null;
	private retryAttempt = 0;
	private retryTimer: ReturnType<typeof setTimeout> | null = null;
	private connecting: Promise<void> | null = null;
	private episodeRegistered = false;
	private authBlocked = false;
	private identityOwned = get(authStore).isAuthenticated;
	private listeners = false;
	private idle = createVisibilityIdle({
		...browserVisibilityDependencies(),
		onPause: () =>
			this.connectionLost('พักการเชื่อมต่อขณะซ่อนหน้า จะเชื่อมต่อใหม่เมื่อกลับมา', true),
		onResume: () => this.wake()
	});
	private wake = () => {
		if (
			!this.enabled ||
			this.disposed ||
			this.manualRequired ||
			this.idle.paused ||
			document.hidden ||
			!navigator.onLine ||
			this.state.status === 'online'
		)
			return;
		if (this.retryTimer !== null) clearTimeout(this.retryTimer);
		this.retryTimer = null;
		void this.connect(true);
	};
	constructor(private changed: (state: FileTransferState) => void) {
		this.stopAuth = authStore.subscribe((auth) => {
			const changedIdentity = this.authEpoch !== authStore.sessionEpoch;
			if (
				changedIdentity ||
				(!auth.isLoading && (!auth.isAuthenticated || auth.user?.user_type !== 'staff'))
			) {
				if (this.identityOwned) {
					this.enabled = false;
					this.manualRequired = true;
					this.idle.stop();
				}
				this.authEpoch = authStore.sessionEpoch;
				this.disconnect('การยืนยันตัวตนเปลี่ยนไป กรุณาเข้าสู่ระบบใหม่ก่อนรับส่งไฟล์', true);
				this.pendingDeviceId = null;
			}
			if (auth.isAuthenticated && !auth.isLoading) this.identityOwned = true;
			this.authBlocked =
				auth.isUnavailable || !auth.isAuthenticated || auth.user?.user_type !== 'staff';
			if (auth.isUnavailable && this.source)
				this.connectionLost('ยังยืนยันสิทธิ์ไม่ได้ กำลังเชื่อมต่อใหม่');
			if (this.authBlocked) {
				this.stopAutomaticDownloads();
				this.archiveController?.abort();
				this.publish({ authBlocked: true, automaticDownloadPending: false });
			} else {
				this.publish({ authBlocked: false });
				if (this.downloadStore && this.downloadOwner)
					this.requestAutomaticDownloads(this.downloadStore, this.downloadOwner);
			}
			if (
				this.enabled &&
				!auth.isLoading &&
				!this.authBlocked &&
				!this.connecting &&
				(this.state.status === 'idle' || changedIdentity)
			)
				void this.connect(false);
		});
	}
	private publish(patch: Partial<FileTransferState> = {}) {
		this.state = {
			...this.state,
			...patch,
			jobs: [...this.entries.values()].map((entry) => entry.view),
			sourceFiles: this.batch?.metadata ?? []
		};
		if (!this.disposed) this.changed(this.state);
	}
	private set(entry: Entry, patch: Partial<TransferJob>) {
		entry.view = { ...entry.view, ...patch };
		this.publish();
	}
	private current(generation: number, entry?: Entry, requestId?: string) {
		return (
			!this.disposed &&
			generation === this.generation &&
			!this.controller.signal.aborted &&
			(!entry ||
				(this.entries.get(entry.view.id) === entry &&
					(!requestId || requestId === entry.view.clientRequestId)))
		);
	}
	private retire(set: Set<string>, id: string) {
		set.add(id);
		if (set.size > 256) {
			const oldest = set.values().next().value;
			if (oldest) set.delete(oldest);
		}
	}
	private message(error: unknown, fallback: string) {
		return error instanceof Error && /^[ก-๛]/u.test(error.message) ? error.message : fallback;
	}
	private stopAutomaticDownloads() {
		const pump = this.automaticPump;
		if (!pump) return this.automaticStopping;
		this.automaticPump = null;
		const task = Promise.all([this.automaticStopping, pump.stop()]).then(() => {});
		this.automaticStopping = task;
		void task.then(() => {
			if (this.automaticStopping !== task) return;
			this.automaticStopping = null;
			if (!this.authBlocked && this.downloadStore && this.downloadOwner)
				this.requestAutomaticDownloads(this.downloadStore, this.downloadOwner);
		});
		return task;
	}
	private releaseDownloads() {
		this.downloadAttempt++;
		this.archiveController?.abort();
		for (const url of this.urls.values()) URL.revokeObjectURL(url);
		this.urls.clear();
		const store = this.downloadStore,
			archive = this.archiveTask,
			automatic = this.stopAutomaticDownloads();
		this.downloadStore = null;
		this.downloadOwner = null;
		this.archiveFile = null;
		this.state = {
			...this.state,
			downloads: [],
			downloadCount: 0,
			downloadBytes: 0,
			downloadPage: 0,
			downloadPending: false,
			downloadError: '',
			saveDestination: null,
			automaticDownloadRequests: 0,
			automaticDownloadPending: false,
			archive: { pending: false, bytes: 0, error: '' }
		};
		if (store)
			void (async () => {
				await archive;
				await automatic;
				await store.dispose();
			})().catch(() => console.warn('File transfer received storage cleanup failed'));
	}
	private pruneHistory(requiredSlots = 0, clearIncoming = false) {
		if (requiredSlots > MAX_TRANSFER_JOBS) return false;
		const downloadOwners = new Set(this.downloadOwner ? [this.downloadOwner] : []);
		const candidates = [...this.entries.values()].filter(
			(entry) =>
				isTransferTerminal(entry.view.phase) &&
				!entry.view.slotReserved &&
				!entry.view.pending &&
				!downloadOwners.has(entry.view.id)
		);
		// Incoming history has no retry action. Keep outgoing retries until capacity needs them.
		for (const direction of ['incoming', 'outgoing'] as const) {
			for (const entry of candidates) {
				if (entry.view.direction !== direction) continue;
				if (
					this.entries.size + requiredSlots <= MAX_TRANSFER_JOBS &&
					!(clearIncoming && direction === 'incoming')
				)
					break;
				if (entry.view.pair) this.retire(this.retiredPairs, entry.view.pair.id);
				this.retire(this.retiredRequests, entry.view.clientRequestId);
				entry.controller.abort();
				entry.transport?.dispose();
				this.entries.delete(entry.view.id);
			}
		}
		return this.entries.size + requiredSlots <= MAX_TRANSFER_JOBS;
	}
	clearDownloads() {
		const owner = this.downloadOwner ? this.entries.get(this.downloadOwner) : undefined;
		if (owner && !isTransferTerminal(owner.view.phase)) {
			this.publish({ downloadError: 'กรุณาสิ้นสุดการรับชุดนี้ก่อนล้างไฟล์ที่รับแล้ว' });
			return;
		}
		for (const entry of this.entries.values())
			if (entry.store === this.downloadStore) {
				entry.store = null;
				entry.manifestAttempt++;
			}
		this.releaseDownloads();
		this.pruneHistory(0, true);
		this.publish();
	}
	clearSource() {
		if (
			[...this.entries.values()].some(
				(entry) =>
					entry.view.direction === 'outgoing' &&
					(!isTransferTerminal(entry.view.phase) || entry.view.slotReserved)
			)
		)
			return;
		this.batch = null;
		this.publish();
	}
	private disconnect(message: string, clearFiles = false) {
		this.generation++;
		this.controller.abort();
		if (this.startup !== null) clearTimeout(this.startup);
		this.startup = null;
		this.source?.close();
		this.source = null;
		if (this.retryTimer !== null) clearTimeout(this.retryTimer);
		this.retryTimer = null;
		const device = this.state.snapshot?.device;
		for (const entry of this.entries.values()) {
			entry.controller.abort();
			entry.transport?.dispose();
			entry.transport = null;
			entry.consented = false;
			entry.intent = null;
			if (entry.view.pair) this.retire(this.retiredPairs, entry.view.pair.id);
			this.retire(this.retiredRequests, entry.view.clientRequestId);
			entry.view = {
				...entry.view,
				phase: isTransferTerminal(entry.view.phase) ? entry.view.phase : 'cancelled',
				pending: false,
				cleanupError: '',
				slotReserved: false
			};
		}
		if (clearFiles) this.batch = null;
		for (const entry of this.entries.values()) this.releaseEntryStore(entry);
		if (clearFiles) {
			this.state = { ...this.state, draftEpoch: this.state.draftEpoch + 1 };
			this.releaseDownloads();
			this.entries.clear();
		} else this.pruneHistory(0, true);
		this.publish({ status: message ? 'offline' : 'idle', snapshot: null, error: message });
		if (device) this.pendingDeviceId = device.id;
	}
	stop() {
		this.enabled = false;
		this.idle.stop();
		this.disconnect('');
		const id = this.pendingDeviceId;
		this.pendingDeviceId = null;
		if (id)
			void transferAPI
				.remove(id)
				.catch(() => console.warn('File transfer device cleanup could not be confirmed'));
	}
	dispose() {
		if (this.disposed) return;
		this.stop();
		this.disconnect('', true);
		this.disposed = true;
		for (const event of ['online', 'focus']) window.removeEventListener(event, this.wake);
		document.removeEventListener('visibilitychange', this.wake);
		this.stopAuth();
		this.entries.clear();
		this.batch = null;
	}
	async start(label = 'อุปกรณ์ของฉัน') {
		if (this.disposed) return;
		if (!label.trim() || [...label.trim()].length > 60 || /[\p{Cc}]/u.test(label)) {
			this.publish({ error: 'กรุณาตั้งชื่ออุปกรณ์ไม่เกิน 60 ตัวอักษร' });
			return;
		}
		this.label = label.trim();
		this.enabled = true;
		this.manualRequired = false;
		this.retryAttempt = 0;
		if (!this.listeners) {
			this.listeners = true;
			for (const event of ['online', 'focus']) window.addEventListener(event, this.wake);
			document.addEventListener('visibilitychange', this.wake);
		}
		this.idle.start();
		await this.connect(this.state.status !== 'idle');
	}
	async rename(label: string) {
		if ([...this.entries.values()].some((entry) => !isTransferTerminal(entry.view.phase))) return;
		if (label.trim() === this.label) return;
		this.disconnect('');
		await this.start(label);
	}
	private connectionLost(message: string, paused = false) {
		if (this.disposed || !this.enabled) return;
		const failedFresh = this.episodeRegistered;
		this.disconnect(message);
		if (paused) {
			this.publish({ status: 'paused' });
			return;
		}
		if (failedFresh) {
			this.manualRequired = true;
			this.publish({
				status: 'offline',
				error: 'เชื่อมต่ออุปกรณ์ใหม่ไม่สำเร็จ กดลองเชื่อมต่ออีกครั้ง ไฟล์ที่เลือกยังอยู่'
			});
			return;
		}
		this.publish({ status: 'reconnecting' });
		this.scheduleRecovery();
	}
	private scheduleRecovery() {
		if (
			!this.enabled ||
			this.disposed ||
			this.manualRequired ||
			this.idle.paused ||
			document.hidden ||
			!navigator.onLine ||
			this.retryTimer !== null
		)
			return;
		const delay = recoveryDelay(this.retryAttempt++, Math.random());
		if (delay === null) {
			this.manualRequired = true;
			this.publish({
				status: 'offline',
				error: 'เชื่อมต่อใหม่ยังไม่สำเร็จ ไฟล์ที่เลือกยังอยู่ กดลองเชื่อมต่ออีกครั้งเมื่อพร้อม'
			});
			return;
		}
		this.retryTimer = setTimeout(() => {
			this.retryTimer = null;
			void this.connect(true);
		}, delay);
	}
	private async connect(reconcile: boolean): Promise<void> {
		if (this.connecting) return this.connecting;
		if (
			!this.enabled ||
			this.disposed ||
			this.manualRequired ||
			this.idle.paused ||
			document.hidden ||
			!navigator.onLine
		)
			return;
		const auth = get(authStore);
		if (auth.isLoading || !auth.isAuthenticated || auth.user?.user_type !== 'staff') return;
		this.controller = new AbortController();
		const generation = this.generation,
			epoch = this.authEpoch;
		const valid = () => this.current(generation) && this.enabled && this.authEpoch === epoch;
		const task = (async () => {
			this.publish({ status: reconcile ? 'reconnecting' : 'connecting', error: '' });
			try {
				if (reconcile || auth.isUnavailable) {
					const result = await authAPI.refreshCurrentUser({ silent: true, invalidate: true });
					if (!valid()) return;
					if (result !== 'authenticated') {
						if (result === 'unavailable') this.scheduleRecovery();
						return;
					}
				}
				if (!valid() || this.authBlocked) return;
				if (this.pendingDeviceId) {
					const id = this.pendingDeviceId;
					try {
						await transferAPI.snapshot(id, this.controller.signal);
						if (!valid()) return;
						await transferAPI.remove(id);
					} catch (error) {
						if (!(error instanceof ApiClientError && error.status === 404)) throw error;
					}
					if (!valid()) return;
					this.pendingDeviceId = null;
				}
			} catch {
				if (valid()) this.scheduleRecovery();
				return;
			}
			if (!valid()) return;
			if (typeof RTCPeerConnection === 'undefined' || typeof EventSource === 'undefined') {
				this.manualRequired = true;
				this.publish({
					status: 'offline',
					error: 'เบราว์เซอร์นี้ไม่รองรับการส่งไฟล์โดยตรง กรุณาใช้เบราว์เซอร์รุ่นปัจจุบัน'
				});
				return;
			}
			let snapshot: TransferSnapshot;
			try {
				snapshot = await transferAPI.register(this.label, this.controller.signal);
			} catch (error) {
				if (valid()) {
					this.manualRequired = true;
					this.publish({
						status: 'offline',
						error: this.message(
							error,
							'ยังยืนยันผลการเปิดอุปกรณ์ไม่ได้ ระบบจะไม่ส่งคำขอซ้ำอัตโนมัติ ไฟล์ที่เลือกยังอยู่ กดลองเชื่อมต่ออีกครั้ง'
						)
					});
				}
				return;
			}
			if (!valid()) {
				if (this.authEpoch === epoch)
					void transferAPI
						.remove(snapshot.device.id)
						.catch(() => console.warn('Late file transfer device cleanup failed'));
				return;
			}
			this.episodeRegistered = true;
			this.publish({ snapshot });
			const source = new EventSource(transferAPI.eventsUrl(snapshot.device.id), {
				withCredentials: true
			});
			this.source = source;
			this.startup = setTimeout(() => {
				if (valid()) this.connectionLost('เชื่อมต่ออุปกรณ์นานเกินไป');
			}, 30000);
			source.addEventListener('file_transfer', (event: MessageEvent<string>) => {
				if (!valid() || this.source !== source) return;
				try {
					this.event(parseTransferEvent(event.data), generation);
				} catch {
					this.manualRequired = true;
					this.disconnect('ข้อมูลการเชื่อมต่อไม่ถูกต้อง กรุณารีเฟรชหน้าเว็บทั้งสองอุปกรณ์');
				}
			});
			source.onerror = () => {
				if (valid() && this.source === source)
					this.connectionLost('ขาดการเชื่อมต่อชั่วคราว กำลังเชื่อมต่อใหม่ ไฟล์ที่เลือกยังอยู่');
			};
			void collectAbandonedTransferStorage().catch(() =>
				console.warn('File transfer stale storage cleanup failed')
			);
		})();
		this.connecting = task;
		try {
			await task;
		} finally {
			if (this.connecting === task) this.connecting = null;
		}
	}
	private event(event: ReturnType<typeof parseTransferEvent>, generation: number) {
		switch (event.type) {
			case 'ready':
				this.episodeRegistered = false;
				this.retryAttempt = 0;
				void this.refresh(generation);
				if (this.startup !== null) clearTimeout(this.startup);
				this.startup = null;
				this.snapshot(event.data);
				this.publish({ status: 'online', error: '' });
				this.pump();
				break;
			case 'presence_changed':
				void this.refresh(generation);
				break;
			case 'pair_changed':
				this.applyPair(event.data);
				break;
			case 'pair_closed':
				this.pairClosed(event.data.pairId, event.data.reason);
				break;
			case 'signal': {
				const entry = [...this.entries.values()].find(
					(item) => item.view.pair?.id === event.data.pairId
				);
				if (!entry || isTransferTerminal(entry.view.phase) || !entry.view.slotReserved) return;
				if (
					!entry.transport &&
					entry.view.direction === 'incoming' &&
					entry.consented &&
					entry.view.pair?.status === 'accepted'
				)
					this.createTransport(entry);
				if (entry.transport) void entry.transport.signal(event.data);
				break;
			}
			case 'session_invalid':
				this.manualRequired = true;
				this.disconnect('ไม่สามารถยืนยันสิทธิ์ได้ กรุณาเข้าสู่ระบบใหม่ก่อนรับส่งไฟล์', true);
				void authAPI.refreshCurrentUser({ silent: true, invalidate: true });
				break;
			case 'session_unavailable':
				this.connectionLost('ยังยืนยันสิทธิ์ไม่ได้ กำลังตรวจสอบใหม่');
				break;
		}
	}
	private snapshot(snapshot: TransferSnapshot) {
		if (snapshot.protocolVersion !== 3 || snapshot.device.id !== this.state.snapshot?.device.id)
			throw new Error('incompatible_snapshot');
		this.publish({ snapshot });
		for (const pair of snapshot.pairs) this.applyPair(pair);
	}
	private async refresh(generation = this.generation): Promise<boolean> {
		if (this.refreshing?.generation === generation) {
			this.refreshAgain = true;
			return this.refreshing.promise;
		}
		const device = this.state.snapshot?.device;
		if (!device || !this.current(generation)) return false;
		const promise = (async () => {
			try {
				do {
					this.refreshAgain = false;
					const revision = this.revision;
					const snapshot = await transferAPI.snapshot(device.id, this.controller.signal);
					if (!this.current(generation)) return false;
					const reconcileMissing = revision === this.revision;
					this.snapshot(snapshot);
					if (reconcileMissing)
						for (const entry of this.entries.values())
							if (
								entry.view.slotReserved &&
								entry.view.pair &&
								!snapshot.pairs.some((pair) => pair.id === entry.view.pair?.id)
							)
								this.pairClosed(entry.view.pair.id, 'disconnected');
				} while (this.refreshAgain && this.current(generation));
				return true;
			} catch {
				if (this.current(generation))
					this.publish({
						error: 'โหลดสถานะอุปกรณ์ไม่สำเร็จ การเชื่อมต่อที่ยังเปิดอยู่จะแสดงสถานะต่อไป'
					});
				return false;
			}
		})();
		this.refreshing = { generation, promise };
		try {
			return await promise;
		} finally {
			if (this.refreshing?.promise === promise) this.refreshing = null;
		}
	}
	private applyPair(pair: TransferPair) {
		const device = this.state.snapshot?.device;
		if (!device || (pair.initiator.id !== device.id && pair.recipient.id !== device.id))
			throw new Error('wrong_device');
		if (this.closedPairs.has(pair.id)) {
			const known = [...this.entries.values()].find(
				(item) =>
					item.view.direction === 'outgoing' && item.view.clientRequestId === pair.clientRequestId
			);
			if (
				known?.view.slotReserved &&
				!known.view.pair &&
				pair.initiator.id === device.id &&
				sameBatch(known.view.batch, pair.batch)
			) {
				known.view = { ...known.view, pair };
				this.pairClosed(pair.id, 'cancelled');
			}
			return;
		}
		if (this.retiredPairs.has(pair.id)) return;
		if (validateBatchSummary(pair.batch)) throw new Error('invalid_batch');
		const outgoing = pair.initiator.id === device.id;
		let entry = [...this.entries.values()].find((item) =>
			outgoing
				? item.view.clientRequestId === pair.clientRequestId && item.view.direction === 'outgoing'
				: item.view.pair?.id === pair.id
		);
		if (
			outgoing &&
			(!entry ||
				this.retiredRequests.has(pair.clientRequestId) ||
				isTransferTerminal(entry.view.phase))
		) {
			if (
				entry &&
				sameBatch(entry.view.batch, pair.batch) &&
				(!entry.view.pair || entry.view.pair.id === pair.id)
			) {
				// A cancelled/uncertain create may commit after its response is lost. Bind its
				// correlation to the terminal job for confirmed cleanup, never to restart it.
				this.revision++;
				this.set(entry, {
					pair,
					batch: pair.batch,
					recipientId: pair.recipient.id,
					slotReserved: true
				});
				void this.close(entry);
			} else this.closeOrphan(pair);
			return;
		}
		if (!entry) {
			if (
				[...this.entries.values()].some(
					(item) => item.view.direction === 'outgoing' && !isTransferTerminal(item.view.phase)
				)
			) {
				this.closeOrphan(pair);
				return;
			}
			if (!this.pruneHistory(1)) {
				this.closeOrphan(pair);
				this.publish({
					error: 'รายการรับส่งเต็ม กรุณาสิ้นสุดคำขอหรือล้างไฟล์ที่รับแล้วก่อนรับชุดใหม่'
				});
				return;
			}
			entry = this.entry(
				'incoming',
				[],
				pair.clientRequestId,
				'',
				null,
				`${pair.initiator.displayName} · ${pair.initiator.label}`
			);
			this.entries.set(entry.view.id, entry);
			entry.view.pair = pair;
			entry.view.batch = pair.batch;
			entry.view.recipientId = device.id;
			entry.view.slotReserved = true;
		}
		const previous = entry.view.pair;
		if (
			!sameBatch(entry.view.batch, pair.batch) ||
			(previous &&
				(previous.id !== pair.id ||
					previous.initiator.id !== pair.initiator.id ||
					previous.recipient.id !== pair.recipient.id ||
					previous.clientRequestId !== pair.clientRequestId))
		)
			throw new Error('changed_batch_identity');
		if (previous?.status === 'accepted' && pair.status === 'pending') return;
		if (
			outgoing &&
			[...this.entries.values()].some(
				(item) =>
					item !== entry &&
					item.batchId === entry.batchId &&
					item.view.direction === 'outgoing' &&
					item.view.recipientId === pair.recipient.id
			)
		) {
			this.fail(entry, 'อุปกรณ์นี้อยู่ในรายชื่อผู้รับชุดนี้แล้ว', pair);
			return;
		}
		this.revision++;
		const phase =
			pair.status === 'pending'
				? outgoing
					? 'waiting'
					: entry.consented
						? 'connecting'
						: 'consent'
				: ['requesting', 'waiting', 'consent'].includes(entry.view.phase)
					? 'connecting'
					: entry.view.phase;
		this.set(entry, {
			pair,
			batch: pair.batch,
			phase,
			recipientId: pair.recipient.id,
			label: outgoing
				? `${pair.recipient.displayName} · ${pair.recipient.label}`
				: entry.view.label,
			slotReserved: true,
			error:
				pair.status === 'accepted' &&
				['requesting', 'waiting', 'consent', 'connecting'].includes(entry.view.phase)
					? ''
					: entry.view.error
		});
		if (pair.status === 'accepted' && outgoing && !entry.transport) {
			const transport = this.createTransport(entry);
			void transport?.offer();
		}
	}
	private entry(
		direction: TransferJob['direction'],
		files: FileMetadata[],
		clientRequestId: string,
		batchId: string,
		target: TransferTarget | null,
		label: string
	): Entry {
		return {
			view: {
				id: crypto.randomUUID(),
				clientRequestId,
				direction,
				label,
				recipientId: target?.type === 'device' ? target.deviceId : null,
				pair: null,
				files,
				batch: batchSummary(files),
				manifestPage: 0,
				manifestPending: false,
				storageWarning: '',
				receiveMode: directoryPickerSupported() ? 'folder' : 'browser-download',
				destinationLabel: '',
				megabytesPerSecond: 0,
				phase: direction === 'incoming' ? 'consent' : 'queued',
				bytes: 0,
				fileIndex: 0,
				completedFiles: 0,
				error: '',
				cleanupError: '',
				pending: false,
				slotReserved: false
			},
			batchId,
			target,
			controller: new AbortController(),
			transport: null,
			consented: false,
			closingPairId: null,
			store: null,
			manifestAttempt: 0,
			consentOperation: 0,
			intent: null
		};
	}
	send(files: readonly File[], targets: readonly TransferTarget[]) {
		const snapshot = this.state.snapshot;
		if (this.state.status !== 'online' || !snapshot || !targets.length) return;
		const metadata = files.map(fileMetadata),
			error = validateTransferBatch(metadata, snapshot.limits);
		if (error) {
			this.publish({ error });
			return;
		}
		if (
			[...this.entries.values()].some(
				(item) => item.view.direction === 'incoming' && item.view.slotReserved
			)
		) {
			this.publish({ error: 'กรุณาสิ้นสุดคำขอรับชุดไฟล์ก่อนส่งชุดใหม่' });
			return;
		}
		const active = [...this.entries.values()].some(
			(item) =>
				item.view.direction === 'outgoing' &&
				(!isTransferTerminal(item.view.phase) || item.view.slotReserved)
		);
		if (
			active &&
			(!this.batch ||
				!this.batch.files.every((file, index) => file === files[index]) ||
				this.batch.files.length !== files.length)
		) {
			this.publish({ error: 'รอให้ผู้รับทุกอุปกรณ์สิ้นสุดก่อนเปลี่ยนชุดไฟล์' });
			return;
		}
		if (
			!this.batch ||
			!this.batch.files.every((file, index) => file === files[index]) ||
			this.batch.files.length !== files.length
		) {
			this.batch = { id: crypto.randomUUID(), files: [...files], metadata };
		}
		const batch = this.batch;
		const requested = new Set<string>();
		const additions = targets.filter((target) => {
			if (target.type === 'device' && target.deviceId === snapshot.device.id) return false;
			const key = target.type === 'device' ? `device:${target.deviceId}` : `code:${target.code}`;
			if (requested.has(key)) return false;
			requested.add(key);
			return ![...this.entries.values()].some(
				(item) =>
					item.batchId === batch.id &&
					item.target?.type === target.type &&
					(target.type === 'device'
						? item.view.recipientId === target.deviceId
						: item.target.type === 'code' && item.target.code === target.code)
			);
		});
		if (!this.pruneHistory(additions.length)) {
			this.publish({ error: 'รายชื่อผู้รับเต็ม กรุณาสิ้นสุดชุดเดิมและล้างรายชื่อก่อนเพิ่มผู้รับ' });
			return;
		}
		for (const target of additions) {
			const peer =
				target.type === 'device'
					? snapshot.peers.find((item) => item.id === target.deviceId)
					: undefined;
			const entry = this.entry(
				'outgoing',
				batch.metadata,
				crypto.randomUUID(),
				batch.id,
				target,
				peer
					? `${peer.displayName} · ${peer.label}`
					: target.type === 'code'
						? `รหัส ${target.code}`
						: 'อุปกรณ์ผู้รับ'
			);
			this.entries.set(entry.view.id, entry);
		}
		this.publish({ error: '' });
		this.pump();
	}
	private pump() {
		const snapshot = this.state.snapshot;
		if (this.pumping || this.state.status !== 'online' || !snapshot) return;
		if (
			[...this.entries.values()].some(
				(entry) => entry.view.direction === 'incoming' && entry.view.slotReserved
			)
		)
			return;
		this.pumping = true;
		try {
			let slots = [...this.entries.values()].filter(
				(entry) => entry.view.direction === 'outgoing' && entry.view.slotReserved
			).length;
			for (const entry of this.entries.values()) {
				if (slots >= snapshot.limits.maxOutgoingTransfers) break;
				if (entry.view.direction !== 'outgoing' || entry.view.phase !== 'queued') continue;
				entry.view = { ...entry.view, phase: 'requesting', pending: true, slotReserved: true };
				slots++;
				this.revision++;
				void this.request(entry, this.generation);
			}
			this.publish();
		} finally {
			this.pumping = false;
		}
	}
	private async request(entry: Entry, generation: number) {
		const device = this.state.snapshot?.device,
			target = entry.target,
			requestId = entry.view.clientRequestId;
		if (!device || !target) return;
		try {
			const pair = await transferAPI.request(
				device.id,
				target,
				entry.view.batch,
				requestId,
				entry.controller.signal
			);
			if (this.current(generation, entry, requestId)) this.applyPair(pair);
			else if (
				this.current(generation) &&
				pair.initiator.id === device.id &&
				!this.retiredPairs.has(pair.id)
			)
				this.closeOrphan(pair);
		} catch (error) {
			if (!this.current(generation, entry, requestId)) return;
			if (
				error instanceof ApiClientError &&
				error.status >= 400 &&
				error.status < 500 &&
				error.status !== 409 &&
				!entry.view.pair
			) {
				this.retire(this.retiredRequests, requestId);
				this.set(entry, {
					phase: entry.view.phase === 'cancelled' ? 'cancelled' : 'failed',
					slotReserved: false,
					error: this.message(error, 'ส่งคำขอไม่สำเร็จ ตรวจสอบผู้รับหรือรหัสแล้วลองใหม่')
				});
				this.pump();
				return;
			}
			const reconciled = await this.refresh(generation);
			if (!this.current(generation, entry, requestId)) return;
			if (
				!entry.view.pair &&
				error instanceof ApiClientError &&
				error.status === 409 &&
				reconciled
			) {
				this.retire(this.retiredRequests, requestId);
				this.set(entry, {
					phase: entry.view.phase === 'cancelled' ? 'cancelled' : 'failed',
					slotReserved: false,
					cleanupError: '',
					error: this.message(error, 'อุปกรณ์ผู้รับไม่ว่าง กรุณาส่งคำขอใหม่เมื่อพร้อม')
				});
				this.pump();
				return;
			}
			if (!entry.view.pair) {
				this.retire(this.retiredRequests, requestId);
				// A negative snapshot can race a still-running create. Keep admission
				// reserved until its matching pair is closed or the device is restarted.
				this.set(entry, {
					phase: entry.view.phase === 'cancelled' ? 'cancelled' : 'failed',
					slotReserved: true,
					cleanupError:
						'ยังยืนยันผลคำขอไม่ได้ ช่องส่งนี้ยังถูกจองไว้ กรุณาปิดรับส่งไฟล์ทั้งหมดแล้วเปิดใหม่หากคำขอไม่สิ้นสุด'
				});
			}
		} finally {
			if (this.current(generation, entry, requestId)) this.set(entry, { pending: false });
		}
	}
	async acceptManifest(id: string, receiveOperation?: number) {
		const entry = this.entries.get(id),
			device = this.state.snapshot?.device;
		if (
			!entry ||
			!device ||
			entry.view.direction !== 'incoming' ||
			!entry.view.pair ||
			entry.view.phase !== 'consent' ||
			(entry.view.pending && receiveOperation === undefined) ||
			this.authBlocked ||
			this.state.status !== 'online'
		)
			return;
		if (this.state.downloadCount) {
			this.set(entry, { error: 'กรุณาดาวน์โหลดและล้างไฟล์ชุดก่อนก่อนรับชุดใหม่นี้' });
			return;
		}
		if (!receivingSupported()) {
			this.set(entry, { error: RECEIVING_UNSUPPORTED });
			return;
		}
		entry.consented = true;
		const operation = receiveOperation ?? ++entry.consentOperation;
		const generation = this.generation,
			requestId = entry.view.clientRequestId;
		this.set(entry, { phase: 'connecting', pending: true, error: '' });
		try {
			const store = await ReceivedBatchStore.open(entry.view.pair.batch);
			if (
				!this.current(generation, entry, requestId) ||
				!entry.consented ||
				isTransferTerminal(entry.view.phase)
			) {
				await store.dispose();
				return;
			}
			entry.store = store;
			this.createTransport(entry);
			if (!entry.transport || isTransferTerminal(entry.view.phase)) return;
			const pair = await transferAPI.accept(entry.view.pair.id, device.id, entry.controller.signal);
			if (
				this.current(generation, entry, requestId) &&
				entry.consentOperation === operation &&
				!isTransferTerminal(entry.view.phase)
			)
				this.applyPair(pair);
		} catch (error) {
			if (
				this.current(generation, entry, requestId) &&
				entry.consented &&
				entry.consentOperation === operation &&
				!isTransferTerminal(entry.view.phase)
			) {
				if (!entry.store) {
					this.fail(entry, storageMessage(error));
					return;
				}
				this.set(entry, {
					error: this.message(error, 'ยังยืนยันการตอบรับไม่ได้ กำลังรอข้อมูลจากระบบ')
				});
				await this.refresh(generation);
			}
		} finally {
			if (
				this.current(generation, entry, requestId) &&
				entry.consentOperation === operation &&
				!entry.intent
			)
				this.set(entry, { pending: false });
		}
	}
	private releaseEntryStore(entry: Entry) {
		entry.manifestAttempt++;
		entry.consentOperation++;
		entry.intent = null;
		const store = entry.store;
		if (!store || store === this.downloadStore) return;
		entry.store = null;
		const generation = this.generation,
			requestId = entry.view.clientRequestId;
		void store.dispose().catch(() => {
			if (this.current(generation, entry, requestId))
				this.set(entry, {
					storageWarning:
						'ยังยืนยันการล้างไฟล์บางส่วนไม่ได้ กรุณาตรวจสอบโฟลเดอร์ปลายทางและลบไฟล์ที่ไม่ต้องการเอง ไฟล์ที่บันทึกครบแล้วคงอยู่'
				});
		});
	}
	async showManifestPage(id: string, page: number) {
		const entry = this.entries.get(id),
			generation = this.generation;
		if (
			!entry?.store ||
			!Number.isInteger(page) ||
			page < 0 ||
			page * TRANSFER_PAGE_SIZE >= entry.view.batch.fileCount
		)
			return;
		const store = entry.store,
			requestId = entry.view.clientRequestId,
			attempt = ++entry.manifestAttempt;
		this.set(entry, { manifestPending: true, error: '' });
		try {
			const files = await store.page(page * TRANSFER_PAGE_SIZE);
			if (
				this.current(generation, entry, requestId) &&
				entry.store === store &&
				entry.manifestAttempt === attempt
			)
				this.set(entry, { files, manifestPage: page });
		} catch (error) {
			if (this.current(generation, entry, requestId) && entry.manifestAttempt === attempt)
				this.set(entry, { error: storageMessage(error) });
		} finally {
			if (this.current(generation, entry, requestId) && entry.manifestAttempt === attempt)
				this.set(entry, { manifestPending: false });
		}
	}
	setReceiveMode(id: string, mode: ReceiveMode) {
		const entry = this.entries.get(id);
		if (
			!entry ||
			entry.view.direction !== 'incoming' ||
			!['consent', 'review'].includes(entry.view.phase) ||
			entry.view.pending ||
			(mode === 'folder' && !directoryPickerSupported())
		)
			return;
		this.set(entry, { receiveMode: mode, error: '', storageWarning: '' });
	}
	async receive(id: string) {
		const entry = this.entries.get(id),
			device = this.state.snapshot?.device;
		if (
			!entry ||
			!device ||
			this.authBlocked ||
			this.state.status !== 'online' ||
			entry.view.direction !== 'incoming' ||
			!entry.view.pair ||
			!['consent', 'review'].includes(entry.view.phase) ||
			entry.view.pending
		)
			return;
		if (this.state.downloadCount) {
			this.set(entry, { error: 'กรุณาดาวน์โหลดและล้างไฟล์ชุดก่อนก่อนรับชุดใหม่นี้' });
			return;
		}
		if (!receivingSupported()) {
			this.set(entry, { error: RECEIVING_UNSUPPORTED });
			return;
		}
		const operation = ++entry.consentOperation,
			generation = this.generation,
			epoch = this.authEpoch,
			pair = entry.view.pair,
			mode = entry.view.receiveMode;
		const valid = () =>
			this.current(generation, entry, pair.clientRequestId) &&
			entry.consentOperation === operation &&
			this.authEpoch === epoch &&
			!this.authBlocked &&
			this.state.status === 'online' &&
			entry.view.pair?.id === pair.id &&
			sameBatch(entry.view.batch, pair.batch) &&
			!isTransferTerminal(entry.view.phase);
		this.set(entry, { pending: true, error: '' });
		try {
			const selected = mode === 'folder' ? pickReceiveDirectory() : null;
			const directory = selected ? await selected : null;
			if (!valid()) return;
			entry.intent = {
				operation,
				generation,
				authEpoch: epoch,
				deviceId: device.id,
				pairId: pair.id,
				requestId: pair.clientRequestId,
				batch: { ...pair.batch },
				destination: directory ? { mode: 'folder', directory } : { mode: 'browser-download' }
			};
			if (entry.view.phase === 'review') await this.continueReceive(entry);
			else await this.acceptManifest(id, operation);
		} catch (error) {
			if (valid()) {
				entry.intent = null;
				this.set(entry, {
					pending: false,
					error: mode === 'folder' ? destinationSelectionMessage(error) : storageMessage(error)
				});
			}
		} finally {
			if (valid() && !entry.intent) this.set(entry, { pending: false });
		}
	}
	async acceptFiles(id: string) {
		await this.receive(id);
	}
	private async continueReceive(entry: Entry) {
		const intent = entry.intent,
			store = entry.store;
		if (!intent || !store) return;
		const operation = ++entry.consentOperation;
		intent.operation = operation;
		const valid = () =>
			this.current(intent.generation, entry, intent.requestId) &&
			entry.intent === intent &&
			entry.consentOperation === operation &&
			this.authEpoch === intent.authEpoch &&
			!this.authBlocked &&
			this.state.status === 'online' &&
			this.state.snapshot?.device.id === intent.deviceId &&
			entry.view.pair?.id === intent.pairId &&
			sameBatch(entry.view.batch, intent.batch) &&
			entry.view.phase === 'review';
		if (!valid()) return;
		this.set(entry, { pending: true, error: '' });
		try {
			await store.configureDestination(intent.destination, entry.controller.signal);
			if (!valid()) return;
			const warning =
				intent.destination.mode === 'browser-download'
					? await checkStorageSpace(intent.batch.totalBytes)
					: null;
			if (!valid()) return;
			this.set(entry, { storageWarning: warning ?? '', destinationLabel: store.destinationLabel });
			entry.transport?.acceptFiles();
			entry.intent = null;
		} catch (error) {
			if (valid()) {
				entry.intent = null;
				this.set(entry, { error: storageMessage(error) });
			}
		} finally {
			if (
				this.current(intent.generation, entry, intent.requestId) &&
				entry.consentOperation === operation
			)
				this.set(entry, { pending: false });
		}
	}
	private requestAutomaticDownloads(store: ReceivedBatchStore, owner: string) {
		if (store.destinationMode !== 'browser-download' || this.automaticStopping || this.authBlocked)
			return;
		const valid = () =>
			!this.disposed &&
			!this.authBlocked &&
			this.downloadStore === store &&
			this.downloadOwner === owner;
		if (!this.automaticPump)
			this.automaticPump = new AutomaticDownloadPump(
				{
					read: async (index) => {
						const file = await store.getCompletedFile(index);
						return { file, name: file.name };
					},
					request: (index, download) => {
						if (!valid()) throw new Error('auth_unavailable');
						this.saveDownload(String(index), download.file, download.name);
					},
					progress: (count) => {
						if (valid())
							this.publish({
								automaticDownloadRequests: count,
								automaticDownloadPending: count < store.completedCount
							});
					},
					failure: () => {
						if (valid())
							this.publish({
								automaticDownloadPending: false,
								downloadError:
									'เบราว์เซอร์ไม่ยอมเริ่มดาวน์โหลดอัตโนมัติ ไฟล์ที่รับครบยังอยู่ กดดาวน์โหลดรายไฟล์หรือ ZIP ได้'
							});
					}
				},
				this.state.automaticDownloadRequests
			);
		this.publish({
			automaticDownloadPending: this.state.automaticDownloadRequests < store.completedCount
		});
		this.automaticPump.advance(store.completedCount);
	}
	async showDownloadsPage(page: number) {
		const store = this.downloadStore,
			owner = this.downloadOwner;
		if (
			!store ||
			!owner ||
			!Number.isInteger(page) ||
			page < 0 ||
			page * TRANSFER_PAGE_SIZE >= store.completedCount
		)
			return;
		const attempt = ++this.downloadAttempt;
		this.publish({ downloadPending: true, downloadError: '' });
		try {
			const files = await store.page(page * TRANSFER_PAGE_SIZE, TRANSFER_PAGE_SIZE, true);
			if (
				!this.disposed &&
				this.downloadStore === store &&
				this.downloadOwner === owner &&
				attempt === this.downloadAttempt
			)
				this.publish({
					downloads: files.map((file: StoredMetadata) => ({
						id: `${owner}:${file.index}`,
						jobId: owner,
						...file
					})),
					downloadPage: page
				});
		} catch (error) {
			if (!this.disposed && this.downloadStore === store && attempt === this.downloadAttempt)
				this.publish({ downloadError: storageMessage(error) });
		} finally {
			if (!this.disposed && this.downloadStore === store && attempt === this.downloadAttempt)
				this.publish({ downloadPending: false });
		}
	}
	private saveDownload(key: string, file: File, name: string) {
		const previous = this.urls.get(key);
		if (previous) URL.revokeObjectURL(previous);
		// Keep at most one URL per visible page plus the archive.
		if (this.urls.size >= TRANSFER_PAGE_SIZE + 1) {
			const oldest = this.urls.keys().next().value;
			if (oldest !== undefined) {
				URL.revokeObjectURL(this.urls.get(oldest)!);
				this.urls.delete(oldest);
			}
		}
		const url = URL.createObjectURL(file);
		this.urls.set(key, url);
		const anchor = document.createElement('a');
		anchor.href = url;
		anchor.download = name;
		anchor.click();
	}
	async downloadFile(index: number) {
		const store = this.downloadStore,
			owner = this.downloadOwner;
		if (!store || !owner || this.authBlocked) return;
		try {
			const metadata = await store.metadata(index),
				file = await store.getCompletedFile(index);
			if (
				!this.disposed &&
				!this.authBlocked &&
				this.downloadStore === store &&
				this.downloadOwner === owner
			)
				this.saveDownload(String(index), file, metadata.name);
		} catch (error) {
			if (!this.disposed && this.downloadStore === store)
				this.publish({ downloadError: storageMessage(error) });
		}
	}
	downloadAll() {
		const store = this.downloadStore,
			owner = this.downloadOwner;
		if (!store || !owner || this.authBlocked || !store.completedCount || this.state.archive.pending)
			return;
		const job = this.entries.get(owner);
		if (job && !isTransferTerminal(job.view.phase)) {
			this.publish({ downloadError: 'กรุณาสิ้นสุดการรับชุดนี้ก่อนดาวน์โหลดรวมเป็น ZIP' });
			return;
		}
		if (this.archiveFile?.store === store && this.archiveFile.count === store.completedCount) {
			this.saveDownload('archive', this.archiveFile.file, 'SchoolOrbit-files.zip');
			return;
		}
		const completedCount = store.completedCount,
			completedBytes = store.completedBytes;
		const controller = new AbortController();
		this.archiveController = controller;
		const valid = () =>
			!this.disposed &&
			this.downloadStore === store &&
			this.downloadOwner === owner &&
			this.archiveController === controller &&
			!controller.signal.aborted;
		this.publish({ archive: { pending: true, bytes: 0, error: '' } });
		this.archiveTask = (async () => {
			try {
				const file = await createTransferArchive(store, controller.signal, (bytes) => {
					if (valid()) this.publish({ archive: { pending: true, bytes, error: '' } });
				});
				if (valid()) {
					this.archiveFile = { store, count: completedCount, file };
					this.saveDownload('archive', file, 'SchoolOrbit-files.zip');
					this.publish({ archive: { pending: false, bytes: completedBytes, error: '' } });
				}
			} catch (error) {
				if (!this.disposed && this.downloadStore === store && this.archiveController === controller)
					this.publish({
						archive: {
							pending: false,
							bytes: 0,
							error: controller.signal.aborted
								? 'ยกเลิกการสร้าง ZIP แล้ว ไฟล์ที่รับครบยังดาวน์โหลดได้'
								: storageMessage(error)
						}
					});
			} finally {
				if (this.archiveController === controller) {
					this.archiveController = null;
					this.archiveTask = null;
				}
			}
		})();
	}
	cancelArchive() {
		this.archiveController?.abort();
	}
	cancel(id: string) {
		const entry = this.entries.get(id);
		if (!entry) return;
		const unknownCreate = entry.view.slotReserved && !entry.view.pair;
		this.retire(this.retiredRequests, entry.view.clientRequestId);
		entry.consented = false;
		entry.intent = null;
		// Preserve the metadata request's response while its resource is unknown;
		// its eventual pair is needed for cleanup. File pumps always stop now.
		if (!unknownCreate) entry.controller.abort();
		entry.transport?.dispose();
		entry.transport = null;
		this.releaseEntryStore(entry);
		this.set(entry, { phase: 'cancelled', pending: false, error: '' });
		if (entry.view.slotReserved) void this.close(entry);
		else this.pump();
	}

	async retry(id: string, selectedFiles: readonly File[]) {
		const entry = this.entries.get(id),
			snapshot = this.state.snapshot;
		const files = [...selectedFiles],
			metadata = files.map(fileMetadata);
		if (
			!entry ||
			!snapshot ||
			entry.view.direction !== 'outgoing' ||
			!isTransferTerminal(entry.view.phase) ||
			validateTransferBatch(metadata, snapshot.limits) ||
			!sameFiles(entry.view.files, metadata)
		) {
			this.publish({ error: 'เลือกชุดไฟล์เดิมก่อนส่งคำขอใหม่ให้ผู้รับนี้' });
			return;
		}
		const generation = this.generation,
			requestId = entry.view.clientRequestId,
			deviceId = snapshot.device.id;
		if (entry.view.slotReserved && !(await this.close(entry))) return;
		if (
			!this.current(generation, entry, requestId) ||
			this.state.status !== 'online' ||
			this.state.snapshot?.device.id !== deviceId ||
			!isTransferTerminal(entry.view.phase) ||
			[...this.entries.values()].some(
				(item) => item.view.direction === 'incoming' && item.view.slotReserved
			)
		)
			return;
		const active = [...this.entries.values()].some(
			(item) =>
				item.view.direction === 'outgoing' &&
				(!isTransferTerminal(item.view.phase) || item.view.slotReserved)
		);
		if (
			active &&
			(!this.batch ||
				this.batch.files.length !== files.length ||
				!this.batch.files.every((file, index) => file === files[index]))
		) {
			this.publish({ error: 'รอให้ผู้รับทุกอุปกรณ์สิ้นสุดก่อนเปลี่ยนชุดไฟล์' });
			return;
		}
		if (
			!this.batch ||
			this.batch.files.length !== files.length ||
			!this.batch.files.every((file, index) => file === files[index])
		)
			this.batch = { id: crypto.randomUUID(), files, metadata };
		this.retire(this.retiredRequests, entry.view.clientRequestId);
		entry.controller = new AbortController();
		entry.batchId = this.batch.id;
		this.set(entry, {
			clientRequestId: crypto.randomUUID(),
			pair: null,
			phase: 'queued',
			bytes: 0,
			megabytesPerSecond: 0,
			fileIndex: 0,
			completedFiles: 0,
			pending: false,
			error: '',
			cleanupError: '',
			slotReserved: false
		});
		this.pump();
	}
	private createTransport(entry: Entry): FileTransferTransport | null {
		const pair = entry.view.pair,
			device = this.state.snapshot?.device;
		if (!pair || !device || entry.transport || isTransferTerminal(entry.view.phase)) return null;
		const outgoing = entry.view.direction === 'outgoing';
		if (!outgoing && !entry.consented) return null;
		if (
			outgoing &&
			(!this.batch ||
				this.batch.id !== entry.batchId ||
				!sameBatch(batchSummary(this.batch.metadata), pair.batch))
		) {
			this.fail(entry, 'ไม่พบชุดไฟล์ต้นฉบับ กรุณาเลือกไฟล์ใหม่');
			return null;
		}
		const generation = this.generation,
			requestId = entry.view.clientRequestId;
		const valid = () => this.current(generation, entry, requestId) && entry.view.slotReserved;
		try {
			entry.transport = new FileTransferTransport(
				pair,
				outgoing ? this.batch!.files : null,
				entry.store,
				{
					signal: async (type, sdp, signal) => {
						if (!valid()) throw new Error('cancelled');
						await transferAPI.signal(pair.id, device.id, type, sdp, signal);
					},
					progress: (progress: TransportProgress) => {
						if (valid() && !isTransferTerminal(entry.view.phase)) this.set(entry, { ...progress });
					},
					manifest: () => {
						if (!valid() || !entry.consented || !entry.store) return;
						if (entry.intent) void this.continueReceive(entry);
						else {
							entry.consentOperation++;
							this.set(entry, { pending: false, error: '' });
							void this.showManifestPage(entry.view.id, 0);
						}
					},
					file: () => {
						if (
							!valid() ||
							!entry.consented ||
							!entry.store ||
							(this.downloadOwner && this.downloadOwner !== entry.view.id)
						)
							throw new Error('unconsented_batch');
						this.downloadStore = entry.store;
						this.downloadOwner = entry.view.id;
						this.publish({
							downloadCount: entry.store.completedCount,
							downloadBytes: entry.store.completedBytes,
							saveDestination: {
								mode: entry.store.destinationMode,
								label: entry.store.destinationLabel
							}
						});
						this.requestAutomaticDownloads(entry.store, entry.view.id);
						void this.showDownloadsPage(this.state.downloadPage);
					},
					complete: () => {
						if (!valid()) return null;
						this.set(entry, {
							phase: 'complete',
							bytes: pair.batch.totalBytes,
							completedFiles: pair.batch.fileCount,
							pending: false
						});
						if (outgoing) void this.close(entry);
					},
					failure: (message) => {
						if (valid()) this.fail(entry, message);
					},
					cleanupFailure: (message) => {
						if (this.current(generation, entry, requestId) && entry.view.pair?.id === pair.id)
							this.set(entry, { storageWarning: message });
					},
					closed: () => {
						if (valid() && entry.view.phase === 'complete') void this.close(entry);
					}
				}
			);
			return entry.transport;
		} catch {
			this.fail(entry, 'เปิดช่องทางรับส่งไม่สำเร็จ กรุณาตรวจสอบเบราว์เซอร์และเครือข่าย');
			return null;
		}
	}
	private fail(entry: Entry, message: string, pair?: TransferPair) {
		if (entry.view.phase === 'complete') {
			void this.close(entry);
			return;
		}
		entry.transport?.dispose();
		entry.transport = null;
		entry.consented = false;
		entry.intent = null;
		this.releaseEntryStore(entry);
		this.set(entry, {
			phase: 'failed',
			error: message,
			pending: false,
			...(pair ? { pair, slotReserved: true } : {})
		});
		if (entry.view.pair && entry.view.slotReserved) void this.close(entry);
		else {
			this.set(entry, { slotReserved: false });
			this.pump();
		}
	}
	private async close(entry: Entry): Promise<boolean> {
		const pair = entry.view.pair,
			device = this.state.snapshot?.device;
		const generation = this.generation,
			requestId = entry.view.clientRequestId;
		if (!device || !entry.view.slotReserved) return true;
		if (!pair) {
			await this.refresh(generation);
			if (!this.current(generation, entry, requestId)) return false;
			if (entry.view.pair) return this.close(entry);
			this.set(entry, {
				cleanupError:
					'คำขอเดิมยังอาจกำลังประมวลผล ช่องส่งยังถูกจองไว้ กรุณาปิดรับส่งไฟล์ทั้งหมดแล้วเปิดใหม่เพื่อยืนยันการยกเลิก'
			});
			return false;
		}
		if (entry.closingPairId === pair.id) return false;
		entry.closingPairId = pair.id;
		const currentPair = () =>
			this.current(generation, entry, requestId) && entry.view.pair?.id === pair.id;
		try {
			await transferAPI.close(pair.id, device.id);
			if (!currentPair()) return false;
			this.pairClosed(pair.id, 'cancelled');
			return true;
		} catch {
			if (currentPair() && entry.view.slotReserved) {
				this.set(entry, {
					cleanupError: 'ยังยืนยันการปิดคำขอไม่ได้ กดสิ้นสุดคำขออีกครั้ง หรือเปิดรับส่งไฟล์ใหม่'
				});
				await this.refresh(generation);
			}
			return currentPair() && !entry.view.slotReserved;
		} finally {
			if (entry.closingPairId === pair.id) entry.closingPairId = null;
		}
	}
	finish(id: string) {
		const entry = this.entries.get(id);
		if (entry) void this.close(entry);
	}
	private closeOrphan(pair: TransferPair) {
		const device = this.state.snapshot?.device;
		if (!device) return;
		this.retire(this.retiredPairs, pair.id);
		const generation = this.generation;
		void transferAPI.close(pair.id, device.id).catch(() => {
			if (this.current(generation))
				this.publish({ error: 'ปิดคำขอที่สิ้นสุดแล้วไม่สำเร็จ กรุณาเปิดรับส่งไฟล์ใหม่' });
		});
	}
	private pairClosed(
		pairId: string,
		reason: 'cancelled' | 'declined' | 'expired' | 'disconnected'
	) {
		this.retire(this.retiredPairs, pairId);
		this.retire(this.closedPairs, pairId);
		this.revision++;
		const entry = [...this.entries.values()].find((item) => item.view.pair?.id === pairId);
		if (!entry) return;
		entry.controller.abort();
		entry.transport?.dispose();
		entry.transport = null;
		entry.consented = false;
		entry.intent = null;
		this.releaseEntryStore(entry);
		const phase = isTransferTerminal(entry.view.phase)
			? entry.view.phase
			: reason === 'declined'
				? 'declined'
				: 'cancelled';
		this.set(entry, {
			phase,
			slotReserved: false,
			pending: false,
			cleanupError: '',
			error: isTransferTerminal(entry.view.phase)
				? entry.view.error
				: reason === 'expired'
					? 'คำขอหมดอายุ กรุณาส่งคำขอใหม่'
					: reason === 'disconnected'
						? 'อุปกรณ์อีกฝั่งขาดการเชื่อมต่อ ไฟล์ที่รับครบแล้วยังดาวน์โหลดได้'
						: ''
		});
		this.pump();
	}
}
