import test from 'node:test';
import assert from 'node:assert/strict';
import { validateDocumentMetadata } from '../../src/lib/correspondence/metadata.ts';

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
