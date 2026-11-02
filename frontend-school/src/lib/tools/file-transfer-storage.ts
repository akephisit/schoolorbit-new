import type { ReceiveDestination, ReceiveMode } from './file-transfer-destination.js';
import {
	TRANSFER_PAGE_SIZE,
	validateTransferFile,
	sameBatch,
	numberedFilename,
	type BatchSummary,
	type FileMetadata
} from './file-transfer.js';

const STORAGE_ROOT = 'schoolorbit-file-transfer-v3';
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
export class TransferStorageError extends Error {}
export function storageMessage(error: unknown): string {
	if (error instanceof TransferStorageError) return error.message;
	if (error instanceof DOMException && error.name === 'QuotaExceededError')
		return 'พื้นที่จัดเก็บปลายทางไม่พอ ดาวน์โหลดและล้างไฟล์เก่า หรือเพิ่มพื้นที่ว่างแล้วลองใหม่';
	if (error instanceof DOMException && ['SecurityError', 'NotAllowedError'].includes(error.name))
		return 'เบราว์เซอร์ไม่อนุญาตให้เขียนพื้นที่ปลายทาง กรุณาตรวจสอบสิทธิ์โฟลเดอร์ หรือเลือกดาวน์โหลดผ่านเบราว์เซอร์ในคำขอใหม่';
	return 'เขียนไฟล์ลงพื้นที่ปลายทางไม่สำเร็จ ตรวจสอบพื้นที่ว่างและสิทธิ์แล้วลองใหม่ ไฟล์ที่รับครบแล้วยังอยู่';
}
export function receivingSupported(): boolean {
	return (
		typeof navigator !== 'undefined' &&
		typeof navigator.storage?.getDirectory === 'function' &&
		typeof navigator.locks?.request === 'function' &&
		typeof FileSystemFileHandle !== 'undefined' &&
		typeof FileSystemFileHandle.prototype.createWritable === 'function'
	);
}
export const RECEIVING_UNSUPPORTED =
	'เบราว์เซอร์นี้ยังไม่รองรับพื้นที่เก็บไฟล์สำหรับการรับ กรุณาใช้ Chrome หรือ Edge รุ่นปัจจุบันผ่าน HTTPS อุปกรณ์นี้ยังส่งไฟล์ได้';
