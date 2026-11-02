import assert from 'node:assert/strict';
import test from 'node:test';
import { registerHooks } from 'node:module';

registerHooks({
	resolve(specifier, context, next) {
		try {
			return next(specifier, context);
		} catch (error) {
			if (specifier.endsWith('.js') && context.parentURL?.includes('/file-transfer-'))
				return next(specifier.slice(0, -3) + '.ts', context);
			throw error;
		}
	}
});
const { ReceivedBatchStore, collectAbandonedTransferStorage, checkStorageSpace } =
	await import('../../src/lib/tools/file-transfer-storage.ts');
const { archiveFilename, createTransferArchive } =
	await import('../../src/lib/tools/file-transfer-archive.ts');

class MemoryFile {
	bytes = new Uint8Array();
	closeGate = null;
	writeError = null;
	aborts = 0;
	constructor(name) {
		this.name = name;
	}
	async getFile() {
		return new File([this.bytes], this.name);
	}
	async createWritable() {
		let chunks = [];
		const stream = new WritableStream({
			write: async (chunk) => {
				if (this.writeError) throw this.writeError;
				chunks.push(new Uint8Array(chunk));
			},
			close: async () => {
				if (this.closeGate) await this.closeGate;
				this.bytes = new Uint8Array(Buffer.concat(chunks));
			},
			abort: () => {
				chunks = [];
			}
		});
		stream.write = async (chunk) => {
			const writer = stream.getWriter();
			try {
				await writer.write(chunk);
			} finally {
				writer.releaseLock();
			}
		};
		stream.close = async () => {
			const writer = stream.getWriter();
			try {
				await writer.close();
			} finally {
				writer.releaseLock();
			}
		};
		const abort = stream.abort.bind(stream);
		stream.abort = async (reason) => {
			this.aborts++;
			return abort(reason);
		};
		return stream;
	}
}
class MemoryDirectory {
	kind = 'directory';
	entriesMap = new Map();
	async getDirectoryHandle(name, { create = false } = {}) {
		if (!this.entriesMap.has(name) && create) this.entriesMap.set(name, new MemoryDirectory());
		if (!this.entriesMap.has(name)) throw new DOMException('missing', 'NotFoundError');
		return this.entriesMap.get(name);
	}
	async getFileHandle(name, { create = false } = {}) {
		if (!this.entriesMap.has(name) && create) this.entriesMap.set(name, new MemoryFile(name));
		if (!this.entriesMap.has(name)) throw new DOMException('missing', 'NotFoundError');
		return this.entriesMap.get(name);
	}
	async removeEntry(name) {
		if (!this.entriesMap.delete(name)) throw new DOMException('missing', 'NotFoundError');
	}
	async *entries() {
		yield* this.entriesMap.entries();
	}
}
function storageHarness() {
	const root = new MemoryDirectory(),
		held = new Set();
	const previousNavigator = Object.getOwnPropertyDescriptor(globalThis, 'navigator'),
		previousHandle = globalThis.FileSystemFileHandle;
	Object.defineProperty(globalThis, 'navigator', {
		configurable: true,
		value: {
			storage: { getDirectory: async () => root, estimate: async () => ({ quota: 1, usage: 1 }) },
			locks: {
				request: async (name, options, callback) => {
					if (typeof options === 'function') {
						callback = options;
						options = {};
					}
					if (held.has(name)) {
						if (options.ifAvailable) return callback(null);
						throw new Error('unexpected lock contention');
					}
					held.add(name);
					try {
						return await callback({ name });
					} finally {
						held.delete(name);
					}
				}
			}
		}
	});
	globalThis.FileSystemFileHandle = MemoryFile;
	return {
		root,
		held,
		async namespace(store) {
			return (await root.getDirectoryHandle('schoolorbit-file-transfer-v3')).getDirectoryHandle(
				store.namespace
			);
		},
		restore() {
			if (previousNavigator) Object.defineProperty(globalThis, 'navigator', previousNavigator);
			else delete globalThis.navigator;
			if (previousHandle) globalThis.FileSystemFileHandle = previousHandle;
			else delete globalThis.FileSystemFileHandle;
		}
	};
}
const metadata = { name: 'ซ้ำ.txt', size: 3, mimeType: 'text/plain' };
async function manifest(store, files = [metadata]) {
	for (const [index, file] of files.entries()) await store.appendManifest(index, file);
	await store.sealManifest({
		fileCount: files.length,
		totalBytes: files.reduce((n, f) => n + f.size, 0)
	});
}
test('held close cancellation never advertises a deleted completed file; earlier committed files survive', async () => {
	const h = storageHarness();
	let store;
	try {
		store = await ReceivedBatchStore.open({ fileCount: 2, totalBytes: 6 });
		await manifest(store, [metadata, metadata]);
		await store.beginFile(0);
		await store.writeFile(new Uint8Array([1, 2, 3]));
		await store.completeFile(0, 3);
		await store.beginFile(1);
		await store.writeFile(new Uint8Array([4, 5, 6]));
		const directory = await h.namespace(store),
			file = await directory.getFileHandle('file-1');
		let release;
		file.closeGate = new Promise((resolve) => {
			release = resolve;
		});
		const closing = store.completeFile(1, 3);
		const rejected = assert.rejects(closing, { name: 'AbortError' });
		const aborting = store.abortPartial();
		release();
		await rejected;
		await aborting;
		assert.equal(store.completedCount, 1);
		assert.equal(store.completedBytes, 3);
		assert.deepEqual(
			new Uint8Array(await (await store.getCompletedFile(0)).arrayBuffer()),
			new Uint8Array([1, 2, 3])
		);
		await assert.rejects(directory.getFileHandle('file-1'), { name: 'NotFoundError' });
	} finally {
		await store?.dispose();
		h.restore();
	}
});
test('sealed manifest pages remain ordered, quota estimates are advisory and locks protect active tabs', async () => {
	const h = storageHarness();
	let a, b;
	try {
		a = await ReceivedBatchStore.open({ fileCount: 51, totalBytes: 0 });
		b = await ReceivedBatchStore.open({ fileCount: 1, totalBytes: 0 });
		await manifest(
			a,
			Array.from({ length: 51 }, (_, index) => ({ ...metadata, name: `${index}.txt`, size: 0 }))
		);
		assert.deepEqual(
			(await a.page(50)).map((f) => f.name),
			['50.txt']
		);
		assert.ok(await checkStorageSpace(9999));
		await collectAbandonedTransferStorage();
		assert.ok(await h.namespace(a));
		assert.ok(await h.namespace(b));
		assert.equal(h.held.size, 2);
		await a.dispose();
		a = null;
		assert.equal(h.held.size, 1);
	} finally {
		await a?.dispose();
		await b?.dispose();
		assert.equal(h.held.size, 0);
		h.restore();
	}
});
test('archive names stay unique, flat and within filesystem length including Unicode and extension', () => {
	const names = ['ซ้ำ.txt', 'ซ้ำ.txt', 'CON', 'a:b?.txt', '😀'.repeat(255) + '.pdf'];
	const output = names.map((name, index) => archiveFilename(index, names.length, name));
	assert.equal(new Set(output).size, names.length);
	for (const name of output) {
		assert.ok(name.length <= 255);
		assert.equal(/[\\/<>:"|?*]/.test(name), false);
		assert.equal(/[\uD800-\uDBFF]$/.test(name), false);
	}
	assert.ok(output.at(-1).endsWith('.pdf'));
});
test('streaming ZIP cancellation removes only partial archive and preserves received originals', async () => {
	const h = storageHarness();
	let store;
	try {
		store = await ReceivedBatchStore.open({ fileCount: 1, totalBytes: 3 });
		await manifest(store);
		await store.beginFile(0);
		await store.writeFile(new Uint8Array([1, 2, 3]));
		await store.completeFile(0, 3);
		const controller = new AbortController();
		await assert.rejects(
			createTransferArchive(store, controller.signal, () => controller.abort()),
			{ name: 'AbortError' }
		);
		assert.equal(store.completedCount, 1);
		assert.equal((await store.getCompletedFile(0)).size, 3);
		await assert.rejects((await h.namespace(store)).getFileHandle('archive.zip'), {
			name: 'NotFoundError'
		});
	} finally {
		await store?.dispose();
		h.restore();
	}
});

test('external completed files survive disposal and failed writable creation removes only its new partial', async () => {
	const h = storageHarness(),
		destination = new MemoryDirectory();
	let store;
	try {
		const sentinel = await destination.getFileHandle('existing.txt', { create: true });
		sentinel.bytes = new Uint8Array([9]);
		store = await ReceivedBatchStore.open({ fileCount: 2, totalBytes: 6 });
		await manifest(store, [metadata, metadata]);
		await store.configureDestination(
			{ mode: 'folder', directory: destination },
			new AbortController().signal
		);
		const folder = [...destination.entriesMap.values()].find(
			(entry) => entry instanceof MemoryDirectory
		);
		assert.ok(folder);
		await store.beginFile(0);
		await store.writeFile(new Uint8Array([1, 2, 3]));
		await store.completeFile(0, 3);
		const saved = await store.getCompletedFile(0);
		assert.equal(saved.name, metadata.name);
		assert.equal(saved.type, metadata.mimeType);
		const getFile = folder.getFileHandle.bind(folder);
		folder.getFileHandle = async (name, options) => {
			const file = await getFile(name, options);
			if (options?.create && name.startsWith('2 - '))
				file.createWritable = async () => {
					throw new DOMException('denied', 'NotAllowedError');
				};
			return file;
		};
		await assert.rejects(store.beginFile(1));
		await store.abortPartial();
		assert.equal(folder.entriesMap.size, 1);
		await store.dispose();
		assert.equal(folder.entriesMap.size, 1);
		assert.deepEqual(
			new Uint8Array(
				await (
					await folder.getFileHandle('1 - ซ้ำ.txt')
				)
					.getFile()
					.then((file) => file.arrayBuffer())
			),
			new Uint8Array([1, 2, 3])
		);
		assert.equal((await sentinel.getFile()).size, 1);
	} finally {
		await store?.dispose();
		h.restore();
	}
});

test('an existing destination filename is never overwritten or removed as a partial', async () => {
	const h = storageHarness(),
		destination = new MemoryDirectory();
	let store;
	try {
		store = await ReceivedBatchStore.open({ fileCount: 1, totalBytes: 3 });
		await manifest(store);
		await store.configureDestination(
			{ mode: 'folder', directory: destination },
			new AbortController().signal
		);
		const folder = [...destination.entriesMap.values()][0],
			sentinel = await folder.getFileHandle('1 - ซ้ำ.txt', { create: true });
		sentinel.bytes = new Uint8Array([9]);
		await assert.rejects(store.beginFile(0), /เขียนทับ/);
		await store.abortPartial();
		await store.dispose();
		assert.deepEqual(
			new Uint8Array(await (await sentinel.getFile()).arrayBuffer()),
			new Uint8Array([9])
		);
	} finally {
		await store?.dispose();
		h.restore();
	}
});

test('a denied partial removal retains its owned path for disposal retry without double abort', async () => {
	const h = storageHarness(),
		destination = new MemoryDirectory();
	let store;
	try {
		const sentinel = await destination.getFileHandle('existing.txt', { create: true });
		sentinel.bytes = new Uint8Array([9]);
		store = await ReceivedBatchStore.open({ fileCount: 1, totalBytes: 3 });
		await manifest(store);
		await store.configureDestination(
			{ mode: 'folder', directory: destination },
			new AbortController().signal
		);
		const folder = [...destination.entriesMap.values()].find(
			(entry) => entry instanceof MemoryDirectory
		);
		await store.beginFile(0);
		await store.writeFile(new Uint8Array([1]));
		const partial = await folder.getFileHandle('1 - ซ้ำ.txt');
		let removals = 0;
		const remove = folder.removeEntry.bind(folder);
		folder.removeEntry = async (name) => {
			removals++;
			if (removals === 1) throw new DOMException('permission revoked', 'NotAllowedError');
			return remove(name);
		};
		await assert.rejects(store.abortPartial(), /ยังยืนยันการล้าง/);
		assert.equal(folder.entriesMap.size, 1);
		assert.equal(partial.aborts, 1);
		await store.dispose();
		assert.equal(removals, 2);
		assert.equal(partial.aborts, 1);
		assert.equal(folder.entriesMap.size, 0);
		assert.deepEqual(
			new Uint8Array(await (await sentinel.getFile()).arrayBuffer()),
			new Uint8Array([9])
		);
	} finally {
		await store?.dispose();
		h.restore();
	}
});
