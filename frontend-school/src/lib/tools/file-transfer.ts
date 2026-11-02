import type { components } from '../api/generated/school-api.js';

export type TransferLimits = components['schemas']['FileTransferLimits'];
export const FILE_TRANSFER_PROTOCOL = 2;
export const MAX_FILE_BYTES = 64 * 1024 * 1024;
export const MAX_BATCH_BYTES = 128 * 1024 * 1024;
export const MAX_BATCH_FILES = 20;
export const FILE_CHUNK_BYTES = 16 * 1024;
export const FILE_READ_BYTES = 1024 * 1024;
export const BUFFER_HIGH_WATER = 1024 * 1024;
export const BUFFER_LOW_WATER = 256 * 1024;
export const DEFAULT_TRANSFER_LIMITS: TransferLimits = {
	maxOutgoingTransfers: 3,
	maxFilesPerBatch: MAX_BATCH_FILES,
	maxFileBytes: MAX_FILE_BYTES,
	maxBatchBytes: MAX_BATCH_BYTES
};
export const LAN_CONNECTION_HELP =
	'เชื่อมต่อโดยตรงไม่สำเร็จ ตรวจสอบว่าอุปกรณ์อยู่ใน Wi-Fi หรือ LAN เดียวกัน และเครือข่ายอนุญาตการเชื่อมต่อระหว่างอุปกรณ์ เครือข่ายผู้เยี่ยมชม การแยกอุปกรณ์ ไฟร์วอลล์ หรือ VPN อาจปิดกั้นได้ QR และรหัสช่วยจับคู่เท่านั้น ไม่สามารถแก้การปิดกั้นเครือข่ายได้';
export type FileMetadata = components['schemas']['FileTransferFile'];
export function fileMetadata(file: File): FileMetadata {
	return { name: file.name, size: file.size, mimeType: file.type };
}
export function batchBytes(files: readonly FileMetadata[]): number {
	return files.reduce((total, file) => total + file.size, 0);
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
export function validateTransferFile(file: FileMetadata, maxBytes = MAX_FILE_BYTES): string | null {
	if (
		!file.name.trim() ||
		file.name === '.' ||
		file.name === '..' ||
		[...file.name].length > 255 ||
		/[/\\\p{Cc}]/u.test(file.name)
	)
		return 'ชื่อไฟล์ต้องไม่เกิน 255 ตัวอักษร และไม่มีเครื่องหมายเส้นทางหรืออักขระควบคุม';
	if (
		!Number.isSafeInteger(file.size) ||
		file.size < 0 ||
		file.size > Math.min(maxBytes, MAX_FILE_BYTES)
	)
		return `ไฟล์แต่ละไฟล์ต้องไม่เกิน ${formatFileSize(Math.min(maxBytes, MAX_FILE_BYTES))}`;
	if ([...file.mimeType].length > 128 || /[\p{Cc}]/u.test(file.mimeType))
		return 'ชนิดไฟล์ไม่ถูกต้อง';
	return null;
}
export function validateTransferBatch(
	files: readonly FileMetadata[],
	limits: TransferLimits = DEFAULT_TRANSFER_LIMITS
): string | null {
	if (!files.length || files.length > limits.maxFilesPerBatch || files.length > MAX_BATCH_FILES)
		return `เลือกได้ครั้งละ 1–${limits.maxFilesPerBatch} ไฟล์`;
	for (const file of files) {
		const error = validateTransferFile(file, limits.maxFileBytes);
		if (error) return error;
	}
	if (batchBytes(files) > Math.min(limits.maxBatchBytes, MAX_BATCH_BYTES))
		return `ขนาดไฟล์รวมต้องไม่เกิน ${formatFileSize(limits.maxBatchBytes)}`;
	return null;
}
export function formatFileSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} ไบต์`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
	return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}
export function pairingCodeFromHash(hash: string): string {
	const code = new URLSearchParams(hash.replace(/^#/, '')).get('code') ?? '';
	return /^[A-Fa-f0-9]{10}$/.test(code) ? code.toUpperCase() : '';
}
export function pairingUrl(origin: string, pathname: string, code: string): string {
	return `${origin}${pathname}#${new URLSearchParams({ code })}`;
}
export type TransferControl =
	| { type: 'file_start'; index: number }
	| { type: 'file_end' | 'file_received'; index: number; bytes: number }
	| { type: 'batch_end' | 'batch_received'; count: number; totalBytes: number };
function integer(value: unknown, max: number): value is number {
	return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= max;
}
export function parseTransferControl(text: string): TransferControl {
	if (text.length > 256) throw new Error('ข้อมูลควบคุมไม่ถูกต้อง');
	const value: unknown = JSON.parse(text);
	if (typeof value !== 'object' || value === null || !('type' in value))
		throw new Error('ข้อมูลควบคุมไม่ถูกต้อง');
	if (
		(value.type === 'file_start' || value.type === 'file_end' || value.type === 'file_received') &&
		'index' in value &&
		integer(value.index, MAX_BATCH_FILES - 1)
	) {
		if (value.type === 'file_start') return { type: value.type, index: value.index };
		if ('bytes' in value && integer(value.bytes, MAX_FILE_BYTES))
			return { type: value.type, index: value.index, bytes: value.bytes };
	}
	if (
		(value.type === 'batch_end' || value.type === 'batch_received') &&
		'count' in value &&
		integer(value.count, MAX_BATCH_FILES) &&
		value.count > 0 &&
		'totalBytes' in value &&
		integer(value.totalBytes, MAX_BATCH_BYTES)
	)
		return { type: value.type, count: value.count, totalBytes: value.totalBytes };
	throw new Error('ข้อมูลควบคุมไม่ถูกต้อง');
}