export async function checkStorageSpace(bytes: number): Promise<string | null> {
	try {
		const { quota, usage } = await navigator.storage.estimate();
		return quota !== undefined && usage !== undefined && quota - usage < bytes
			? 'พื้นที่ว่างที่เบราว์เซอร์ประมาณไว้อาจไม่พอ ระบบจะลองเขียนไฟล์จริง หากพื้นที่เต็มจะหยุดและเก็บไฟล์ที่รับครบแล้วไว้'
			: null;
	} catch {
		return 'ตรวจสอบพื้นที่ว่างล่วงหน้าไม่ได้ ระบบจะลองเขียนไฟล์จริงและแจ้งหากพื้นที่ไม่พอ';
	}
}
/** Only namespaces whose exclusive ownership lock is available may be collected. */
export async function collectAbandonedTransferStorage(): Promise<void> {
	if (!receivingSupported()) return;
	const root = await (
		await navigator.storage.getDirectory()
	).getDirectoryHandle(STORAGE_ROOT, { create: true });
	let collected = 0;
	for await (const [name, entry] of root.entries()) {
		if (entry.kind !== 'directory' || !/^[a-f0-9-]{36}$/.test(name)) continue;
		await navigator.locks.request(
			`${STORAGE_ROOT}:${name}`,
			{ ifAvailable: true },
			async (lock) => {
				if (!lock) return;
				await root.removeEntry(name, { recursive: true });
				collected++;
			}
		);
		if (collected >= 32) break;
	}
}
export interface StoredMetadata extends FileMetadata {
	index: number;
}
/** A parent-retained disk owner. The transport may abort only its incomplete file. */
export class ReceivedBatchStore {
	readonly namespace = crypto.randomUUID();
	completedCount = 0;
	completedBytes = 0;
	manifestCount = 0;
	private manifestBytes = 0;
	private metadataBytes = 0;
	private sealed = false;
	private disposed = false;
	private releaseLock: (() => void) | null = null;
	private lockTask: Promise<unknown> | null = null;
	private root: FileSystemDirectoryHandle | null = null;
	private directory: FileSystemDirectoryHandle | null = null;
	private payloadDirectory: FileSystemDirectoryHandle | null = null;
	destinationMode: ReceiveMode = 'browser-download';
	destinationLabel = 'ดาวน์โหลดผ่านเบราว์เซอร์';
	private partialCleanup: Promise<void> | null = null;
	private manifest: FileSystemWritableFileStream | null = null;
	private offsets: FileSystemWritableFileStream | null = null;
	private partial: {
		index: number;
		stream: FileSystemWritableFileStream | null;
		name: string;
		created: boolean;
		closed: boolean;
		bytes: number;
		cancelled: boolean;
	} | null = null;
	private active = new Set<Promise<unknown>>();
	private disposal: Promise<void> | null = null;
	readonly batch: BatchSummary;
	private constructor(batch: BatchSummary) {
		this.batch = batch;
	}
	static async open(batch: BatchSummary): Promise<ReceivedBatchStore> {
		if (!receivingSupported()) throw new TransferStorageError(RECEIVING_UNSUPPORTED);
		const store = new ReceivedBatchStore(batch);
		try {
			await new Promise<void>((resolve, reject) => {
				store.lockTask = navigator.locks.request(`${STORAGE_ROOT}:${store.namespace}`, async () => {
					await new Promise<void>((release) => {
						store.releaseLock = release;
						resolve();
					});
				});
				void store.lockTask.catch(reject);
			});
			store.root = await (
				await navigator.storage.getDirectory()
			).getDirectoryHandle(STORAGE_ROOT, { create: true });
			store.directory = await store.root.getDirectoryHandle(store.namespace, { create: true });
			store.payloadDirectory = store.directory;
			store.manifest = await (
				await store.directory.getFileHandle('manifest', { create: true })
			).createWritable();
			store.offsets = await (
				await store.directory.getFileHandle('offsets', { create: true })
			).createWritable();
			return store;
		} catch (error) {
			await store.dispose();
			throw new TransferStorageError(storageMessage(error));
		}
	}
	private live(): FileSystemDirectoryHandle {
		if (this.disposed || !this.directory) throw new DOMException('Cancelled', 'AbortError');
		return this.directory;
	}
	private async operation<T>(work: () => Promise<T>): Promise<T> {
		this.live();
		const pending = work();
		this.active.add(pending);
		try {
			const result = await pending;
			this.live();
			return result;
		} catch (error) {
			if (error instanceof DOMException && error.name === 'AbortError') throw error;
			throw new TransferStorageError(storageMessage(error));
		} finally {
			this.active.delete(pending);
		}
	}
	async appendManifest(index: number, file: FileMetadata): Promise<void> {
		if (
			this.sealed ||
			index !== this.manifestCount ||
			index >= this.batch.fileCount ||
			validateTransferFile(file) ||
			!this.manifest ||
			!this.offsets
		)
			throw new Error('invalid_manifest_order');
		const record = encoder.encode(JSON.stringify(file) + '\n');
		if (
			record.byteLength > 4096 ||
			!Number.isSafeInteger(this.metadataBytes + record.byteLength) ||
			!Number.isSafeInteger(this.manifestBytes + file.size) ||
			this.manifestBytes + file.size > this.batch.totalBytes
		)
			throw new Error('invalid_manifest_size');
		const offset = new Uint8Array(8);
		new DataView(offset.buffer).setBigUint64(0, BigInt(this.metadataBytes), true);
		await this.operation(async () => {
			await this.manifest!.write(record);
			await this.offsets!.write(offset);
			this.manifestCount++;
			this.manifestBytes += file.size;
			this.metadataBytes += record.byteLength;
		});
	}
	async sealManifest(batch: BatchSummary): Promise<void> {
		if (
			this.sealed ||
			!sameBatch(batch, this.batch) ||
			this.manifestCount !== batch.fileCount ||
			this.manifestBytes !== batch.totalBytes ||
			!this.manifest ||
			!this.offsets
		)
			throw new Error('invalid_manifest_total');
		await this.operation(async () => {
			await this.manifest!.close();
			this.manifest = null;
			await this.offsets!.close();
			this.offsets = null;
			this.sealed = true;
		});
	}
	async metadata(index: number): Promise<FileMetadata> {
		if (!this.sealed || !Number.isSafeInteger(index) || index < 0 || index >= this.manifestCount)
			throw new Error('invalid_manifest_index');
		return this.operation(async () => {
			const directory = this.live();
			const offsets = await (await directory.getFileHandle('offsets')).getFile();
			const data = await offsets.slice(index * 8, (index + 2) * 8).arrayBuffer();
			const view = new DataView(data),
				start = Number(view.getBigUint64(0, true)),
				end = data.byteLength >= 16 ? Number(view.getBigUint64(8, true)) : this.metadataBytes;
			if (end - start > 4096 || end < start) throw new Error('invalid_manifest_offset');
			const file = await (await directory.getFileHandle('manifest')).getFile();
			const value: unknown = JSON.parse(decoder.decode(await file.slice(start, end).arrayBuffer()));
			if (
				!value ||
				typeof value !== 'object' ||
				!('name' in value) ||
				typeof value.name !== 'string' ||
				!('size' in value) ||
				typeof value.size !== 'number' ||
				!('mimeType' in value) ||
				typeof value.mimeType !== 'string'
			)
				throw new Error('invalid_manifest');
			const result = { name: value.name, size: value.size, mimeType: value.mimeType };
			if (validateTransferFile(result)) throw new Error('invalid_manifest');
			return result;
		});
	}
	async page(
		start: number,
		count = TRANSFER_PAGE_SIZE,
		completedOnly = false
	): Promise<StoredMetadata[]> {
		const limit = completedOnly ? this.completedCount : this.manifestCount;
		if (
			!Number.isSafeInteger(start) ||
			start < 0 ||
			!Number.isInteger(count) ||
			count < 1 ||
			count > TRANSFER_PAGE_SIZE
		)
			throw new Error('invalid_page');
		const result: StoredMetadata[] = [];
		for (let index = start; index < Math.min(limit, start + count); index++)
			result.push({ ...(await this.metadata(index)), index });
		return result;
	}
	async configureDestination(destination: ReceiveDestination, signal: AbortSignal): Promise<void> {
		if (!this.sealed || this.partial || this.completedCount)
			throw new Error('destination_already_receiving');
		if (signal.aborted) throw new DOMException('Cancelled', 'AbortError');
		if (destination.mode === 'browser-download') {
			this.live();
			this.payloadDirectory = this.directory;
			this.destinationMode = destination.mode;
			this.destinationLabel = 'ดาวน์โหลดผ่านเบราว์เซอร์';
			return;
		}
		await this.operation(async () => {
			const name = `SchoolOrbit-${crypto.randomUUID()}`;
			try {
				await destination.directory.getDirectoryHandle(name);
				throw new TransferStorageError('พบโฟลเดอร์ชื่อนี้อยู่แล้ว กรุณาเลือกปลายทางใหม่');
			} catch (error) {
				if (!(error instanceof DOMException && error.name === 'NotFoundError')) throw error;
			}
			if (signal.aborted || this.disposed) throw new DOMException('Cancelled', 'AbortError');
			const directory = await destination.directory.getDirectoryHandle(name, { create: true });
			if (signal.aborted || this.disposed) {
				await destination.directory.removeEntry(name);
				throw new DOMException('Cancelled', 'AbortError');
			}
			this.payloadDirectory = directory;
			this.destinationMode = 'folder';
			this.destinationLabel = name;
		});
	}
	async beginFile(index: number): Promise<void> {
		if (!this.sealed || this.partial || index !== this.completedCount || !this.payloadDirectory)
			throw new Error('invalid_file_order');
		const name =
			this.destinationMode === 'folder'
				? numberedFilename(index, this.batch.fileCount, (await this.metadata(index)).name)
				: `file-${index}`;
		this.live();
		const directory = this.payloadDirectory;
		const partial = {
			index,
			name,
			stream: null as FileSystemWritableFileStream | null,
			bytes: 0,
			cancelled: false,
			created: false,
			closed: false
		};
		// Track the new path before createWritable can reject after file creation.
		this.partial = partial;
		await this.operation(async () => {
			try {
				await directory.getFileHandle(name);
				throw new TransferStorageError(
					'มีไฟล์ชื่อนี้ในปลายทางแล้ว ยกเลิกเพื่อป้องกันการเขียนทับไฟล์เดิม'
				);
			} catch (error) {
				if (!(error instanceof DOMException && error.name === 'NotFoundError')) throw error;
			}
			if (partial.cancelled || this.disposed) throw new DOMException('Cancelled', 'AbortError');
			const handle = await directory.getFileHandle(name, { create: true });
			partial.created = true;
			partial.stream = await handle.createWritable();
			if (partial.cancelled || this.disposed) throw new DOMException('Cancelled', 'AbortError');
		});
	}
	async writeFile(bytes: Uint8Array<ArrayBuffer>): Promise<void> {
		const partial = this.partial;
		if (!partial?.stream || partial.cancelled) throw new Error('missing_file');
		const stream = partial.stream;
		await this.operation(async () => {
			await stream.write(bytes);
			partial.bytes += bytes.byteLength;
		});
	}
	async completeFile(index: number, size: number): Promise<void> {
		const partial = this.partial;
		if (
			!partial?.stream ||
			partial.index !== index ||
			partial.bytes !== size ||
			index !== this.completedCount
		)
			throw new Error('incomplete_file');
		const stream = partial.stream;
		await this.operation(async () => {
			await stream.close();
			partial.closed = true;
			if (partial.cancelled || this.disposed) throw new DOMException('Cancelled', 'AbortError');
			this.partial = null;
			this.completedCount++;
			this.completedBytes += size;
		});
	}
	abortPartial(): Promise<void> {
		if (this.partial) this.partial.cancelled = true;
		if (this.partialCleanup) return this.partialCleanup;
		this.partialCleanup = (async () => {
			await Promise.allSettled([...this.active]);
			const partial = this.partial;
			this.partial = null;
			if (!partial || !this.payloadDirectory) return;
			const failures: unknown[] = [];
			if (partial.stream && !partial.closed) {
				try {
					await partial.stream.abort();
				} catch (error) {
					failures.push(error);
				}
			}
			if (partial.created) {
				try {
					await this.payloadDirectory.removeEntry(partial.name);
				} catch (error) {
					if (!(error instanceof DOMException && error.name === 'NotFoundError'))
						failures.push(error);
				}
			}
			if (failures.length) throw new TransferStorageError(storageMessage(failures[0]));
		})().finally(() => {
			this.partialCleanup = null;
		});
		return this.partialCleanup;
	}
	async getCompletedFile(index: number): Promise<File> {
		if (
			!Number.isSafeInteger(index) ||
			index < 0 ||
			index >= this.completedCount ||
			!this.payloadDirectory
		)
			throw new Error('incomplete_file');
		const metadata = await this.metadata(index),
			directory = this.payloadDirectory;
		const name =
			this.destinationMode === 'folder'
				? numberedFilename(index, this.batch.fileCount, metadata.name)
				: `file-${index}`;
		return this.operation(async () => {
			const file = await (await directory.getFileHandle(name)).getFile();
			return new File([file], metadata.name, {
				type: metadata.mimeType || 'application/octet-stream',
				lastModified: file.lastModified
			});
		});
	}
	async archiveTarget(): Promise<FileSystemFileHandle> {
		return this.operation(() => this.live().getFileHandle('archive.zip', { create: true }));
	}
	async removeArchive(): Promise<void> {
		try {
			await this.live().removeEntry('archive.zip');
		} catch (error) {
			if (!(error instanceof DOMException && error.name === 'NotFoundError')) throw error;
		}
	}
	dispose(): Promise<void> {
		if (this.disposal) return this.disposal;
		this.disposed = true;
		if (this.partial) this.partial.cancelled = true;
		this.disposal = (async () => {
			await Promise.allSettled([...this.active]);
			const failures: unknown[] = [];
			try {
				await this.abortPartial();
			} catch (error) {
				failures.push(error);
			}
			for (const stream of [this.manifest, this.offsets])
				if (stream) {
					try {
						await stream.abort();
					} catch (error) {
						if (!(error instanceof TypeError)) failures.push(error);
					}
				}
			this.manifest = this.offsets = null;
			this.partial = null;
			try {
				if (this.root) await this.root.removeEntry(this.namespace, { recursive: true });
			} catch (error) {
				if (!(error instanceof DOMException && error.name === 'NotFoundError'))
					failures.push(error);
			} finally {
				this.directory = null;
				this.payloadDirectory = null;
				this.releaseLock?.();
				this.releaseLock = null;
				await this.lockTask;
			}
			if (failures.length) throw new TransferStorageError(storageMessage(failures[0]));
		})();
		return this.disposal;
	}
}
