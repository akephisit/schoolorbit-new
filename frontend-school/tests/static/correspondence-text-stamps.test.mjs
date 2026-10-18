import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import * as presentation from '../../src/lib/correspondence/presentation.ts';

const source = await readFile(
	new URL('../../src/lib/correspondence/text-stamps.ts', import.meta.url),
	'utf8'
);
const compiled = ts.transpileModule(source, {
	compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
}).outputText;
const helper = {};
new Function('require', 'exports', compiled)((id) => {
	assert.equal(id, './presentation.js');
	return presentation;
}, helper);
const { numberingError, numberingExample, stampTemplateErrors, expandStamp } = helper;
const record = {
	registerNumber: 18,
	registerYear: 2569,
	registerLabel: 'รับ 0018/2569',
	createdAt: '2026-10-18T18:30:00Z'
};
const context = {
	schoolName: 'โรงเรียนทดสอบ',
	actorDisplayName: 'ชื่อผู้ดำเนินการ ทดสอบ',
	signerTitle: null
};

test('number examples validate tokens and digit bounds while old record labels stay frozen', () => {
	assert.equal(
		numberingExample({ format: 'รับ {number}/{year}', minimumDigits: 4 }),
		'รับ 0001/2569'
	);
	assert.equal(numberingExample({ format: '{number}', minimumDigits: 1 }), '1');
	for (const format of [
		'{year}',
		'{number}{number}',
		'{number}/{year}/{year}',
		'{unknown}',
		'{number',
		'number}',
		'{number}\n'
	])
		assert.ok(numberingError({ format, minimumDigits: 1 }), format);
	for (const minimumDigits of [0, 9, 1.5, Number.NaN])
		assert.ok(numberingError({ format: '{number}', minimumDigits }));
	assert.equal(presentation.correspondenceRegisterLabel(record), 'รับ 0018/2569');
	assert.equal(
		presentation.correspondenceRegisterLabel({ ...record, registerLabel: null }),
		'18/2569'
	);
});

test('all nine stamp tokens expand to concrete editable text with Bangkok registration and insertion times', () => {
	const body =
		'{schoolName}\n{registerNumber}\n{registeredDate} {registeredTime}\n{date} {time}\n{signerName}\n{signerTitle}\n{instruction}';
	const expanded = expandStamp(
		body,
		context,
		record,
		{ signerTitle: 'ตำแหน่งที่ผู้ใช้ระบุ', instruction: 'เห็นควรดำเนินการ' },
		new Date('2026-10-20T03:15:00Z')
	);
	assert.deepEqual(expanded.errors, {});
	assert.match(expanded.text, /โรงเรียนทดสอบ\nรับ 0018\/2569/);
	assert.match(expanded.text, /19 ตุลาคม 2569 01:30/);
	assert.match(expanded.text, /20 ตุลาคม 2569 10:15/);
	assert.match(expanded.text, /ชื่อผู้ดำเนินการ ทดสอบ\nตำแหน่งที่ผู้ใช้ระบุ\nเห็นควรดำเนินการ/);
	assert.doesNotMatch(expanded.text, /[{}]/);
	assert.equal(body.includes('{schoolName}'), true);
});

test('missing required token inputs and unknown token syntax fail without inventing school or title', () => {
	const expanded = expandStamp('{instruction}\n{signerTitle}', context, record, {
		signerTitle: '',
		instruction: ''
	});
	assert.ok(expanded.errors.instruction);
	assert.ok(expanded.errors.signerTitle);
	assert.ok(
		expandStamp('{schoolName}', { ...context, schoolName: null }, record, {
			signerTitle: '',
			instruction: ''
		}).errors.body
	);
	assert.ok(
		expandStamp('{unknown}', context, record, { signerTitle: '', instruction: '' }).errors.body
	);
	for (const body of ['{unknown}', '{date', 'date}', 'x\u0000'])
		assert.ok(stampTemplateErrors({ name: 'แม่แบบ', body }).body);
	assert.deepEqual(stampTemplateErrors({ name: 'แม่แบบ', body: 'ทราบ\n{signerName}\t{date}' }), {});
	assert.ok(stampTemplateErrors({ name: '', body: 'ข้อความ' }).name);
	assert.ok(stampTemplateErrors({ name: 'แม่แบบ', body: 'ก'.repeat(5001) }).body);
	assert.deepEqual(stampTemplateErrors({ name: 'แม่แบบ', body: 'ก'.repeat(5000) }), {});
	assert.equal(
		expandStamp('<b>ข้อความ</b>', context, record, { signerTitle: '', instruction: '' }).text,
		'<b>ข้อความ</b>'
	);
});
