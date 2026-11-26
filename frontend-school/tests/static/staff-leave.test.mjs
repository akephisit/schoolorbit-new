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
import {
	leaveAmount,
	leavePeriodCaption,
	leaveUsageAmount,
	leaveHistoryLabel
} from '../../src/lib/staff-leave/presentation.ts';
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

test('historical half-day evidence stays exact while active policy amounts require whole units', () => {
	assert.equal(leaveAmount(3, 'half_day'), '1.5 วัน');
	assert.equal(leaveHistoryLabel('created'), 'สร้างฉบับร่าง');
	assert.equal(leaveHistoryLabel('cancellation_approve'), 'อนุมัติการยกเลิกคำขอ');
	assert.equal(displayedPolicyLimit(5, 'half_day'), '2.5');
	assert.throws(() => policyLimitInput('2.5'));
	assert.equal(policyLimitInput('8'), 8);
	assert.equal(policyLimitInput(''), null);
	assert.throws(() => policyLimitInput('0.25'));
	assert.throws(() => policyLimitInput('-1'));
	assert.throws(() => policyLimitInput('1.5'));
	assert.throws(() => policyLimitInput('4503599627370496.5'));
	assert.throws(() => policyLimitInput('9007199254740993'));
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
		halfDayBoundary: '11:30:00',
		reason: ' เหตุผล ',
		evidenceFileIds: ['file']
	};
	assert.deepEqual(leavePeriodInput(draft, 'exit'), {
		typeId: 'type',
		academicYearId: null,
		academicTermId: null,
		startDate: '2026-10-18',
		endDate: '2026-10-18',
		startTime: '09:00:00',
		endTime: '10:30:00',
		reason: 'เหตุผล',
		evidenceFileIds: ['file']
	});
	assert.equal('dayPortion' in leavePeriodInput(draft, 'leave'), false);
	assert.equal('halfDayBoundary' in leavePeriodInput(draft, 'exit'), false);
	assert.equal(leavePeriodInput(draft, 'leave').startTime, null);
	assert.equal('amount' in leavePeriodInput(draft, 'exit'), false);
	assert.equal(leaveDraftErrors({ ...draft, endDate: '2026-10-19' }, 'leave').endDate, undefined);
	assert.equal(
		leaveDraftErrors({ ...draft, endTime: '08:00' }, 'exit').endTime,
		'เวลาสิ้นสุดต้องอยู่หลังเวลาเริ่ม'
	);
});
test('travel pairs optional boundary times and carries only authorized travel fields in the draft payload', () => {
	const draft = {
		...leaveDraft(),
		typeId: 'travel',
		startDate: '2026-10-18',
		endDate: '2026-10-20',
		startTime: '17:00',
		endTime: '08:00',
		reason: 'ประชุมวิชาการ',
		destination: ' ศูนย์ประชุม ',
		companions: [
			{ id: 'other-a', displayName: 'ครูเอ' },
			{ id: 'other-b', displayName: 'ครูบี' }
		]
	};
	assert.deepEqual(leaveDraftErrors(draft, 'official_travel'), {});
	assert.match(leaveDraftErrors({ ...draft, endTime: '' }, 'official_travel').endTime, /ระบุเวลา/);
	assert.match(
		leaveDraftErrors({ ...draft, destination: ' ' }, 'official_travel').destination,
		/ระบุสถานที่/
	);
	assert.match(
		leaveDraftErrors({ ...draft, endDate: draft.startDate }, 'official_travel').endTime,
		/อยู่หลัง/
	);
	const travel = leavePeriodInput(draft, 'official_travel');
	assert.deepEqual(travel.companionUserIds, ['other-a', 'other-b']);
	assert.equal(travel.destination, 'ศูนย์ประชุม');
	assert.match(leavePeriodCaption(travel), /17:00 ถึง .* 08:00/);
	assert.equal(
		leavePeriodInput({ ...draft, startTime: '', endTime: '' }, 'official_travel').startTime,
		null
	);
	assert.equal('destination' in leavePeriodInput(draft, 'exit'), false);
	assert.equal('companionUserIds' in leavePeriodInput(draft, 'leave'), false);
	assert.equal(
		safeLeaveReturn('/staff/official-travel?status=draft', '/staff/leave'),
		'/staff/official-travel?status=draft'
	);
});
test('school exit reference starts a fresh policy without inventing school-specific limits or routing', () => {
	const preset = {
		id: 'school_exit',
		kind: 'exit',
		unit: 'minute',
		countingMode: 'working_days',
		personnelTypes: []
	};
	const draft = leavePolicyDraft('exit', undefined, preset);
	assert.equal(draft.sourceReference, '');
	assert.equal(draft.unit, 'minute');
	assert.equal(draft.countingMode, 'working_days');
	assert.equal(draft.effectiveFrom, undefined);
	assert.equal(draft.quotaMonth, undefined);
	assert.equal(draft.maxPerRequest, '');
	assert.deepEqual(draft.weekdays, []);
	assert.deepEqual(draft.personnelTypes, []);
	assert.deepEqual(draft.steps[0].recipients, []);
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
	assert.equal(draft.unit, 'day');
	assert.equal(draft.quotaMonth, 10);
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
				approvedAmount: '3',
				reservedAmount: '1',
				annualLimit: null,
				unit: 'day',
				amountScale: 2
			}
		]
	});
	assert.match(csv, /"'=formula"/);
	assert.match(csv, /"1.5 วัน","0.5 วัน","ไม่กำหนด"/);
	assert.equal(validReportDate('2026-02-30'), undefined);
	assert.equal(validReportDate('2024-02-29'), '2024-02-29');
});

