import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { registerDeliveryDraftReconcile } from '../../src/lib/academic/delivery-draft-reconcile.ts';

test('opening publication has a chosen date and hard deletion retires cancellation', async () => {
	const api = JSON.parse(
		await readFile(new URL('../../../contracts/openapi/school-api.json', import.meta.url), 'utf8')
	);
	const schemas = api.components.schemas;
	assert.equal(schemas.CreateAcademicTermChangeSetRequest.properties.effectiveFrom, undefined);
	assert.ok(schemas.PublishAcademicTermChangeSetRequest.required.includes('effectiveFrom'));
	assert.ok(schemas.AcademicTermChangeSet.properties.effectiveFrom.type.includes('null'));
	assert.ok(schemas.AcademicTermChangeSet.required.includes('referenceDate'));
	assert.ok(schemas.AcademicTermChangeSet.required.includes('changes'));
	for (const field of [
		'rowVersion',
		'changeSetRowVersion',
		'expectedItemCount',
		'expectedOfferingCount',
		'expectedGroupCount'
	])
		assert.ok(schemas.DeleteDeliveryVersionRequest.required.includes(field));
	assert.equal(
		api.paths['/api/academic/delivery-versions/{id}'].delete.operationId,
		'deleteDeliveryVersion'
	);
	assert.equal(api.paths['/api/academic/term-change-sets/{id}/cancel'], undefined);
});
test('draft reconciliation stays event driven, pauses while hidden, and unregisters on disposal', () => {
	const win = new EventTarget(),
		doc = new EventTarget();
	doc.hidden = false;
	let timer;
	let disposed = false;
	let polling = false;
	let reads = 0;
	win.setInterval = (callback) => {
		timer = callback;
		return 1;
	};
	win.clearInterval = (id) => {
		assert.equal(id, 1);
		disposed = true;
	};
	const unregister = registerDeliveryDraftReconcile(
		() => reads++,
		() => polling,
		win,
		doc
	);
	assert.equal(reads, 0);
	timer();
	assert.equal(reads, 0);
	win.dispatchEvent(new Event('focus'));
	assert.equal(reads, 1);
	doc.hidden = true;
	polling = true;
	timer();
	assert.equal(reads, 1);
	doc.hidden = false;
	doc.dispatchEvent(new Event('visibilitychange'));
	timer();
	assert.equal(reads, 3);
	unregister();
	assert.equal(disposed, true);
	win.dispatchEvent(new Event('focus'));
	assert.equal(reads, 3);
});
