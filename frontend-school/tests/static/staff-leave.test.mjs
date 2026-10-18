import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import {
	leaveDraft,
	leaveDraftErrors,
	leavePeriodInput,
	displayedPolicyLimit,
	policyLimitInput,
	leavePolicyDraft
} from '../../src/lib/staff-leave/drafts.ts';
import {
	leaveListFilters,
	leaveListDestination,
	safeLeaveReturn
} from '../../src/lib/staff-leave/filters.ts';
import { leaveAmount } from '../../src/lib/staff-leave/presentation.ts';
import { LeaveMutationIntent } from '../../src/lib/staff-leave/mutation-intent.ts';

const presentation = ts.transpileModule(
	await readFile(new URL('../../src/lib/staff-leave/presentation.ts', import.meta.url), 'utf8'),
	{ compilerOptions: { module: ts.ModuleKind.ESNext } }
).outputText;
const presentationUrl = `data:text/javascript;base64,${Buffer.from(presentation).toString('base64')}`;
const reportSource = ts
	.transpileModule(
		await readFile(new URL('../../src/lib/staff-leave/report.ts', import.meta.url), 'utf8'),
		{ compilerOptions: { module: ts.ModuleKind.ESNext } }
	)
	.outputText.replace('./presentation.js', presentationUrl);
const { leaveCsvCell, leaveUsageCsv, validReportDate } = await import(
	`data:text/javascript;base64,${Buffer.from(reportSource).toString('base64')}`
);

test('half-day granularity displays days and preserves the shared wire allowance', () => {
	assert.equal(leaveAmount(3, 'half_day'), '1.5 วัน');
	assert.equal(displayedPolicyLimit(5, 'half_day'), '2.5');
	assert.equal(policyLimitInput('2.5', 'half_day'), 5);
	assert.equal(policyLimitInput('', 'half_day'), null);
	assert.throws(() => policyLimitInput('0.25', 'half_day'));
	assert.throws(() => policyLimitInput('-1', 'day'));
	assert.throws(() => policyLimitInput('1.5', 'minute'));
});
test('request payload omits client amounts and normalizes stale unit-dependent fields', () => {
	const draft = {
		...leaveDraft(),
		typeId: 'type',
		startDate: '2026-10-18',
		endDate: '2026-10-18',
		startTime: '09:00',
		endTime: '10:30',
		dayPortion: 'morning',
		reason: ' เหตุผล ',
		evidenceFileIds: ['file']
	};
	assert.deepEqual(leavePeriodInput(draft, 'minute'), {
		typeId: 'type',
		startDate: '2026-10-18',
		endDate: '2026-10-18',
		startTime: '09:00:00',
		endTime: '10:30:00',
		dayPortion: 'full_day',
		reason: 'เหตุผล',
		evidenceFileIds: ['file']
	});
	assert.equal(leavePeriodInput(draft, 'half_day').dayPortion, 'morning');
	assert.equal(leavePeriodInput(draft, 'day').startTime, null);
	assert.equal('amount' in leavePeriodInput(draft, 'minute'), false);
	assert.equal(
		leaveDraftErrors({ ...draft, endDate: '2026-10-19' }, { unit: 'half_day' }).endDate,
		'การลาครึ่งวันต้องอยู่ในวันเดียวกัน'
	);
	assert.equal(
		leaveDraftErrors({ ...draft, endTime: '08:00' }, { unit: 'minute' }).endTime,
		'เวลาสิ้นสุดต้องอยู่หลังเวลาเริ่ม'
	);
});
test('list context restores status and paging and refuses external return paths', () => {
	assert.deepEqual(
		leaveListFilters(
			new URL('https://example.invalid/staff/leave?status=returned&offset=20'),
			'leave'
		),
		{ kind: 'leave', status: 'returned', limit: 20, offset: 20 }
	);
	assert.equal(
		leaveListFilters(new URL('https://example.invalid/?offset=-1&status=made-up'), 'exit').offset,
		0
	);
	assert.equal(
		leaveListDestination('/staff/leave', 'draft', 20),
		'/staff/leave?status=draft&offset=20'
	);
	assert.equal(safeLeaveReturn('//evil.invalid', '/staff/leave'), '/staff/leave');
	assert.equal(
		safeLeaveReturn('/staff/leave?status=returned&offset=20', '/staff/leave'),
		'/staff/leave?status=returned&offset=20'
	);
});
test('mutation retry key remains stable only for the exact unchanged payload', () => {
	const intent = new LeaveMutationIntent();
	const first = intent.for({ revision: 1, reason: 'เดิม' });
	assert.equal(intent.for({ revision: 1, reason: 'เดิม' }), first);
	assert.notEqual(intent.for({ revision: 1, reason: 'ใหม่' }), first);
	intent.clear();
	assert.notEqual(intent.for({ revision: 1, reason: 'เดิม' }), first);
});
test('policy amendment copies all criteria without mutating the prior version or choosing its effective date', () => {
	const prior = {
		unit: 'half_day',
		sourceReference: 'คำสั่งเดิม',
		personnelTypes: ['contract_employee'],
		countingMode: 'working_days',
		workingWeekdays: [1, 3, 5],
		excludedDates: ['2026-10-23'],
		quotaYearStartMonth: 10,
		maxPerRequest: 5,
		annualLimit: 20,
		evidenceRequired: false,
		evidenceFromAmount: 3,
		minimumNoticeDays: 2,
		steps: [
			{
				title: 'ตรวจสอบ',
				recipients: [{ kind: 'user', userId: 'reviewer' }],
				requiresSignature: true,
				kind: 'review',
				completionPolicy: 'all'
			}
		]
	};
	const draft = leavePolicyDraft('leave', prior);
	assert.equal(draft.effectiveFrom, undefined);
	assert.equal(draft.maxPerRequest, '2.5');
	assert.equal(draft.evidenceFromAmount, '1.5');
	assert.equal(draft.minimumNoticeDays, '2');
	draft.steps[0].recipients[0].userId = 'other';
	draft.excludedDates.push('2026-10-24');
	assert.equal(prior.steps[0].recipients[0].userId, 'reviewer');
	assert.deepEqual(prior.excludedDates, ['2026-10-23']);
	assert.equal(leavePolicyDraft('exit').maxPerRequest, '120');
	assert.equal(leavePolicyDraft('leave').annualLimit, '');
});
test('CSV quotes delimiters, newlines and spreadsheet formula prefixes while using displayed days', () => {
	for (const value of ['=SUM(1,2)', '+cmd', '-1+2', '@name', '\t=1', ' \n=1'])
		assert.match(leaveCsvCell(value), /^"'/);
	assert.equal(leaveCsvCell('ครู "เอ",\nทดสอบ'), '"ครู ""เอ"",\nทดสอบ"');
	const csv = leaveUsageCsv({
		from: '2026-10-01',
		to: '2026-10-31',
		items: [
			{
				displayName: '=formula',
				typeName: 'ลากิจ',
				quotaYearStart: '2026-01-01',
				approvedAmount: 3,
				reservedAmount: 1,
				annualLimit: null,
				unit: 'half_day'
			}
		]
	});
	assert.match(csv, /"'=formula"/);
	assert.match(csv, /"1.5 วัน","0.5 วัน","ไม่กำหนด"/);
	assert.equal(validReportDate('2026-02-30'), undefined);
	assert.equal(validReportDate('2024-02-29'), '2024-02-29');
});