test('scaled report strings preserve half-days and integers beyond Number precision', () => {
	assert.equal(
		leaveUsageAmount('9007199254740993', { unit: 'day', amountScale: 2 }),
		'4503599627370496.5 วัน'
	);
	assert.equal(
		leaveUsageAmount('9007199254740993', { unit: 'minute', amountScale: 1 }),
		'9007199254740993 นาที'
	);
	assert.equal(leaveUsageAmount('0', { unit: 'day', amountScale: 2 }), '0 วัน');
	assert.equal(leaveUsageAmount('-1', { unit: 'day', amountScale: 2 }), '-0.5 วัน');
	assert.throws(() => leaveUsageAmount('1', { unit: 'day', amountScale: 3 }));
});

test('unsafe original policy limits remain unresolved before half-day conversion even if the quotient looks safe', () => {
	const roundedWireValue = JSON.parse('9007199254740993');
	assert.equal(roundedWireValue, 9007199254740992);
	assert.equal(Number.isSafeInteger(roundedWireValue / 2), true);
	for (const unit of ['half_day', 'day', 'minute']) {
		assert.equal(displayedPolicyLimit(roundedWireValue, unit), undefined);
	}
	for (const kind of ['leave', 'exit']) {
		const draft = leavePolicyDraft(kind, {
			unit: 'half_day',
			maxPerRequest: roundedWireValue,
			annualLimit: roundedWireValue,
			evidenceFromAmount: roundedWireValue,
			steps: []
		});
		assert.deepEqual(draft.unresolvedLimits, {
			maxPerRequest: true,
			annualLimit: true,
			evidenceFromAmount: true
		});
		assert.equal(draft.maxPerRequest, '');
		assert.equal(draft.annualLimit, '');
		assert.equal(draft.evidenceFromAmount, '');
	}
	assert.equal(displayedPolicyLimit(Number.MAX_SAFE_INTEGER, 'half_day'), '4503599627370495.5');
	assert.equal(displayedPolicyLimit(null, 'day'), '');
	const safe = leavePolicyDraft('leave', { unit: 'half_day', maxPerRequest: 5, steps: [] });
	assert.equal(safe.maxPerRequest, '2.5');
	assert.equal(safe.unresolvedLimits.maxPerRequest, false);
	assert.throws(() => policyLimitInput(safe.maxPerRequest));
});
