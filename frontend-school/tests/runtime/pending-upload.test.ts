import assert from 'node:assert/strict';
import test from 'node:test';
import { PendingUpload } from '../../src/lib/files/pending-upload.ts';

test('preparing stays local; concurrent commits and later business retries reuse one file', async () => {
	let calls = 0;
	let complete!: (file: { id: string }) => void;
	const file = new File(['image'], 'signature.png', { type: 'image/png' });
	const draft = new PendingUpload(
		file,
		{
			purpose: 'correspondence_signature',
			resourceId: 'user-a',
			owner: 'document-a'
		},
		(selected, purpose, resource) => {
			calls++;
			assert.equal(selected, file);
			assert.equal(purpose, 'correspondence_signature');
			assert.equal(resource, 'user-a');
			return new Promise((resolve) => {
				complete = resolve;
			});
		}
	);
	assert.equal(calls, 0);
	const first = draft.commit();
	const second = draft.commit();
	assert.equal(calls, 1);
	complete({ id: 'uploaded-a' });
	assert.deepEqual(await Promise.all([first, second]), ['uploaded-a', 'uploaded-a']);
	assert.equal(await draft.commit(), 'uploaded-a');
	assert.equal(calls, 1);
});

test('replacement or unmount rejects late completion and never adopts it into a new scope', async () => {
	let complete!: (file: { id: string }) => void;
	const first = new PendingUpload(
		new File(['old'], 'old.png'),
		{
			purpose: 'correspondence_signature',
			resourceId: 'old-user',
			owner: 'old-document'
		},
		() =>
			new Promise((resolve) => {
				complete = resolve;
			})
	);
	const uploading = first.commit();
	first.dispose();
	complete({ id: 'old-upload' });
	await assert.rejects(uploading, /ถูกเปลี่ยนแล้ว/);
	await assert.rejects(first.commit(), /ถูกเปลี่ยนแล้ว/);
	const replacement = new PendingUpload(
		new File(['new'], 'new.png'),
		{
			purpose: 'correspondence_signature',
			resourceId: 'new-user',
			owner: 'new-document'
		},
		async () => ({ id: 'new-upload' })
	);
	assert.equal(await replacement.commit(), 'new-upload');
});

test('failed transport preserves the draft and permits an explicit retry without claiming exactly once', async () => {
	let calls = 0;
	const draft = new PendingUpload(
		new File(['body'], 'draft.pdf'),
		{
			purpose: 'correspondence_document',
			resourceId: 'document',
			owner: 'draft'
		},
		async () => {
			if (++calls === 1) throw new Error('response unavailable');
			return { id: 'known-upload' };
		}
	);
	await assert.rejects(draft.commit(), /response unavailable/);
	assert.equal(await draft.commit(), 'known-upload');
	assert.equal(await draft.commit(), 'known-upload');
	assert.equal(calls, 2);
});
