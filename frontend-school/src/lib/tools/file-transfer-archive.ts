import type { ReceivedBatchStore } from './file-transfer-storage.js';
import { checkStorageSpace } from './file-transfer-storage.js';

export { numberedFilename as archiveFilename } from './file-transfer.js';
import { numberedFilename } from './file-transfer.js';
/** Contents stream one file at a time; zip.js retains central-directory metadata. */
export async function createTransferArchive(
	store: ReceivedBatchStore,
	signal: AbortSignal,
	progress: (bytes: number) => void
): Promise<File> {
	const count = store.completedCount;
	if (!count) throw new Error('ไม่มีไฟล์ที่รับครบสำหรับดาวน์โหลด');
	await checkStorageSpace(store.completedBytes + count * 2048 + 1024);
	if (signal.aborted) throw new DOMException('Cancelled', 'AbortError');
	const { ZipWriter, BlobReader } = await import('@zip.js/zip.js');
	const handle = await store.archiveTarget();
	const writable = await handle.createWritable();
	let closed = false;
	try {
		const writer = new ZipWriter(writable, {
			zip64: true,
			level: 0,
			useWebWorkers: false,
			bufferedWrite: false,
			signal,
			preventClose: true
		});
		let bytes = 0;
		for (let index = 0; index < count; index++) {
			if (signal.aborted) throw new DOMException('Cancelled', 'AbortError');
			const metadata = await store.metadata(index),
				file = await store.getCompletedFile(index);
			await writer.add(numberedFilename(index, count, metadata.name), new BlobReader(file), {
				signal,
				onprogress: (written) => progress(bytes + written)
			});
			bytes += file.size;
			progress(bytes);
		}
		if (signal.aborted) throw new DOMException('Cancelled', 'AbortError');
		await writer.close(undefined, { zip64: true, preventClose: true });
		if (signal.aborted) throw new DOMException('Cancelled', 'AbortError');
		await writable.close();
		closed = true;
		return await handle.getFile();
	} catch (error) {
		if (!closed) {
			try {
				await writable.abort();
			} catch (abortError) {
				if (!(abortError instanceof TypeError)) console.warn('File transfer archive abort failed');
			}
		}
		try {
			await store.removeArchive();
		} catch {
			console.warn('File transfer partial archive cleanup failed');
		}
		throw error;
	}
}
