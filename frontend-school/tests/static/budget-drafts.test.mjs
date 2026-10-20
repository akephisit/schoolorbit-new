import assert from 'node:assert/strict';
import test from 'node:test';
import ts from 'typescript';
import { readFile } from 'node:fs/promises';
const source = async (path) =>
	ts.transpileModule(
		await readFile(new URL(`../../src/lib/budget/${path}`, import.meta.url), 'utf8'),
		{ compilerOptions: { module: ts.ModuleKind.ESNext } }
	).outputText;
const moneyUrl = `data:text/javascript;base64,${Buffer.from(await source('money.ts')).toString('base64')}`;
const load = async (file) =>
	import(
		`data:text/javascript;base64,${Buffer.from((await source(file)).replace('./money.js', moneyUrl)).toString('base64')}`
	);
const { budgetPivot } = await load('pivot.ts');
const { paymentLines, paymentAllocations } = await load('allocations.ts');
const { newActivity, newLine, applyDefaults, catalogLine, activityInput, projectErrors } =
	await load('drafts.ts');
function row(activityId, fundId, expenseCategoryId, plannedSatang, paidSatang = '0') {
	return {
		projectId: 'math',
		projectName: 'คณิตศาสตร์',
		activityId,
		activityTitle: activityId,
		fundId,
		fundName: fundId,
		expenseCategoryId,
		expenseCategoryName: expenseCategoryId,
		plannedSatang,
		adjustedSatang: '0',
		currentSatang: plannedSatang,
		reservedSatang: '0',
		paidSatang,
		availableSatang: plannedSatang,
		loanOutstandingSatang: '0'
	};
}
test('three activity fixture reconciles the 9928 baht fund and category matrix', () => {
	const rows = [
		row('one', 'grant', 'materials', '358800'),
		row('two', 'grant', 'equipment', '304000'),
		row('three', 'learner', 'materials', '330000')
	];
	const pivot = budgetPivot(rows);
	assert.equal(pivot.totalSatang, '992800');
	assert.deepEqual(
		pivot.activities.map((item) => item.totalSatang),
		['358800', '304000', '330000']
	);
	assert.equal(pivot.columnTotals['grant|materials'], '358800');
	assert.equal(pivot.columnTotals['grant|equipment'], '304000');
	assert.equal(pivot.columnTotals['learner|materials'], '330000');
	assert.equal(pivot.groups[0].columns.length, 2);
	assert.equal(pivot.groups[1].columns.length, 1);
	assert.equal(budgetPivot(rows, 'paid').totalSatang, '0');
});
test('pivot adds repeated contributions once and keeps distinct activity identities with equal names', () => {
	const rows = [
		row('a', 'grant', 'materials', '100', '20'),
		row('a', 'grant', 'materials', '200', '30'),
		row('b', 'grant', 'materials', '400', '40')
	].map((item) => ({ ...item, activityTitle: 'ชื่อเหมือนกัน' }));
	const pivot = budgetPivot(rows, 'paid');
	assert.equal(pivot.activities.length, 2);
	assert.equal(pivot.activities[0].values['grant|materials'], '50');
	assert.equal(pivot.totalSatang, '90');
});
test('activity defaults fill new rows only; bulk application touches explicitly selected rows', () => {
	let activity = {
		...newActivity(),
		defaultFundId: 'first',
		defaultExpenseCategoryId: 'materials'
	};
	activity.lines = [newLine(activity), newLine(activity)];
	const original = activity.lines.map((item) => ({ ...item }));
	activity.defaultFundId = 'second';
	assert.equal(activity.lines[0].fundId, 'first');
	assert.equal(newLine(activity).fundId, 'second');
	const changed = applyDefaults(activity, [activity.lines[0].key]);
	assert.equal(changed.lines[0].fundId, 'second');
	assert.equal(changed.lines[1].fundId, 'first');
	assert.deepEqual(activity.lines, original);
});
test('catalog rows retain the selected historical version and exact copied price in payloads', () => {
	const activity = {
		...newActivity(),
		title: 'กิจกรรม',
		defaultFundId: 'grant',
		defaultExpenseCategoryId: 'materials'
	};
	const item = {
		id: 'item',
		name: 'พัสดุเดิม',
		unit: 'ชุด',
		latestVersion: { id: 'old-version', price: '1794.00' }
	};
	const line = catalogLine(activity, item);
	item.latestVersion = { id: 'new-version', price: '2000.00' };
	activity.lines = [line];
	const input = activityInput(activity);
	assert.equal(input.lines[0].supplyVersionId, 'old-version');
	assert.equal(input.lines[0].unitPriceSatang, '179400');
	assert.equal(input.lines[0].description, 'พัสดุเดิม');
	assert.equal('key' in input.lines[0], false);
	assert.equal('key' in input, false);
});
test('project validates civil dates and does not create extra project fields', () => {
	assert.deepEqual(
		projectErrors({
			name: 'โครงการ',
			fiscalYearId: 'year',
			organizationUnitId: 'org',
			startDate: '2026-10-01',
			endDate: '2027-09-30'
		}),
		{}
	);
	assert.ok(
		projectErrors({
			name: '',
			fiscalYearId: '',
			organizationUnitId: '',
			startDate: '2026-02-30',
			endDate: '2026-02-01'
		}).startDate
	);
});

test('partial payment uses request reservations and refunds use original payment less prior refunds', () => {
	const limits = paymentLines({
		lines: [{ planLineId: 'a', amountSatang: '1000', reservedSatang: '400', paidSatang: '600' }]
	});
	assert.deepEqual(paymentAllocations({ a: '4.00' }, limits), [
		{ planLineId: 'a', amountSatang: '400' }
	]);
	assert.throws(() => paymentAllocations({ a: '4.01' }, limits));
	const refundLimits = paymentLines(
		{ lines: [] },
		{
			allocations: [{ planLineId: 'a', amountSatang: '600' }],
			refunds: [
				{ allocations: [{ planLineId: 'a', amountSatang: '100' }] },
				{ allocations: [{ planLineId: 'a', amountSatang: '200' }] }
			]
		}
	);
	assert.equal(refundLimits[0].remainingSatang, '300');
	assert.deepEqual(paymentAllocations({ a: '3.00' }, refundLimits), [
		{ planLineId: 'a', amountSatang: '300' }
	]);
	assert.throws(() => paymentAllocations({ a: '3.01' }, refundLimits));
	assert.throws(() => paymentAllocations({ a: '0.00' }, refundLimits));
});
