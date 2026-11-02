export type ReceiveMode = 'folder' | 'browser-download';
export type ReceiveDestination =
	{ mode: 'folder'; directory: FileSystemDirectoryHandle } | { mode: 'browser-download' };
type DirectoryPicker = (options: {
	id: string;
	mode: 'readwrite';
}) => Promise<FileSystemDirectoryHandle>;
/** The picker is not yet declared by TypeScript's DOM library. */
declare global {
	interface Window {
		showDirectoryPicker?: DirectoryPicker;
	}
}
export function directoryPickerSupported(): boolean {
	return typeof window !== 'undefined' && typeof window.showDirectoryPicker === 'function';
}
/** Call directly in the Receive click stack, before any awaited preparation. */
export function pickReceiveDirectory(): Promise<FileSystemDirectoryHandle> {
	if (!directoryPickerSupported())
		return Promise.reject(
			new Error('เบราว์เซอร์นี้ไม่รองรับการเลือกโฟลเดอร์ กรุณาเลือกดาวน์โหลดผ่านเบราว์เซอร์')
		);
	return window.showDirectoryPicker!({ id: 'schoolorbit-receive-files', mode: 'readwrite' });
}
export function destinationSelectionMessage(error: unknown): string {
	if (error instanceof DOMException && error.name === 'AbortError')
		return 'ยกเลิกการเลือกโฟลเดอร์แล้ว ยังไม่ได้รับไฟล์ เลือกโฟลเดอร์อีกครั้ง หรือเลือกดาวน์โหลดผ่านเบราว์เซอร์ก่อนกดรับ';
	if (error instanceof DOMException && ['NotAllowedError', 'SecurityError'].includes(error.name))
		return 'ยังไม่ได้รับสิทธิ์เขียนโฟลเดอร์และยังไม่ได้รับไฟล์ เลือกโฟลเดอร์ที่เขียนได้ หรือเลือกดาวน์โหลดผ่านเบราว์เซอร์';
	return error instanceof Error && /^[ก-๛]/u.test(error.message)
		? error.message
		: 'เตรียมโฟลเดอร์รับไฟล์ไม่สำเร็จ ยังไม่ได้รับไฟล์ กรุณาเลือกโฟลเดอร์ใหม่ หรือเลือกดาวน์โหลดผ่านเบราว์เซอร์';
}
