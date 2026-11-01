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
	type TransferTarget,
	type TransferSignal
} from '#lib/api/file-transfer.js';
import {
	BUFFER_HIGH_WATER,
	BUFFER_LOW_WATER,
	FILE_CHUNK_BYTES,
	LAN_CONNECTION_HELP,
	parseTransferControl,
	validateTransferFile
} from './file-transfer.js';

export type TransferPhase =
	| 'idle'
	| 'requesting'
	| 'waiting'
	| 'consent'
	| 'connecting'
	| 'sending'
	| 'receiving'
	| 'acknowledging'
	| 'complete'
	| 'cancelled'
	| 'failed';
export interface FileTransferState {
	status: 'idle' | 'connecting' | 'online' | 'offline';
	snapshot: TransferSnapshot | null;
	pair: TransferPair | null;
	phase: TransferPhase;
	bytes: number;
	error: string;
	download: { url: string; name: string; size: number } | null;
	pending: boolean;
}
export function initialTransferState(): FileTransferState {
	return {
		status: 'idle',
		snapshot: null,
		pair: null,
		phase: 'idle',
		bytes: 0,
		error: '',
		download: null,
		pending: false
	};
}

/** Owns this page's registration, signaling, peer and all temporary file memory. */
export class FileTransferSession {
	private state = initialTransferState();
	private generation = 0;
	private attempt = 0;
	private source: EventSource | null = null;
	private controller = new AbortController();
	private transferController = new AbortController();
	private peer: RTCPeerConnection | null = null;
	private channel: RTCDataChannel | null = null;
	private file: File | null = null;
	private chunks: ArrayBuffer[] = [];
	private consented = '';
	private negotiating = '';
	private retiredPairs = new Set<string>();
	private startupDeadline: ReturnType<typeof setTimeout> | null = null;
	private deadline: ReturnType<typeof setTimeout> | null = null;
	private stopAuth: (() => void) | null = null;
	private refreshingGeneration: number | null = null;
	private refreshAgain = false;
	private idle = createVisibilityIdle({
		...browserVisibilityDependencies(),
		onPause: () =>
			this.disconnect('พักการเชื่อมต่อเมื่อซ่อนหน้านี้ เปิดรับส่งไฟล์อีกครั้งเมื่อพร้อม'),
		onResume: () => {}
	});
	constructor(private changed: (state: FileTransferState) => void) {}
	private publish(patch: Partial<FileTransferState>) {
		this.state = { ...this.state, ...patch };
		this.changed(this.state);
	}
	private current(generation: number, attempt?: number) {
		return (
			generation === this.generation &&
			(attempt === undefined || attempt === this.attempt) &&
			!this.controller.signal.aborted
		);
	}
	private releaseDownload() {
		if (this.state.download) URL.revokeObjectURL(this.state.download.url);
		this.state = { ...this.state, download: null };
	}
	private clearDeadline() {
		if (this.deadline !== null) clearTimeout(this.deadline);
		this.deadline = null;
	}
	private armDeadline(milliseconds: number, message: string) {
		this.clearDeadline();
		this.deadline = setTimeout(() => this.fail(message), milliseconds);
	}
	private disposeTransfer() {
		this.attempt++;
		this.transferController.abort();
		this.transferController = new AbortController();
		this.clearDeadline();
		if (this.channel) {
			this.channel.onopen =
				this.channel.onclose =
				this.channel.onerror =
				this.channel.onmessage =
					null;
			this.channel.close();
		}
		if (this.peer) {
			this.peer.onconnectionstatechange = this.peer.ondatachannel = null;
			this.peer.close();
		}
		this.channel = null;
		this.peer = null;
		this.file = null;
		this.chunks = [];
		this.consented = '';
		this.negotiating = '';
	}
	private notifyClose(pair = this.state.pair, device = this.state.snapshot?.device) {
		if (pair) this.retirePair(pair.id);
		if (pair && device)
			void transferAPI.close(pair.id, device.id).catch(() => {
				// The stream/device expiry also closes this pair; never retry an uncertain mutation.
				console.warn('File transfer cancellation could not be confirmed');
			});
	}
	private retirePair(id: string) {
		this.retiredPairs.add(id);
		if (this.retiredPairs.size > 128) {
			const oldest = this.retiredPairs.values().next().value;
			if (oldest) this.retiredPairs.delete(oldest);
		}
	}
	private fail(message: string) {
		// Receipt of exact bytes and the final control commits the completed file locally.
		// Peer shutdown or a late signaling failure must not revoke that completed result.
		if (this.state.phase === 'complete') {
			this.finishCompleted();
			return;
		}
		this.notifyClose();
		this.disposeTransfer();
		this.releaseDownload();
		this.publish({ phase: 'failed', error: message, pending: false });
	}
	private disconnect(message: string) {
		this.stop();
		this.publish({
			status: 'offline',
			snapshot: null,
			pair: null,
			phase: 'idle',
			error: message,
			pending: false
		});
	}
	stop() {
		this.generation++;
		if (this.startupDeadline !== null) clearTimeout(this.startupDeadline);
		this.startupDeadline = null;
		this.controller.abort();
		this.source?.close();
		this.source = null;
		this.stopAuth?.();
		this.stopAuth = null;
		this.idle.stop();
		this.disposeTransfer();
		this.releaseDownload();
		const device = this.state.snapshot?.device;
		if (device)
			void transferAPI
				.remove(device.id)
				.catch(() => console.warn('File transfer device cleanup could not be confirmed'));
		this.state = initialTransferState();
		this.retiredPairs.clear();
		this.changed(this.state);
	}
	async start(label: string) {
		this.stop();
		if (typeof RTCPeerConnection === 'undefined' || typeof EventSource === 'undefined') {
			this.publish({
				status: 'offline',
				error: 'เบราว์เซอร์นี้ไม่รองรับการส่งไฟล์โดยตรง กรุณาใช้เบราว์เซอร์รุ่นปัจจุบัน'
			});
			return;
		}
		if (!label.trim() || [...label.trim()].length > 60 || /[\p{Cc}]/u.test(label)) {
			this.publish({ error: 'กรุณาตั้งชื่ออุปกรณ์ไม่เกิน 60 ตัวอักษร' });
			return;
		}
		const auth = get(authStore);
		if (!auth.isAuthenticated || auth.isUnavailable || auth.user?.user_type !== 'staff') return;
		this.controller = new AbortController();
		const generation = this.generation;
		const epoch = authStore.sessionEpoch;
		this.publish({ status: 'connecting', error: '' });
		this.startupDeadline = setTimeout(() => {
			if (this.current(generation)) this.disconnect('เปิดการเชื่อมต่อนานเกินไป กรุณาลองใหม่');
		}, 30_000);
		this.stopAuth = authStore.subscribe((next) => {
			if (
				epoch !== authStore.sessionEpoch ||
				!next.isAuthenticated ||
				next.isUnavailable ||
				next.user?.user_type !== 'staff'
			)
				this.disconnect('การยืนยันตัวตนเปลี่ยนไป กรุณาเข้าสู่ระบบใหม่ก่อนรับส่งไฟล์');
		});
		try {
			const snapshot = await transferAPI.register(label.trim(), this.controller.signal);
			if (!this.current(generation)) return;
			this.publish({ snapshot });
			const source = new EventSource(transferAPI.eventsUrl(snapshot.device.id), {
				withCredentials: true
			});
			this.source = source;
			source.addEventListener('file_transfer', (event: MessageEvent<string>) => {
				if (!this.current(generation)) return;
				try {
					this.event(parseTransferEvent(event.data), generation);
				} catch {
					this.disconnect('ข้อมูลการเชื่อมต่อไม่ถูกต้อง กรุณาเปิดรับส่งไฟล์ใหม่');
				}
			});
			source.onerror = () => {
				if (this.current(generation))
					this.disconnect('ขาดการเชื่อมต่อกับระบบ เปิดรับส่งไฟล์ใหม่และจับคู่อีกครั้ง');
			};
			this.idle.start();
		} catch (error) {
			if (this.current(generation))
				this.disconnect(this.message(error, 'เปิดรับส่งไฟล์ไม่สำเร็จ กรุณาลองใหม่'));
		}
	}
	private message(error: unknown, fallback: string) {
		if (
			error instanceof Error &&
			/^(?:กรุณา|ไม่|เปิด|โหลด|ส่ง|ตอบ|ยกเลิก|ปิด|รหัส|อุปกรณ์|ไฟล์)/u.test(error.message)
		)
			return error.message;
		return fallback;
	}
	private event(event: ReturnType<typeof parseTransferEvent>, generation: number) {
		switch (event.type) {
			case 'ready':
				if (this.startupDeadline !== null) clearTimeout(this.startupDeadline);
				this.startupDeadline = null;
				this.applySnapshot(event.data);
				this.publish({ status: 'online', error: '' });
				break;
			case 'presence_changed':
				void this.refresh(generation);
				break;
			case 'pair_changed':
				this.applyPair(event.data);
				break;
			case 'pair_closed':
				this.retirePair(event.data.pairId);
				if (this.state.pair?.id !== event.data.pairId) return;
				if (['complete', 'failed', 'cancelled'].includes(this.state.phase)) {
					this.disposeTransfer();
					this.publish({ pair: null, pending: false });
					return;
				}
				this.disposeTransfer();
				this.releaseDownload();
				this.publish({
					pair: null,
					phase: 'cancelled',
					pending: false,
					error:
						event.data.reason === 'declined'
							? 'ผู้รับปฏิเสธคำขอ'
							: event.data.reason === 'expired'
								? 'คำขอหมดอายุ กรุณาส่งคำขอใหม่'
								: 'การรับส่งไฟล์สิ้นสุดแล้ว'
				});
				break;
			case 'signal':
				void this.receiveSignal(event.data, generation);
				break;
			case 'session_invalid':
			case 'session_unavailable':
				this.disconnect('ไม่สามารถยืนยันสิทธิ์ได้ กรุณาเข้าสู่ระบบใหม่ก่อนรับส่งไฟล์');
				break;
		}
	}
	private async refresh(generation: number) {
		if (this.refreshingGeneration === generation) {
			this.refreshAgain = true;
			return;
		}
		const device = this.state.snapshot?.device;
		if (!device) return;
		this.refreshingGeneration = generation;
		try {
			do {
				this.refreshAgain = false;
				const snapshot = await transferAPI.snapshot(device.id, this.controller.signal);
				if (!this.current(generation)) return;
				this.applySnapshot(snapshot);
			} while (this.refreshAgain);
		} catch {
			if (this.current(generation))
				this.disconnect('โหลดสถานะอุปกรณ์ไม่สำเร็จ กรุณาเปิดรับส่งไฟล์ใหม่');
		} finally {
			if (this.refreshingGeneration === generation) this.refreshingGeneration = null;
		}
	}
	private applySnapshot(snapshot: TransferSnapshot) {
		if (snapshot.device.id !== this.state.snapshot?.device.id) throw new Error('device_changed');
		this.publish({ snapshot });
		for (const pair of snapshot.pairs) this.applyPair(pair);
	}
	private applyPair(pair: TransferPair) {
		if (this.retiredPairs.has(pair.id)) return;
		const previous = this.state.pair;
		if (previous?.id === pair.id) {
			if (
				previous.initiator.id !== pair.initiator.id ||
				previous.recipient.id !== pair.recipient.id ||
				previous.file.name !== pair.file.name ||
				previous.file.size !== pair.file.size ||
				previous.file.mimeType !== pair.file.mimeType
			)
				throw new Error('pair_changed_identity');
			if (previous.status === 'accepted' && pair.status === 'pending') return;
			if (
				this.state.phase === 'complete' ||
				this.state.phase === 'failed' ||
				this.state.phase === 'cancelled'
			)
				return;
		}
		const id = this.state.snapshot?.device.id;
		if (!id || (pair.initiator.id !== id && pair.recipient.id !== id))
			throw new Error('wrong_device');
		if (validateTransferFile(pair.file)) {
			this.notifyClose(pair);
			throw new Error('invalid_file');
		}
		if (this.state.pair && this.state.pair.id !== pair.id) throw new Error('unexpected_pair');
		this.publish({ pair });
		if (pair.status === 'pending') {
			if (!previous) this.armDeadline(60_000, 'คำขอหมดอายุ กรุณาส่งคำขอใหม่');
			this.publish({ phase: pair.initiator.id === id ? 'waiting' : 'consent' });
		} else if (pair.initiator.id === id && this.negotiating !== pair.id) {
			if (
				!this.file ||
				this.file.name !== pair.file.name ||
				this.file.size !== pair.file.size ||
				this.file.type !== pair.file.mimeType
			) {
				this.fail('ไฟล์ที่เลือกไม่ตรงกับคำขอ กรุณาเลือกไฟล์ใหม่');
				return;
			}
			this.negotiating = pair.id;
			void this.offer(pair, this.generation);
		}
	}
	async send(file: File, target: TransferTarget) {
		const device = this.state.snapshot?.device;
		if (this.state.status !== 'online' || !device || this.state.pending || this.state.pair) return;
		const error = validateTransferFile({ name: file.name, size: file.size, mimeType: file.type });
		if (error) {
			this.publish({ error });
			return;
		}
		this.disposeTransfer();
		this.releaseDownload();
		this.file = file;
		const generation = this.generation,
			attempt = this.attempt;
		this.publish({ phase: 'requesting', pending: true, bytes: 0, error: '' });
		try {
			const pair = await transferAPI.request(
				device.id,
				target,
				{ name: file.name, size: file.size, mimeType: file.type },
				this.transferController.signal
			);
			if (this.current(generation, attempt)) this.applyPair(pair);
		} catch (error) {
			if (this.current(generation, attempt))
				this.fail(this.message(error, 'ส่งคำขอไม่สำเร็จ ตรวจสอบรหัสหรือเลือกอุปกรณ์ใหม่'));
		} finally {
			if (this.current(generation, attempt)) this.publish({ pending: false });
		}
	}
	private isConnecting() {
		return this.state.phase === 'connecting';
	}
	async accept() {
		const pair = this.state.pair,
			device = this.state.snapshot?.device;
		if (
			!pair ||
			!device ||
			pair.recipient.id !== device.id ||
			this.state.phase !== 'consent' ||
			this.state.pending
		)
			return;
		this.releaseDownload();
		this.consented = pair.id;
		const generation = this.generation,
			attempt = this.attempt;
		this.publish({ pending: true, phase: 'connecting', error: '' });
		this.armDeadline(20_000, LAN_CONNECTION_HELP);
		try {
			const accepted = await transferAPI.accept(pair.id, device.id, this.transferController.signal);
			if (!this.current(generation, attempt)) return;
			this.applyPair(accepted);
		} catch (error) {
			if (this.current(generation, attempt) && this.isConnecting() && !this.peer) {
				this.clearDeadline();
				this.consented = '';
				this.publish({
					phase: 'consent',
					error: this.message(error, 'ตอบรับไม่สำเร็จ กรุณาลองใหม่')
				});
			}
		} finally {
			if (this.current(generation, attempt)) this.publish({ pending: false });
		}
	}
	cancel() {
		this.notifyClose();
		this.disposeTransfer();
		this.releaseDownload();
		this.publish({ pair: null, phase: 'cancelled', bytes: 0, error: '', pending: false });
	}
	discard() {
		this.releaseDownload();
		this.publish({ download: null });
	}
	private finishCompleted() {
		if (this.state.phase !== 'complete' || !this.state.pair) return;
		this.notifyClose();
		this.disposeTransfer();
		this.publish({ pair: null, pending: false });
	}
	finish() {
		this.finishCompleted();
	}
	private createPeer(pair: TransferPair, generation: number, attempt: number) {
		const peer = new RTCPeerConnection({ iceServers: [] });
		this.peer = peer;
		this.publish({ phase: 'connecting', bytes: 0 });
		this.armDeadline(20_000, LAN_CONNECTION_HELP);
		peer.onconnectionstatechange = () => {
			if (!this.current(generation, attempt)) return;
			if (peer.connectionState === 'failed' || peer.connectionState === 'disconnected') {
				if (this.state.phase === 'complete') this.finishCompleted();
				else this.fail(LAN_CONNECTION_HELP);
			}
		};
		peer.ondatachannel = ({ channel }) => {
			if (
				!this.current(generation, attempt) ||
				this.channel ||
				this.consented !== pair.id ||
				pair.recipient.id !== this.state.snapshot?.device.id ||
				channel.label !== 'schoolorbit-file-v1' ||
				!channel.ordered ||
				channel.maxRetransmits !== null ||
				channel.maxPacketLifeTime !== null
			) {
				channel.close();
				if (this.current(generation, attempt)) this.fail('ช่องทางรับไฟล์ไม่ถูกต้อง');
				return;
			}
			this.attachChannel(channel, pair, generation, attempt);
		};
		return peer;
	}
	private async gathered(peer: RTCPeerConnection, signal: AbortSignal): Promise<string> {
		if (peer.iceGatheringState !== 'complete')
			await new Promise<void>((resolve, reject) => {
				const cleanup = () => {
					clearTimeout(timer);
					peer.removeEventListener('icegatheringstatechange', changed);
					signal.removeEventListener('abort', aborted);
				};
				const changed = () => {
					if (peer.iceGatheringState === 'complete') {
						cleanup();
						resolve();
					}
				};
				const aborted = () => {
					cleanup();
					reject(new DOMException('Aborted', 'AbortError'));
				};
				const timer = setTimeout(() => {
					cleanup();
					reject(new Error(LAN_CONNECTION_HELP));
				}, 10_000);
				peer.addEventListener('icegatheringstatechange', changed);
				signal.addEventListener('abort', aborted, { once: true });
				if (signal.aborted) aborted();
				else changed();
			});
		const sdp = peer.localDescription?.sdp;
		if (!sdp || sdp.length > 32768) throw new Error('ข้อมูลการเชื่อมต่อไม่ถูกต้อง');
		return sdp;
	}
	private async offer(pair: TransferPair, generation: number) {
		const attempt = this.attempt,
			device = this.state.snapshot?.device;
		if (!device) return;
		try {
			const peer = this.createPeer(pair, generation, attempt);
			this.attachChannel(
				peer.createDataChannel('schoolorbit-file-v1', { ordered: true }),
				pair,
				generation,
				attempt
			);
			await peer.setLocalDescription(await peer.createOffer());
			const sdp = await this.gathered(peer, this.transferController.signal);
			if (!this.current(generation, attempt)) return;
			await transferAPI.signal(pair.id, device.id, 'offer', sdp, this.transferController.signal);
		} catch {
			if (this.current(generation, attempt)) this.fail(LAN_CONNECTION_HELP);
		}
	}
	private async receiveSignal(signal: TransferSignal, generation: number) {
		const pair = this.state.pair,
			device = this.state.snapshot?.device,
			attempt = this.attempt;
		if (!pair || !device || pair.id !== signal.pairId || pair.status !== 'accepted') return;
		try {
			if (signal.type === 'offer') {
				if (pair.recipient.id !== device.id || this.consented !== pair.id || this.peer)
					throw new Error('unexpected_offer');
				const peer = this.createPeer(pair, generation, attempt);
				await peer.setRemoteDescription({ type: 'offer', sdp: signal.sdp });
				await peer.setLocalDescription(await peer.createAnswer());
				const sdp = await this.gathered(peer, this.transferController.signal);
				if (!this.current(generation, attempt)) return;
				await transferAPI.signal(pair.id, device.id, 'answer', sdp, this.transferController.signal);
			} else {
				if (
					pair.initiator.id !== device.id ||
					!this.peer ||
					this.peer.signalingState !== 'have-local-offer'
				)
					throw new Error('unexpected_answer');
				await this.peer.setRemoteDescription({ type: 'answer', sdp: signal.sdp });
			}
		} catch {
			if (this.current(generation, attempt)) this.fail(LAN_CONNECTION_HELP);
		}
	}
	private attachChannel(
		channel: RTCDataChannel,
		pair: TransferPair,
		generation: number,
		attempt: number
	) {
		this.channel = channel;
		channel.binaryType = 'arraybuffer';
		channel.bufferedAmountLowThreshold = BUFFER_LOW_WATER;
		const sending = pair.initiator.id === this.state.snapshot?.device.id;
		let opened = false;
		const open = () => {
			if (opened || !this.current(generation, attempt)) return;
			opened = true;
			this.publish({ phase: sending ? 'sending' : 'receiving' });
			this.armDeadline(30_000, 'การรับส่งไฟล์หยุดนานเกินไป กรุณาจับคู่และส่งใหม่');
			if (sending) void this.sendBytes(channel, pair, generation, attempt);
		};
		channel.onopen = open;
		if (channel.readyState === 'open') open();
		channel.onclose = () => {
			if (this.current(generation, attempt) && this.state.phase === 'complete') {
				this.finishCompleted();
				return;
			}
			if (this.current(generation, attempt) && this.state.phase !== 'complete')
				this.fail('ช่องทางส่งไฟล์ปิดก่อนรับไฟล์ครบ กรุณาส่งใหม่');
		};
		channel.onerror = () => {
			if (this.current(generation, attempt)) this.fail(LAN_CONNECTION_HELP);
		};
		channel.onmessage = (event: MessageEvent<unknown>) => {
			if (!this.current(generation, attempt)) return;
			if (channel.readyState === 'open') open();
			try {
				if (typeof event.data === 'string') {
					const control = parseTransferControl(event.data);
					if (sending) {
						if (
							control.type !== 'received' ||
							this.state.phase !== 'acknowledging' ||
							control.bytes !== pair.file.size
						)
							throw new Error('invalid_ack');
						this.notifyClose();
						this.disposeTransfer();
						this.publish({ pair: null, phase: 'complete', bytes: control.bytes, pending: false });
					} else {
						if (
							control.type !== 'end' ||
							this.state.phase !== 'receiving' ||
							control.bytes !== pair.file.size ||
							this.state.bytes !== pair.file.size
						)
							throw new Error('invalid_end');
						const blob = new Blob(this.chunks, { type: 'application/octet-stream' });
						this.chunks = [];
						this.releaseDownload();
						channel.send(JSON.stringify({ type: 'received', bytes: blob.size }));
						const download = {
							url: URL.createObjectURL(blob),
							name: pair.file.name,
							size: blob.size
						};
						this.clearDeadline();
						this.publish({ phase: 'complete', download, pending: false });
					}
				} else {
					if (
						sending ||
						this.state.phase !== 'receiving' ||
						!(event.data instanceof ArrayBuffer) ||
						event.data.byteLength === 0 ||
						event.data.byteLength > FILE_CHUNK_BYTES ||
						this.state.bytes + event.data.byteLength > pair.file.size
					)
						throw new Error('invalid_chunk');
					this.chunks.push(event.data);
					this.publish({ bytes: this.state.bytes + event.data.byteLength });
					this.armDeadline(30_000, 'การรับส่งไฟล์หยุดนานเกินไป กรุณาส่งใหม่');
				}
			} catch {
				this.fail('ข้อมูลไฟล์ไม่ครบหรือไม่ถูกต้อง ยกเลิกการรับไฟล์แล้ว กรุณาส่งใหม่');
			}
		};
	}
	private async drained(channel: RTCDataChannel, signal: AbortSignal) {
		if (channel.bufferedAmount <= BUFFER_HIGH_WATER - FILE_CHUNK_BYTES) return;
		await new Promise<void>((resolve, reject) => {
			const cleanup = () => {
				channel.removeEventListener('bufferedamountlow', ready);
				channel.removeEventListener('close', closed);
				signal.removeEventListener('abort', closed);
			};
			const ready = () => {
				cleanup();
				resolve();
			};
			const closed = () => {
				cleanup();
				reject(new Error('channel_closed'));
			};
			channel.addEventListener('bufferedamountlow', ready, { once: true });
			channel.addEventListener('close', closed, { once: true });
			signal.addEventListener('abort', closed, { once: true });
			if (signal.aborted || channel.readyState !== 'open') closed();
			else if (channel.bufferedAmount <= BUFFER_LOW_WATER) ready();
		});
	}
	private async sendBytes(
		channel: RTCDataChannel,
		pair: TransferPair,
		generation: number,
		attempt: number
	) {
		const file = this.file;
		if (!file) {
			this.fail('ไม่พบไฟล์ที่เลือก กรุณาเลือกไฟล์ใหม่');
			return;
		}
		try {
			for (let offset = 0; offset < file.size; offset += FILE_CHUNK_BYTES) {
				await this.drained(channel, this.transferController.signal);
				const chunk = await file.slice(offset, offset + FILE_CHUNK_BYTES).arrayBuffer();
				if (!this.current(generation, attempt)) return;
				channel.send(chunk);
				this.publish({ bytes: offset + chunk.byteLength });
				this.armDeadline(30_000, 'การรับส่งไฟล์หยุดนานเกินไป กรุณาส่งใหม่');
			}
			if (!this.current(generation, attempt)) return;
			this.publish({ phase: 'acknowledging' });
			channel.send(JSON.stringify({ type: 'end', bytes: pair.file.size }));
		} catch {
			if (this.current(generation, attempt)) this.fail('ส่งไฟล์ไม่สำเร็จ กรุณาจับคู่และส่งใหม่');
		}
	}
}
