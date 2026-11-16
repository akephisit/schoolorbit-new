import assert from 'node:assert/strict';
import test from 'node:test';
import {
	currentReturnReason,
	observationWorkflow,
	supervisionStatusLabel
} from '../../src/lib/supervision/supervision-presentation.ts';

test('manual cycle states and observation stages have distinct Thai meanings', () => {
	assert.equal(supervisionStatusLabel('open'), 'เปิดรับคำขอ');
	assert.equal(supervisionStatusLabel('closed'), 'ปิดรับคำขอ');
	assert.equal(supervisionStatusLabel('completed'), 'เสร็จสิ้น');
	assert.equal(
		observationWorkflow({ status: 'returned', evaluators: [] }).nextAction,
		'แก้ไขและส่งใหม่'
	);
	assert.equal(
		observationWorkflow({ status: 'requested', evaluators: [] }).nextOwner,
		'ผู้รับผิดชอบตรวจคำขอ'
	);
	assert.equal(
		observationWorkflow({ status: 'approved', evaluators: [] }).nextOwner,
		'ผู้มีสิทธิ์อนุมัติผล'
	);
	assert.equal(observationWorkflow({ status: 'published', evaluators: [] }).stage, 4);
	assert.equal(observationWorkflow({ status: 'completed', evaluators: [] }).stage, 5);
	assert.equal(observationWorkflow({ status: 'cancelled', evaluators: [] }).stage, -1);
});

test('required evaluator progress names only required pending evaluators and never advances server status', () => {
	const evaluators = [
		{ isRequired: true, status: 'submitted', evaluatorDisplayName: 'ส่งแล้ว' },
		{ isRequired: true, status: 'assigned', evaluatorDisplayName: 'ผู้ประเมินที่ยังค้าง' },
		{ isRequired: false, status: 'assigned', evaluatorDisplayName: 'ผู้ประเมินเสริม' }
	];
	const pending = observationWorkflow({ status: 'in_progress', evaluators });
	assert.equal(pending.evaluatorProgress, 'ผู้ประเมินที่จำเป็นส่งแล้ว 1/2 คน');
	assert.equal(pending.nextOwner, 'ผู้ประเมินที่ยังค้าง');
	assert.equal(pending.pendingRequiredLabel, 'ยังรอผู้ประเมินที่จำเป็น: ผู้ประเมินที่ยังค้าง');
	assert.equal(observationWorkflow({ status: 'in_progress', evaluators: [] }).stage, 1);
	assert.equal(observationWorkflow({ status: 'evaluators_submitted', evaluators }).stage, 2);
});

test('only the most recent current return reason is exposed, without mutating audit facts', () => {
	const actions = [
		{ toStatus: 'returned', comment: 'เหตุผลเดิม', createdAt: '2026-10-01T00:00:00Z' },
		{ toStatus: 'returned', comment: ' เหตุผลปัจจุบัน ', createdAt: '2026-10-03T00:00:00Z' },
		{ toStatus: 'requested', comment: 'ส่งใหม่', createdAt: '2026-10-04T00:00:00Z' }
	];
	const original = structuredClone(actions);
	assert.equal(currentReturnReason({ status: 'returned', actions }), 'เหตุผลปัจจุบัน');
	assert.equal(currentReturnReason({ status: 'requested', actions }), null);
	assert.equal(currentReturnReason({ status: 'returned', actions: [] }), null);
	assert.deepEqual(actions, original);
});
