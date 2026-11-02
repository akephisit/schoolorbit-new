import type { components } from '../api/generated/school-api.js';

export type TransferLimits = components['schemas']['FileTransferLimits'];
export type BatchSummary = components['schemas']['FileTransferBatchSummary'];
export interface FileMetadata {
	name: string;
	size: number;
	mimeType: string;
}
export const FILE_TRANSFER_PROTOCOL = 3;
export const FILE_CHUNK_BYTES = 16 * 1024;
export const FILE_READ_BYTES = 1024 * 1024;
export const BUFFER_HIGH_WATER = 1024 * 1024;
export const BUFFER_LOW_WATER = 256 * 1024;
export const PERSISTED_ACK_BYTES = 256 * 1024;
export const MANIFEST_WINDOW = 16;
export const MANIFEST_FRAME_BYTES = 4096;
export const TRANSFER_PAGE_SIZE = 50;
export const DEFAULT_TRANSFER_LIMITS: TransferLimits = { maxOutgoingTransfers: 6 };
export const LAN_CONNECTION_HELP =
	'เชื่อมต่อโดยตรงไม่สำเร็จ ตรวจสอบว่าอุปกรณ์อยู่ใน Wi-Fi หรือ LAN เดียวกัน และเครือข่ายอนุญาตการเชื่อมต่อระหว่างอุปกรณ์ เครือข่ายผู้เยี่ยมชม การแยกอุปกรณ์ ไฟร์วอลล์ หรือ VPN อาจปิดกั้นได้ QR และรหัสช่วยจับคู่เท่านั้น ไม่สามารถแก้การปิดกั้นเครือข่ายได้';
export function fileMetadata(file: File): FileMetadata {
	return { name: file.name, size: file.size, mimeType: file.type };
}
export function batchBytes(files: readonly FileMetadata[]): number {
	return files.reduce((total, file) => total + file.size, 0);
}
export function batchSummary(files: readonly FileMetadata[]): BatchSummary {
	return { fileCount: files.length, totalBytes: batchBytes(files) };
}
export function sameBatch(a: BatchSummary, b: BatchSummary): boolean {
	return a.fileCount === b.fileCount && a.totalBytes === b.totalBytes;
}
export function sameFiles(a: readonly FileMetadata[], b: readonly FileMetadata[]): boolean {
	return (
		a.length === b.length &&
		a.every(
			(file, index) =>
				file.name === b[index].name &&
				file.size === b[index].size &&
				file.mimeType === b[index].mimeType
		)
	);
}
export function safeInteger(value: unknown): value is number {
	return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}
export function validateBatchSummary(batch: BatchSummary): string | null {
	return safeInteger(batch.fileCount) && batch.fileCount > 0 && safeInteger(batch.totalBytes)
		? null
		: 'จำนวนหรือขนาดรวมของไฟล์เกินช่วงตัวเลขที่เบราว์เซอร์รองรับ';
}
export function validateTransferFile(file: FileMetadata): string | null {
	if (
		!file.name.trim() ||
		file.name === '.' ||
		file.name === '..' ||
		[...file.name].length > 255 ||
		/[/\\\p{Cc}]/u.test(file.name)
	)
		return 'ชื่อไฟล์ต้องไม่เกิน 255 ตัวอักษร และไม่มีเครื่องหมายเส้นทางหรืออักขระควบคุม';
	if (!safeInteger(file.size)) return 'ขนาดไฟล์ต้องเป็นจำนวนเต็มที่เบราว์เซอร์รองรับ';
	if ([...file.mimeType].length > 128 || /[\p{Cc}]/u.test(file.mimeType))
		return 'ชนิดไฟล์ไม่ถูกต้อง';
	return null;
}
export function validateTransferBatch(
	files: readonly FileMetadata[],
	_limits?: TransferLimits
): string | null {
	if (!files.length) return 'กรุณาเลือกอย่างน้อย 1 ไฟล์';
	let total = 0;
	for (const file of files) {
		const error = validateTransferFile(file);
		if (error) return error;
		total += file.size;
		if (!safeInteger(total)) return 'ขนาดไฟล์รวมเกินช่วงตัวเลขที่เบราว์เซอร์รองรับ';
	}
	return null;
}
export function formatFileSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} ไบต์`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
	if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
	return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GiB`;
}
export function pairingCodeFromHash(hash: string): string {
	const code = new URLSearchParams(hash.replace(/^#/, '')).get('code') ?? '';
	return /^[A-Fa-f0-9]{10}$/.test(code) ? code.toUpperCase() : '';
}
export function pairingUrl(origin: string, pathname: string, code: string): string {
	return `${origin}${pathname}#${new URLSearchParams({ code })}`;
}
export type TransferControl =
	| ({
			type:
				| 'manifest_start'
				| 'manifest_end'
				| 'manifest_ready'
				| 'receive_ready'
				| 'batch_end'
				| 'batch_received';
	  } & { count: number; totalBytes: number })
	| { type: 'manifest_file'; index: number; file: FileMetadata }
	| { type: 'manifest_ack'; count: number }
	| { type: 'persisted'; bytes: number }
	| { type: 'file_start'; index: number }
	| { type: 'file_end' | 'file_received'; index: number; bytes: number };
export function parseTransferControl(text: string): TransferControl {
	if (
		text.length > MANIFEST_FRAME_BYTES ||
		new TextEncoder().encode(text).byteLength > MANIFEST_FRAME_BYTES
	)
		throw new Error('ข้อมูลควบคุมใหญ่เกินกำหนด');
	const value: unknown = JSON.parse(text);
	if (typeof value !== 'object' || value === null || !('type' in value))
		throw new Error('ข้อมูลควบคุมไม่ถูกต้อง');
	if (
		[
			'manifest_start',
			'manifest_end',
			'manifest_ready',
			'receive_ready',
			'batch_end',
			'batch_received'
		].includes(String(value.type)) &&
		'count' in value &&
		safeInteger(value.count) &&
		value.count > 0 &&
		'totalBytes' in value &&
		safeInteger(value.totalBytes)
	)
		return {
			type: value.type as 'manifest_start',
			count: value.count,
			totalBytes: value.totalBytes
		};
	if (value.type === 'manifest_ack' && 'count' in value && safeInteger(value.count))
		return { type: value.type, count: value.count };
	if (value.type === 'persisted' && 'bytes' in value && safeInteger(value.bytes))
		return { type: value.type, bytes: value.bytes };
	if ('index' in value && safeInteger(value.index)) {
		if (value.type === 'file_start') return { type: value.type, index: value.index };
		if (
			(value.type === 'file_end' || value.type === 'file_received') &&
			'bytes' in value &&
			safeInteger(value.bytes)
		)
			return { type: value.type, index: value.index, bytes: value.bytes };
		if (
			value.type === 'manifest_file' &&
			'file' in value &&
			typeof value.file === 'object' &&
			value.file !== null &&
			'name' in value.file &&
			typeof value.file.name === 'string' &&
			'size' in value.file &&
			safeInteger(value.file.size) &&
			'mimeType' in value.file &&
			typeof value.file.mimeType === 'string'
		) {
			const file = { name: value.file.name, size: value.file.size, mimeType: value.file.mimeType };
			if (!validateTransferFile(file)) return { type: value.type, index: value.index, file };
		}
	}
	throw new Error('ข้อมูลควบคุมไม่ถูกต้อง');
}
