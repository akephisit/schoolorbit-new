import assert from 'node:assert/strict';
import test from 'node:test';
import {
	actionRequirements,
	validateSigningDraft,
	historyActionLabel
} from '../../src/lib/correspondence/drafts.ts';
const step = {
	kind: 'approve',
	requiresSignature: true,
	requiresComment: true,
	requiresEvidence: true
};
const signature = {
	method: 'drawn',
	strokes: [],
	page: 1,
	x: 0.5,
	y: 0.7,
	width: 0.25,
	height: 0.1,
	documentRevision: 3,
	contentHash: 'hash'
};
const paths = [
	{
		points: [
			{ x: 0.1, y: 0.1 },
			{ x: 0.5, y: 0.4 }
		]
	}
];
test('opening a document stays distinct from completing a required-signature step', () => {
	assert.deepEqual(actionRequirements('read', step), {
		acceptsEvidence: false,
		comment: false,
		evidence: false,
		signature: false
	});
	assert.deepEqual(actionRequirements('approve', step), {
		acceptsEvidence: true,
		comment: true,
		evidence: true,
		signature: true
	});
	assert.equal(actionRequirements('return_for_revision', step).evidence, false);
	assert.equal(actionRequirements('reassign', step).acceptsEvidence, false);
});
test('signature confirmation binds the immutable document revision and refuses escaped placement', () => {
	assert.equal(validateSigningDraft(signature, paths, 'drawn', '', true, 3), null);
	assert.match(validateSigningDraft(signature, paths, 'drawn', '', false, 3), /ยืนยัน/);
	assert.match(validateSigningDraft(signature, paths, 'drawn', '', true, 4), /ฉบับใหม่/);
	assert.match(
		validateSigningDraft({ ...signature, x: 0.9 }, paths, 'drawn', '', true, 3),
		/ภายในหน้า/
	);
	assert.match(
		validateSigningDraft({ ...signature, width: NaN }, paths, 'drawn', '', true, 3),
		/ภายในหน้า/
	);
	assert.equal(
		validateSigningDraft({ ...signature, method: 'image' }, [], 'image', 'own-asset', true, 3),
		null
	);
});
test('ordinary history renders meaningful activity names without internal codes', () => {
	assert.equal(historyActionLabel('create'), 'ลงทะเบียนหนังสือ');
	assert.equal(historyActionLabel('internal_transport_code'), 'บันทึกการดำเนินการ');
});
