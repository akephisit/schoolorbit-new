export const MAX_FILE_BYTES = 64 * 1024 * 1024;
export const FILE_CHUNK_BYTES = 16 * 1024;
export const BUFFER_HIGH_WATER = 256 * 1024;
export const BUFFER_LOW_WATER = 64 * 1024;
export const LAN_CONNECTION_HELP =
	'เชื่อมต่อโดยตรงไม่สำเร็จ ตรวจสอบว่าอุปกรณ์อยู่ใน Wi-Fi หรือ LAN เดียวกัน และเครือข่ายอนุญาตการเชื่อมต่อระหว่างอุปกรณ์ เครือข่ายผู้เยี่ยมชม การแยกอุปกรณ์ ไฟร์วอลล์ หรือ VPN อาจปิดกั้นได้ QR และรหัสช่วยจับคู่เท่านั้น ไม่สามารถแก้การปิดกั้นเครือข่ายได้';

export function validateTransferFile(file: {
	name: string;
	size: number;
	mimeType: string;
}): string | null {
	if (
		!file.name.trim() ||
		file.name === '.' ||
		file.name === '..' ||
		[...file.name].length > 255 ||
		/[/\\\p{Cc}]/u.test(file.name)
	)
		return 'ชื่อไฟล์ต้องไม่เกิน 255 ตัวอักษร และไม่มีเครื่องหมายเส้นทางหรืออักขระควบคุม';
	if (!Number.isSafeInteger(file.size) || file.size < 0 || file.size > MAX_FILE_BYTES)
		return 'ส่งได้ครั้งละ 1 ไฟล์ ขนาดไม่เกิน 64 MiB';
	if ([...file.mimeType].length > 128 || /[\p{Cc}]/u.test(file.mimeType))
		return 'ชนิดไฟล์ไม่ถูกต้อง';
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

export type TransferControl = { type: 'end'; bytes: number } | { type: 'received'; bytes: number };
export function parseTransferControl(text: string): TransferControl {
	if (text.length > 128) throw new Error('ข้อมูลควบคุมไม่ถูกต้อง');
	const value: unknown = JSON.parse(text);
	if (
		typeof value !== 'object' ||
		value === null ||
		!('type' in value) ||
		!('bytes' in value) ||
		(value.type !== 'end' && value.type !== 'received') ||
		typeof value.bytes !== 'number' ||
		!Number.isSafeInteger(value.bytes) ||
		value.bytes < 0 ||
		value.bytes > MAX_FILE_BYTES
	)
		throw new Error('ข้อมูลควบคุมไม่ถูกต้อง');
	return { type: value.type, bytes: value.bytes };
}
