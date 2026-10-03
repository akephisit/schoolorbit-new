import assert from 'node:assert/strict';
import test from 'node:test';
import { execFileSync } from 'node:child_process';
import {
	staffCareerDraft,
	buildCreateStaffCareer,
	buildUpdateStaffCareer,
	resetCareerDetailsForChangedValue,
	formatCareerDate
} from '../../src/lib/forms/staff-career.ts';
const reference = { id: '55000000-0000-4000-8000-000000000001', revision: 2 };
test('career distinguishes absent current, null facts, and unchanged values', () => {
	const before = staffCareerDraft(null);
	assert.equal(buildCreateStaffCareer(before), undefined);
	assert.equal(buildUpdateStaffCareer(before, before, {}), undefined);
	const after = structuredClone(before);
	after.academicRank.value = 'none';
	assert.deepEqual(buildCreateStaffCareer(after), {
		entries: [
			{
				fact: { kind: 'academic_rank', value: 'none' },
				effectiveDate: null,
				orderDate: null,
				orderNumber: null,
				note: null
			}
		]
	});
	assert.equal(buildUpdateStaffCareer(before, after, {}).changes[0].expectedCurrent, null);
	after.academicRank.reference = reference;
	const clear = structuredClone(after);
	clear.academicRank.value = null;
	assert.deepEqual(buildUpdateStaffCareer(after, clear, {}).changes[0].entry.fact, {
		kind: 'academic_rank',
		value: null
	});
	assert.deepEqual(buildUpdateStaffCareer(after, clear, {}).changes[0].expectedCurrent, reference);
});
test('value changes reset only their own dates and order metadata', () => {
	const before = staffCareerDraft(null);
	before.academicRank.value = 'none';
	before.academicRank.effectiveDate = '2020-01-01';
	before.academicRank.orderNumber = '1/2563';
	before.jobPosition.effectiveDate = '2019-01-01';
	const after = structuredClone(before);
	after.academicRank.value = 'proficient';
	const changed = resetCareerDetailsForChangedValue(before, after);
	assert.equal(changed.academicRank.effectiveDate, '');
	assert.equal(changed.academicRank.orderNumber, '');
	assert.equal(changed.jobPosition.effectiveDate, before.jobPosition.effectiveDate);
	assert.equal(before.academicRank.effectiveDate, '2020-01-01');
});
test('metadata corrections carry reasons and references without other facts', () => {
	const before = staffCareerDraft(null);
	before.academicRank.value = 'none';
	before.academicRank.reference = reference;
	const after = structuredClone(before);
	after.academicRank.effectiveDate = '2020-01-01';
	assert.throws(() => buildUpdateStaffCareer(before, after, {}), /เหตุผล/);
	const patch = buildUpdateStaffCareer(before, after, { academic_rank: ' เติมตามคำสั่ง ' });
	assert.equal(patch.changes.length, 1);
	assert.equal(patch.changes[0].correctionReason, 'เติมตามคำสั่ง');
	assert.deepEqual(patch.changes[0].expectedCurrent, reference);
});
test('career accepts retroactive orders, rejects future or invalid calendar dates and bounded text', () => {
	const draft = staffCareerDraft(null);
	draft.personnelType.value = 'civil_servant';
	draft.personnelType.effectiveDate = '2020-01-01';
	draft.personnelType.orderDate = '2020-02-01';
	assert.equal(buildCreateStaffCareer(draft).entries[0].orderDate, '2020-02-01');
	for (const date of ['2023-02-29', '2999-01-01', 'bad-date']) {
		draft.personnelType.effectiveDate = date;
		assert.throws(() => buildCreateStaffCareer(draft));
	}
	draft.personnelType.effectiveDate = '';
	draft.personnelType.orderNumber = 'ก'.repeat(101);
	assert.throws(() => buildCreateStaffCareer(draft));
	draft.personnelType.orderNumber = 'คำสั่ง\n';
	assert.throws(() => buildCreateStaffCareer(draft));
});
test('calendar formatting keeps leap day and Buddhist year across time zones', () => {
	assert.equal(formatCareerDate(null), 'ยังไม่ระบุ');
	const script = `import { formatCareerDate } from './src/lib/forms/staff-career.ts';process.stdout.write(formatCareerDate('2024-02-29'));`;
	const values = ['UTC', 'America/Los_Angeles'].map((TZ) =>
		execFileSync(process.execPath, ['--input-type=module', '-e', script], {
			cwd: new URL('../../', import.meta.url),
			env: { ...process.env, TZ },
			encoding: 'utf8'
		})
	);
	assert.equal(values[0], values[1]);
	assert.match(values[0], /29/);
	assert.match(values[0], /2567/);
});
