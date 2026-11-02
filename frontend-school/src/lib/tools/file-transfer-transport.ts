import type { TransferPair, TransferSignal } from '../api/file-transfer.js';
import {
	batchBytes,
	BUFFER_HIGH_WATER,
	BUFFER_LOW_WATER,
	FILE_CHUNK_BYTES,
	FILE_READ_BYTES,
	LAN_CONNECTION_HELP,
	parseTransferControl,
	type TransferControl
} from './file-transfer.js';

export type TransportPhase = 'connecting' | 'sending' | 'receiving' | 'acknowledging' | 'complete';
export interface TransportProgress {
	phase: TransportPhase;
	bytes: number;
	fileIndex: number;
	fileBytes: number;
	completedFiles: number;
}
type Ports = {
	signal(type: 'offer' | 'answer', sdp: string, signal: AbortSignal): Promise<void>;
	progress(progress: TransportProgress): void;
	file(index: number, blob: Blob): void;
	complete(): void;
	failure(message: string): void;
	closed(): void;
};

/** One consenting pair owns its peer, read pump, counters and partial RAM buffers. */
export class FileTransferTransport {
	private controller = new AbortController();
	private peer: RTCPeerConnection;
	private channel: RTCDataChannel | null = null;
	private stopped = false;
	private completed = false;
	private started = false;
	private answered = false;
	private chunks: ArrayBuffer[] = [];
	private bytes = 0;
	private fileBytes = 0;
	private fileIndex = 0;
	private completedFiles = 0;
	private receivingFile = false;
	private expectedAck: {
		index: number;
		bytes: number;
		resolve(): void;
		reject(error: Error): void;
	} | null = null;
	private awaitingBatch = false;
	private phase: TransportPhase = 'connecting';
	private lastActivity = performance.now();
	private lastPaint = 0;
	private paintTimer: ReturnType<typeof setTimeout> | null = null;
	private watchdog: ReturnType<typeof setTimeout> | null = null;
	private connectTimer: ReturnType<typeof setTimeout> | null = null;
	constructor(
		private pair: TransferPair,
		private files: readonly File[] | null,
		private ports: Ports
	) {
		this.peer = new RTCPeerConnection({ iceServers: [] });
		this.connectTimer = setTimeout(() => this.fail(LAN_CONNECTION_HELP), 20_000);
		this.peer.onconnectionstatechange = () => {
			if (this.stopped) return;
			if (this.peer.connectionState === 'failed' || this.peer.connectionState === 'disconnected') {
				if (this.completed) this.finishClosed();
				else this.fail(LAN_CONNECTION_HELP);
			}
		};
		this.peer.ondatachannel = ({ channel }) => {
			if (
				this.files ||
				this.channel ||
				channel.label !== 'schoolorbit-files-v2' ||
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
		if (this.stopped) return;
		const emit = () => {
			this.paintTimer = null;
			this.lastPaint = performance.now();
			this.ports.progress({
				phase: this.phase,
				bytes: this.bytes,
				fileIndex: this.fileIndex,
				fileBytes: this.fileBytes,
				completedFiles: this.completedFiles
			});
		};
		if (force || performance.now() - this.lastPaint >= 100) {
			if (this.paintTimer !== null) clearTimeout(this.paintTimer);
			emit();
		} else if (this.paintTimer === null)
			this.paintTimer = setTimeout(emit, 100 - (performance.now() - this.lastPaint));
	}
	private watch() {
		if (this.stopped || this.completed) return;
		const remaining = 30_000 - (performance.now() - this.lastActivity);
		if (remaining <= 0) {
			this.fail('การรับส่งชุดไฟล์หยุดนานเกินไป กรุณาส่งคำขอใหม่');
			return;
		}
		this.watchdog = setTimeout(() => this.watch(), remaining);
	}
	private clearTimers() {
		if (this.paintTimer !== null) clearTimeout(this.paintTimer);
		if (this.watchdog !== null) clearTimeout(this.watchdog);
		if (this.connectTimer !== null) clearTimeout(this.connectTimer);
		this.paintTimer = this.watchdog = this.connectTimer = null;
	}
	dispose() {
		if (this.stopped) return;
		this.stopped = true;
		this.controller.abort();
		this.clearTimers();
		this.expectedAck?.reject(new Error('cancelled'));
		this.expectedAck = null;
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
		this.chunks = [];
		this.files = null;
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
		if (this.completed || this.stopped) return;
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
			this.attach(this.peer.createDataChannel('schoolorbit-files-v2', { ordered: true }));
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
			this.phase = this.files ? 'sending' : 'receiving';
			this.touch();
			this.watch();
			this.progress(true);
			if (this.files) void this.sendFiles();
		};
		channel.onopen = open;
		channel.onclose = () => {
			if (this.completed) this.finishClosed();
			else this.fail('ช่องทางปิดก่อนรับชุดไฟล์ครบ กรุณาส่งใหม่');
		};
		channel.onerror = () => this.fail(LAN_CONNECTION_HELP);
		channel.onmessage = (event: MessageEvent<unknown>) => {
			if (!this.live()) return;
			open();
			try {
				this.touch();
				if (typeof event.data === 'string') this.control(parseTransferControl(event.data));
				else {
					const expected = this.pair.files[this.fileIndex];
					if (
						this.files ||
						!this.receivingFile ||
						!(event.data instanceof ArrayBuffer) ||
						!event.data.byteLength ||
						event.data.byteLength > FILE_CHUNK_BYTES ||
						!expected ||
						this.fileBytes + event.data.byteLength > expected.size ||
						this.bytes + event.data.byteLength > batchBytes(this.pair.files)
					)
						throw new Error('invalid_chunk');
					this.chunks.push(event.data);
					this.fileBytes += event.data.byteLength;
					this.bytes += event.data.byteLength;
					this.progress();
				}
			} catch {
				this.fail('ข้อมูลชุดไฟล์ไม่ครบหรือไม่ถูกต้อง ยกเลิกส่วนที่ยังรับไม่ครบแล้ว');
			}
		};
		if (channel.readyState === 'open') open();
	}
	private sendControl(control: TransferControl) {
		if (!this.channel || this.channel.readyState !== 'open') throw new Error('channel_closed');
		this.channel.send(JSON.stringify(control));
		this.touch();
	}
	private control(control: TransferControl) {
		if (this.completed) throw new Error('batch_already_complete');
		if (this.files) {
			if (
				control.type === 'file_received' &&
				this.expectedAck &&
				control.index === this.expectedAck.index &&
				control.bytes === this.expectedAck.bytes
			) {
				const ack = this.expectedAck;
				this.expectedAck = null;
				ack.resolve();
				return;
			}
			if (
				control.type === 'batch_received' &&
				this.awaitingBatch &&
				control.count === this.pair.files.length &&
				control.totalBytes === batchBytes(this.pair.files)
			) {
				this.complete();
				return;
			}
			throw new Error('invalid_ack');
		}
		const file = this.pair.files[this.fileIndex];
		if (
			control.type === 'file_start' &&
			!this.receivingFile &&
			control.index === this.fileIndex &&
			file
		) {
			this.receivingFile = true;
			this.fileBytes = 0;
			this.chunks = [];
			this.progress(true);
			return;
		}
		if (
			control.type === 'file_end' &&
			this.receivingFile &&
			file &&
			control.index === this.fileIndex &&
			control.bytes === file.size &&
			this.fileBytes === file.size
		) {
			const index = this.fileIndex,
				blob = new Blob(this.chunks, { type: 'application/octet-stream' });
			this.chunks = [];
			this.receivingFile = false;
			this.completedFiles++;
			this.fileIndex++;
			this.ports.file(index, blob);
			this.sendControl({ type: 'file_received', index, bytes: file.size });
			this.progress(true);
			return;
		}
		if (
			control.type === 'batch_end' &&
			!this.receivingFile &&
			this.fileIndex === this.pair.files.length &&
			control.count === this.pair.files.length &&
			control.totalBytes === this.bytes &&
			this.bytes === batchBytes(this.pair.files)
		) {
			this.sendControl({ type: 'batch_received', count: control.count, totalBytes: this.bytes });
			this.complete();
			return;
		}
		throw new Error('invalid_control_order');
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
						if (channel.bufferedAmount > BUFFER_HIGH_WATER - FILE_CHUNK_BYTES) await this.drained();
						if (!this.live()) return;
						const chunk = new Uint8Array(
							block,
							start,
							Math.min(FILE_CHUNK_BYTES, block.byteLength - start)
						);
						channel.send(chunk);
						this.fileBytes += chunk.byteLength;
						this.bytes += chunk.byteLength;
						this.touch();
						this.progress();
					}
				}
				if (!this.live()) return;
				this.phase = 'acknowledging';
				this.progress(true);
				await new Promise<void>((resolve, reject) => {
					this.expectedAck = { index, bytes: file.size, resolve, reject };
					this.sendControl({ type: 'file_end', index, bytes: file.size });
				});
				if (!this.live()) return;
				this.completedFiles++;
				this.progress(true);
			}
			if (!this.live()) return;
			this.awaitingBatch = true;
			this.phase = 'acknowledging';
			this.sendControl({ type: 'batch_end', count: files.length, totalBytes: this.bytes });
			this.progress(true);
		} catch {
			if (this.live()) this.fail('ส่งชุดไฟล์ไม่สำเร็จ ไฟล์ที่ผู้รับได้รับครบแล้วยังดาวน์โหลดได้');
		}
	}
}
