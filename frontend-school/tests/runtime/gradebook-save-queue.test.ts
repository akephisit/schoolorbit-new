import assert from 'node:assert/strict';
import test from 'node:test';

import { createGradebookSaveQueue } from '../../src/lib/academic/gradebook/save-queue.ts';

interface CellMutation {
	itemId: string;
	studentId: string;
	value: string | null;
}

const keyOf = (cell: CellMutation) => `${cell.itemId}:${cell.studentId}`;
const wait = (milliseconds: number) =>
	new Promise<void>((resolve) => setTimeout(resolve, milliseconds));

test('phase batches never mix and retry does not resend a phase already saved', async () => {
	const batches: string[][] = [];
	let fail = true;
	const queue = createGradebookSaveQueue<{ id: string; phase: string }>({
		delayMs: 10000,
		keyOf: (cell) => cell.id,
		partitionKey: (cell) => cell.phase,
		saveBatch: async (cells) => {
			batches.push(cells.map((cell) => cell.id));
			if (cells[0]?.phase === 'final' && fail) throw new Error('offline');
		}
	});
	queue.enqueue({ id: 'before-1', phase: 'before_midterm' });
	queue.enqueue({ id: 'final-1', phase: 'final' });
	queue.enqueue({ id: 'before-2', phase: 'before_midterm' });
	await assert.rejects(queue.flush(), /offline/);
	assert.deepEqual(batches, [['before-1', 'before-2'], ['final-1']]);
	assert.equal(queue.status().pendingCount, 1);
	fail = false;
	await queue.retry();
	assert.deepEqual(batches, [['before-1', 'before-2'], ['final-1'], ['final-1']]);
	assert.equal(queue.status().state, 'saved');
});

test('save queue debounces and coalesces the latest value for each cell', async () => {
	const batches: CellMutation[][] = [];
	const queue = createGradebookSaveQueue<CellMutation>({
		delayMs: 5,
		keyOf,
		saveBatch: async (batch) => {
			batches.push(batch);
		}
	});

	queue.enqueue({ itemId: 'item-a', studentId: 'student-1', value: '1' });
	queue.enqueue({ itemId: 'item-a', studentId: 'student-1', value: '2' });
	queue.enqueue({ itemId: 'item-b', studentId: 'student-1', value: '3' });

	assert.equal(queue.status().state, 'unsaved');
	assert.equal(queue.status().pendingCount, 2);
	await wait(25);

	assert.deepEqual(batches, [
		[
			{ itemId: 'item-a', studentId: 'student-1', value: '2' },
			{ itemId: 'item-b', studentId: 'student-1', value: '3' }
		]
	]);
	assert.equal(queue.status().state, 'saved');
});

test('flush drains edits made while a save is in flight before resolving', async () => {
	let releaseFirst: (() => void) | undefined;
	const batches: CellMutation[][] = [];
	const queue = createGradebookSaveQueue<CellMutation>({
		delayMs: 1_000,
		keyOf,
		saveBatch: async (batch) => {
			batches.push(batch);
			if (batches.length === 1) {
				await new Promise<void>((resolve) => {
					releaseFirst = resolve;
				});
			}
		}
	});

	queue.enqueue({ itemId: 'item-a', studentId: 'student-1', value: '1' });
	const flushing = queue.flush();
	await wait(0);
	queue.enqueue({ itemId: 'item-b', studentId: 'student-1', value: '2' });
	releaseFirst?.();
	await flushing;

	assert.equal(batches.length, 2);
	assert.equal(queue.status().state, 'saved');
	assert.equal(queue.status().pendingCount, 0);
});

test('failed saves retain local mutations and retry sends them again', async () => {
	let attempts = 0;
	const conflict = Object.assign(new Error('ข้อมูลถูกแก้โดยผู้ใช้อื่น'), { status: 409 });
	const queue = createGradebookSaveQueue<CellMutation>({
		delayMs: 1_000,
		keyOf,
		saveBatch: async () => {
			attempts += 1;
			if (attempts === 1) throw conflict;
		}
	});

	queue.enqueue({ itemId: 'item-a', studentId: 'student-1', value: '4' });
	await assert.rejects(queue.flush(), conflict);
	assert.equal(queue.status().state, 'failed');
	assert.equal(queue.status().pendingCount, 1);
	assert.equal(queue.status().error, conflict);

	await queue.retry();
	assert.equal(attempts, 2);
	assert.equal(queue.status().state, 'saved');
	assert.equal(queue.status().pendingCount, 0);
});

test('a newer local edit wins when an older in-flight mutation fails', async () => {
	let rejectFirst: ((reason: Error) => void) | undefined;
	const batches: CellMutation[][] = [];
	const queue = createGradebookSaveQueue<CellMutation>({
		delayMs: 1_000,
		keyOf,
		saveBatch: async (batch) => {
			batches.push(batch);
			if (batches.length === 1) {
				await new Promise<void>((_resolve, reject) => {
					rejectFirst = reject;
				});
			}
		}
	});

	queue.enqueue({ itemId: 'item-a', studentId: 'student-1', value: '1' });
	const firstFlush = queue.flush();
	await wait(0);
	queue.enqueue({ itemId: 'item-a', studentId: 'student-1', value: '2' });
	rejectFirst?.(new Error('conflict'));
	await assert.rejects(firstFlush);

	await queue.retry();
	assert.equal(batches[1]?.[0]?.value, '2');
});

test('subscribe reports state changes and discard clears unsaved work', () => {
	const states: string[] = [];
	const queue = createGradebookSaveQueue<CellMutation>({
		delayMs: 1_000,
		keyOf,
		saveBatch: async () => undefined
	});
	const unsubscribe = queue.subscribe((snapshot) => states.push(snapshot.state));

	queue.enqueue({ itemId: 'item-a', studentId: 'student-1', value: null });
	queue.discard();
	unsubscribe();
	queue.enqueue({ itemId: 'item-b', studentId: 'student-1', value: '0' });

	assert.deepEqual(states, ['saved', 'unsaved', 'saved']);
	assert.equal(queue.status().pendingCount, 1);
	queue.discard();
});
