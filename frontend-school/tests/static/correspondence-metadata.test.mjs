import test from 'node:test';
import assert from 'node:assert/strict';
import {
	documentMetadataErrors,
	validateDocumentMetadata
} from '../../src/lib/correspondence/metadata.ts';

test('document metadata accepts the server Unicode bounds and rejects each next character', () => {
	assert.equal(
		validateDocumentMetadata({
			title: ` ${'ก'.repeat(300)} `,
			sender: '😀'.repeat(300),
			documentNumber: 'ก'.repeat(150)
		}),
		null
	);
	assert.match(validateDocumentMetadata({ title: 'ก'.repeat(301), sender: 'โรงเรียน' }), /300/);
	assert.match(validateDocumentMetadata({ title: 'เรื่อง', sender: '😀'.repeat(301) }), /300/);
	assert.match(
		validateDocumentMetadata({
			title: 'เรื่อง',
			sender: 'โรงเรียน',
			documentNumber: 'ก'.repeat(151)
		}),
		/150/
	);
	assert.match(validateDocumentMetadata({ title: '  ', sender: 'โรงเรียน' }), /กรอก/);
});

test('metadata errors identify every invalid field without changing the entered values', () => {
	const input = { title: '  ', sender: '😀'.repeat(301), documentNumber: 'ก'.repeat(151) };
	const before = structuredClone(input);
	const errors = documentMetadataErrors(input);
	assert.deepEqual(Object.keys(errors), ['title', 'sender', 'documentNumber']);
	assert.match(errors.title, /กรอกเรื่อง/);
	assert.match(errors.sender, /300/);
	assert.match(errors.documentNumber, /150/);
	assert.deepEqual(input, before);
	assert.deepEqual(documentMetadataErrors({ title: 'เรื่อง', sender: 'ผู้ส่ง' }), {});
});
