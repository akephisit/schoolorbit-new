import type { TransferPair, TransferSignal } from '../api/file-transfer.js';
import {
	BUFFER_HIGH_WATER,
	BUFFER_LOW_WATER,
	FILE_CHUNK_BYTES,
	FILE_READ_BYTES,
	MANIFEST_WINDOW,
	PERSISTED_ACK_BYTES,
	LAN_CONNECTION_HELP,
	fileMetadata,
	parseTransferControl,
	type TransferControl,
	type FileMetadata
} from './file-transfer.js';
import { ReceivedBatchStore, TransferStorageError } from './file-transfer-storage.js';
import { TransferRate } from './file-transfer-rate.js';

export type TransportPhase =
	'connecting' | 'manifest' | 'review' | 'sending' | 'receiving' | 'acknowledging' | 'complete';
export interface TransportProgress {
	phase: TransportPhase;
	bytes: number;
	fileIndex: number;
	fileBytes: number;
	completedFiles: number;
	megabytesPerSecond: number;
}
type Ports = {
	signal(type: 'offer' | 'answer', sdp: string, signal: AbortSignal): Promise<void>;
	progress(progress: TransportProgress): void;
	manifest(): void;
	file(index: number): void;
	complete(): void;
	failure(message: string): void;
	closed(): void;
	cleanupFailure(message: string): void;
};
/** One pair owns network state and a bounded inbox; its parent retains completed disk files. */
export class FileTransferTransport {
	private controller = new AbortController();
	private peer: RTCPeerConnection;
	private channel: RTCDataChannel | null = null;
	private stopped = false;
	private completed = false;
	private started = false;
	private answered = false;
	private bytes = 0;
	private sentBytes = 0;
	private fileBytes = 0;
	private fileIndex = 0;
	private completedFiles = 0;
	private receivingFile: FileMetadata | null = null;
	private receivingConsent = false;
	private manifestStarted = false;
	private manifestSealed = false;
	private manifestSent = 0;
	private manifestAck = 0;
	private remoteManifestReady = false;
	private remoteReceiveReady = false;
	private lastPersistedAck = 0;
	private receivedFileAck = -1;
	private awaitingBatch = false;
	private phase: TransportPhase = 'connecting';
	private lastActivity = performance.now();
	private lastPaint = 0;
	private rate = new TransferRate();
	private rateTimer: ReturnType<typeof setInterval> | null = null;
	private watchdog: ReturnType<typeof setTimeout> | null = null;
	private reviewTimer: ReturnType<typeof setTimeout> | null = null;
	private connectTimer: ReturnType<typeof setTimeout> | null = null;
	private inbox: (string | ArrayBuffer)[] = [];
	private inboxBytes = 0;
	private inboxControls = 0;
	private processing = false;
	private waiter: { resolve(): void; reject(error: Error): void } | null = null;
	constructor(
		private pair: TransferPair,
		private files: readonly File[] | null,
		private store: ReceivedBatchStore | null,
		private ports: Ports
	) {
		this.peer = new RTCPeerConnection({ iceServers: [] });
		this.connectTimer = setTimeout(() => this.fail(LAN_CONNECTION_HELP), 20_000);
		this.peer.onconnectionstatechange = () => {
			if (!this.live()) return;
			if (this.peer.connectionState === 'failed' || this.peer.connectionState === 'disconnected') {
				if (this.completed) this.finishClosed();
				else this.fail(LAN_CONNECTION_HELP);
			}
		};
		this.peer.ondatachannel = ({ channel }) => {
			if (
				this.files ||
				this.channel ||
				channel.label !== 'schoolorbit-files-v3' ||
				!channel.ordered ||
				channel.maxRetransmits !== null ||
				channel.maxPacketLifeTime !== null
			) {
				channel.close();
				this.fail('ช่องทางรับชุดไฟล์ไม่ถูกต้อง กรุณารีเฟรชทั้งสองอุปกรณ์');
				return;
			}
			this.attach(channel);
		};
	}
	private live() {
		return !this.stopped && !this.controller.signal.aborted;
	}
	private touch() {
		this.lastActivity = performance.now();
	}
	private progress(force = false) {
		if (this.stopped || (!force && performance.now() - this.lastPaint < 100)) return;
		this.lastPaint = performance.now();
		this.ports.progress({
			phase: this.phase,
			bytes: this.bytes,
			fileIndex: this.fileIndex,
			fileBytes: this.fileBytes,
			completedFiles: this.completedFiles,
			megabytesPerSecond: ['sending', 'receiving', 'acknowledging'].includes(this.phase)
				? this.rate.sample(performance.now(), this.bytes)
				: 0
		});
	}
	private watch() {
		if (!this.live() || this.completed || this.phase === 'review') return;
		const remaining = 30_000 - (performance.now() - this.lastActivity);
		if (remaining <= 0) {
			this.fail('การรับส่งชุดไฟล์หยุดนานเกินไป กรุณาส่งคำขอใหม่ ไฟล์ที่รับครบแล้วยังดาวน์โหลดได้');
			return;
		}
		this.watchdog = setTimeout(() => this.watch(), remaining);
	}
	private review() {
		this.phase = 'review';
		if (this.watchdog !== null) clearTimeout(this.watchdog);
		this.watchdog = null;
		this.reviewTimer = setTimeout(
			() => this.fail('หมดเวลาตรวจสอบรายการไฟล์ กรุณาให้ผู้ส่งส่งคำขอใหม่'),
			10 * 60_000
		);
		this.progress(true);
	}
	private transferring() {
		if (this.reviewTimer !== null) clearTimeout(this.reviewTimer);
		this.reviewTimer = null;
		this.phase = this.files ? 'sending' : 'receiving';
		this.touch();
		this.rate.reset(performance.now(), this.bytes);
		this.watch();
		this.progress(true);
	}
	private clearTimers() {
		if (this.rateTimer !== null) clearInterval(this.rateTimer);
		for (const timer of [this.watchdog, this.reviewTimer, this.connectTimer])
			if (timer !== null) clearTimeout(timer);
		this.rateTimer = null;
		this.watchdog = this.reviewTimer = this.connectTimer = null;
	}
	dispose() {
		if (this.stopped) return;
		this.stopped = true;
		this.controller.abort();
		this.clearTimers();
		this.waiter?.reject(new Error('cancelled'));
		this.waiter = null;
		if (this.channel) {
			this.channel.onopen =
				this.channel.onclose =
				this.channel.onerror =
				this.channel.onmessage =
					null;
			this.channel.close();
		}
		this.peer.onconnectionstatechange = this.peer.ondatachannel = null;
		this.peer.close();
		this.channel = null;
		this.inbox = [];
		this.inboxBytes = this.inboxControls = 0;
		this.files = null;
		if (this.store)
			void this.store
				.abortPartial()
				.catch((error) =>
					this.ports.cleanupFailure(
						error instanceof TransferStorageError
							? error.message
							: 'ยังยืนยันการล้างไฟล์บางส่วนไม่ได้ กรุณาตรวจสอบโฟลเดอร์ปลายทาง ไฟล์ที่บันทึกครบแล้วคงอยู่'
					)
				);
	}
	private fail(message: string) {
		if (this.stopped) return;
		if (this.completed) {
			this.finishClosed();
			return;
		}
		this.dispose();
		this.ports.failure(message);
	}
	private finishClosed() {
		if (!this.stopped) {
			this.dispose();
			this.ports.closed();
		}
	}
	private complete() {
		if (this.completed || !this.live()) return;
		this.completed = true;
		this.phase = 'complete';
		this.clearTimers();
		this.progress(true);
		this.ports.complete();
		if (this.files) this.finishClosed();
	}
	async offer() {
		if (!this.files || this.started || !this.live()) return;
		this.started = true;
		try {
			this.attach(this.peer.createDataChannel('schoolorbit-files-v3', { ordered: true }));
			await this.peer.setLocalDescription(await this.peer.createOffer());
			const sdp = await this.gathered();
			if (this.live()) await this.ports.signal('offer', sdp, this.controller.signal);
		} catch {
			if (this.live()) this.fail(LAN_CONNECTION_HELP);
		}
	}
	async signal(signal: TransferSignal) {
		if (!this.live() || signal.pairId !== this.pair.id) return;
		try {
			if (signal.type === 'offer') {
				if (this.files || this.answered) throw new Error('unexpected_offer');
				this.answered = true;
				await this.peer.setRemoteDescription({ type: 'offer', sdp: signal.sdp });
				await this.peer.setLocalDescription(await this.peer.createAnswer());
				const sdp = await this.gathered();
				if (this.live()) await this.ports.signal('answer', sdp, this.controller.signal);
			} else {
				if (!this.files || this.peer.signalingState !== 'have-local-offer')
					throw new Error('unexpected_answer');
				await this.peer.setRemoteDescription({ type: 'answer', sdp: signal.sdp });
			}
		} catch {
			if (this.live()) this.fail(LAN_CONNECTION_HELP);
		}
	}
	private async gathered(): Promise<string> {
		if (this.peer.iceGatheringState !== 'complete')
			await new Promise<void>((resolve, reject) => {
				const signal = this.controller.signal;
				const cleanup = () => {
					clearTimeout(timer);
					this.peer.removeEventListener('icegatheringstatechange', changed);
					signal.removeEventListener('abort', aborted);
				};
				const changed = () => {
					if (this.peer.iceGatheringState === 'complete') {
						cleanup();
						resolve();
					}
				};
				const aborted = () => {
					cleanup();
					reject(new Error('cancelled'));
				};
				const timer = setTimeout(() => {
					cleanup();
					reject(new Error('gathering_timeout'));
				}, 10_000);
				this.peer.addEventListener('icegatheringstatechange', changed);
				signal.addEventListener('abort', aborted, { once: true });
				if (signal.aborted) aborted();
				else changed();
			});
		const sdp = this.peer.localDescription?.sdp;
		if (!sdp || sdp.length > 32768) throw new Error('invalid_sdp');
		return sdp;
	}
	private attach(channel: RTCDataChannel) {
		this.channel = channel;
		channel.binaryType = 'arraybuffer';
		channel.bufferedAmountLowThreshold = BUFFER_LOW_WATER;
		let opened = false;
		const open = () => {
			if (opened || !this.live()) return;
			opened = true;
			if (this.connectTimer !== null) clearTimeout(this.connectTimer);
			this.connectTimer = null;
			this.phase = 'manifest';
			this.touch();
			this.watch();
			this.rateTimer = setInterval(() => this.progress(true), 500);
			this.progress(true);
			if (this.files) void this.sendFiles();
		};
		channel.onopen = open;
		channel.onclose = () => {
			if (this.completed) this.finishClosed();
			else this.fail('ช่องทางปิดก่อนรับชุดไฟล์ครบ กรุณาส่งใหม่ ไฟล์ที่รับครบแล้วยังดาวน์โหลดได้');
		};
		channel.onerror = () => this.fail(LAN_CONNECTION_HELP);
		channel.onmessage = (event: MessageEvent<unknown>) => {
			if (!this.live()) return;
			open();
			if (typeof event.data === 'string') {
				if (event.data.length > 4096 || this.inboxControls >= 32) {
					this.fail('ข้อมูลควบคุมมากเกินไป ยกเลิกการรับชุดนี้แล้ว');
					return;
				}
				this.inboxControls++;
			} else if (
				event.data instanceof ArrayBuffer &&
				event.data.byteLength > 0 &&
				event.data.byteLength <= FILE_CHUNK_BYTES &&
				this.inboxBytes + event.data.byteLength <= BUFFER_HIGH_WATER &&
				!this.files &&
				this.receivingConsent
			)
				this.inboxBytes += event.data.byteLength;
			else {
				this.fail('ข้อมูลชุดไฟล์ไม่ครบหรือไม่ถูกต้อง ยกเลิกส่วนที่ยังรับไม่ครบแล้ว');
				return;
			}
			this.inbox.push(event.data);
			void this.processInbox();
		};
		if (channel.readyState === 'open') open();
	}
	private async processInbox() {
		if (this.processing) return;
		this.processing = true;
		try {
			while (this.live() && this.inbox.length) {
				const first = this.inbox.shift()!;
				if (typeof first === 'string') {
					this.inboxControls--;
					await this.control(parseTransferControl(first));
				} else {
					const frames = [first];
					let size = first.byteLength;
					while (
						this.inbox[0] instanceof ArrayBuffer &&
						size + this.inbox[0].byteLength <= PERSISTED_ACK_BYTES
					) {
						const next = this.inbox.shift() as ArrayBuffer;
						frames.push(next);
						size += next.byteLength;
					}
					this.inboxBytes -= size;
					const data = new Uint8Array(size);
					let offset = 0;
					for (const frame of frames) {
						data.set(new Uint8Array(frame), offset);
						offset += frame.byteLength;
					}
					await this.binary(data);
				}
			}
		} catch (error) {
			if (this.live())
				this.fail(
					error instanceof TransferStorageError
						? error.message
						: 'ข้อมูลชุดไฟล์ไม่ครบหรือไม่ถูกต้อง ยกเลิกส่วนที่ยังรับไม่ครบแล้ว ไฟล์ที่รับครบแล้วยังดาวน์โหลดได้'
				);
		} finally {
			this.processing = false;
		}
	}
	private sendControl(control: TransferControl) {
		if (!this.live() || !this.channel || this.channel.readyState !== 'open')
			throw new Error('channel_closed');
		this.channel.send(JSON.stringify(control));
	}
	private wake() {
		const waiter = this.waiter;
		this.waiter = null;
		waiter?.resolve();
	}
	private async until(predicate: () => boolean) {
		while (this.live() && !predicate())
			await new Promise<void>((resolve, reject) => {
				this.waiter = { resolve, reject };
			});
		if (!this.live()) throw new Error('cancelled');
	}
	private matches(control: { count: number; totalBytes: number }) {
		return (
			control.count === this.pair.batch.fileCount &&
			control.totalBytes === this.pair.batch.totalBytes
		);
	}
	private async control(control: TransferControl) {
		if (this.completed) throw new Error('batch_already_complete');
		if (this.files) {
			if (
				control.type === 'manifest_ack' &&
				this.phase === 'manifest' &&
				control.count >= this.manifestAck &&
				control.count <= this.manifestSent
			) {
				this.manifestAck = control.count;
				this.touch();
				this.wake();
				return;
			}
			if (
				control.type === 'manifest_ready' &&
				this.phase === 'manifest' &&
				this.manifestSent === this.files.length &&
				this.matches(control) &&
				!this.remoteManifestReady
			) {
				this.remoteManifestReady = true;
				this.touch();
				this.review();
				this.wake();
				return;
			}
			if (
				control.type === 'receive_ready' &&
				this.remoteManifestReady &&
				!this.remoteReceiveReady &&
				this.matches(control)
			) {
				this.remoteReceiveReady = true;
				this.transferring();
				this.wake();
				return;
			}
			if (
				control.type === 'persisted' &&
				this.remoteReceiveReady &&
				control.bytes >= this.bytes &&
				control.bytes <= this.sentBytes
			) {
				if (control.bytes > this.bytes) this.touch();
				this.bytes = control.bytes;
				this.progress();
				this.wake();
				return;
			}
			if (
				control.type === 'file_received' &&
				this.remoteReceiveReady &&
				this.phase === 'acknowledging' &&
				control.index === this.fileIndex &&
				control.bytes === this.files[this.fileIndex]?.size &&
				this.receivedFileAck < this.fileIndex &&
				this.bytes === this.sentBytes
			) {
				this.receivedFileAck = control.index;
				this.touch();
				this.wake();
				return;
			}
			if (
				control.type === 'batch_received' &&
				this.awaitingBatch &&
				this.matches(control) &&
				this.bytes === this.pair.batch.totalBytes
			) {
				this.complete();
				return;
			}
			throw new Error('invalid_ack');
		}
		const store = this.store;
		if (!store) throw new Error('missing_store');
		if (control.type === 'manifest_start' && !this.manifestStarted && this.matches(control)) {
			this.manifestStarted = true;
			this.touch();
			return;
		}
		if (control.type === 'manifest_file' && this.manifestStarted && !this.manifestSealed) {
			await store.appendManifest(control.index, control.file);
			if (!this.live()) return;
			this.sendControl({ type: 'manifest_ack', count: store.manifestCount });
			this.touch();
			return;
		}
		if (
			control.type === 'manifest_end' &&
			this.manifestStarted &&
			!this.manifestSealed &&
			this.matches(control)
		) {
			await store.sealManifest(this.pair.batch);
			if (!this.live()) return;
			this.manifestSealed = true;
			this.sendControl({
				type: 'manifest_ready',
				count: store.manifestCount,
				totalBytes: this.pair.batch.totalBytes
			});
			this.review();
			this.ports.manifest();
			return;
		}
		if (
			control.type === 'file_start' &&
			this.manifestSealed &&
			this.receivingConsent &&
			!this.receivingFile &&
			control.index === this.fileIndex &&
			this.fileIndex < this.pair.batch.fileCount
		) {
			const metadata = await store.metadata(this.fileIndex);
			await store.beginFile(this.fileIndex);
			if (!this.live()) return;
			this.receivingFile = metadata;
			this.fileBytes = 0;
			this.touch();
			this.progress(true);
			return;
		}
		if (
			control.type === 'file_end' &&
			this.receivingFile &&
			control.index === this.fileIndex &&
			control.bytes === this.receivingFile.size &&
			this.fileBytes === control.bytes
		) {
			await store.completeFile(this.fileIndex, control.bytes);
			if (!this.live()) return;
			this.persisted(true);
			this.ports.file(this.fileIndex);
			this.sendControl({ type: 'file_received', index: this.fileIndex, bytes: control.bytes });
			this.receivingFile = null;
			this.completedFiles++;
			this.fileIndex++;
			this.touch();
			this.progress(true);
			return;
		}
		if (
			control.type === 'batch_end' &&
			this.receivingConsent &&
			!this.receivingFile &&
			this.fileIndex === this.pair.batch.fileCount &&
			this.matches(control) &&
			this.bytes === this.pair.batch.totalBytes
		) {
			this.sendControl({
				type: 'batch_received',
				count: this.completedFiles,
				totalBytes: this.bytes
			});
			this.complete();
			return;
		}
		throw new Error('invalid_control_order');
	}
	acceptFiles() {
		if (
			!this.live() ||
			this.files ||
			!this.manifestSealed ||
			this.receivingConsent ||
			this.phase !== 'review'
		)
			throw new Error('invalid_consent');
		this.receivingConsent = true;
		this.transferring();
		this.sendControl({
			type: 'receive_ready',
			count: this.pair.batch.fileCount,
			totalBytes: this.pair.batch.totalBytes
		});
	}
	private async binary(data: Uint8Array<ArrayBuffer>) {
		if (
			!this.receivingConsent ||
			!this.receivingFile ||
			!this.store ||
			this.fileBytes + data.byteLength > this.receivingFile.size ||
			this.bytes + data.byteLength > this.pair.batch.totalBytes
		)
			throw new Error('invalid_binary');
		await this.store.writeFile(data);
		if (!this.live()) return;
		this.bytes += data.byteLength;
		this.fileBytes += data.byteLength;
		this.touch();
		this.persisted();
		this.progress();
	}
	private persisted(force = false) {
		if (force || this.bytes - this.lastPersistedAck >= PERSISTED_ACK_BYTES) {
			this.sendControl({ type: 'persisted', bytes: this.bytes });
			this.lastPersistedAck = this.bytes;
		}
	}
	private async drained() {
		const channel = this.channel;
		if (!channel || channel.readyState !== 'open') throw new Error('channel_closed');
		if (channel.bufferedAmount <= BUFFER_HIGH_WATER - FILE_CHUNK_BYTES) return;
		await new Promise<void>((resolve, reject) => {
			const signal = this.controller.signal;
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
	private async sendFiles() {
		const files = this.files,
			channel = this.channel;
		if (!files || !channel) return;
		try {
			this.sendControl({
				type: 'manifest_start',
				count: files.length,
				totalBytes: this.pair.batch.totalBytes
			});
			for (let index = 0; index < files.length; index++) {
				await this.until(() => this.manifestSent - this.manifestAck < MANIFEST_WINDOW);
				await this.drained();
				this.sendControl({ type: 'manifest_file', index, file: fileMetadata(files[index]) });
				this.manifestSent++;
				this.touch();
			}
			await this.until(() => this.manifestAck === files.length);
			this.sendControl({
				type: 'manifest_end',
				count: files.length,
				totalBytes: this.pair.batch.totalBytes
			});
			await this.until(() => this.remoteReceiveReady);
			for (let index = 0; index < files.length; index++) {
				this.fileIndex = index;
				this.fileBytes = 0;
				this.phase = 'sending';
				this.sendControl({ type: 'file_start', index });
				this.progress(true);
				const file = files[index];
				for (let offset = 0; offset < file.size; offset += FILE_READ_BYTES) {
					const block = await file.slice(offset, offset + FILE_READ_BYTES).arrayBuffer();
					if (!this.live()) return;
					for (let start = 0; start < block.byteLength; start += FILE_CHUNK_BYTES) {
						const size = Math.min(FILE_CHUNK_BYTES, block.byteLength - start);
						await this.until(() => this.sentBytes - this.bytes + size <= BUFFER_HIGH_WATER);
						await this.drained();
						if (!this.live()) return;
						channel.send(new Uint8Array(block, start, size));
						this.sentBytes += size;
						this.fileBytes += size;
					}
				}
				if (!this.live()) return;
				this.phase = 'acknowledging';
				this.sendControl({ type: 'file_end', index, bytes: file.size });
				this.progress(true);
				await this.until(() => this.receivedFileAck === index);
				this.completedFiles++;
				this.progress(true);
			}
			if (!this.live()) return;
			this.awaitingBatch = true;
			this.phase = 'acknowledging';
			this.sendControl({ type: 'batch_end', count: files.length, totalBytes: this.sentBytes });
			this.progress(true);
		} catch {
			if (this.live()) this.fail('ส่งชุดไฟล์ไม่สำเร็จ ไฟล์ที่ผู้รับได้รับครบแล้วยังดาวน์โหลดได้');
		}
	}
}
