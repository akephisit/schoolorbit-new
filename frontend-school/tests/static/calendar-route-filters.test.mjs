import assert from 'node:assert/strict';
import test from 'node:test';
import ts from 'typescript';
import { readFile } from 'node:fs/promises';
const source = await readFile(
	new URL('../../src/lib/utils/calendar-route-filters.ts', import.meta.url),
	'utf8'
);
const compiled = ts.transpileModule(source, {
	compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
}).outputText;
const exports = {};
const dates = await import('date-fns'),
	locale = await import('date-fns/locale');
const calendarSource = await readFile(
	new URL('../../src/lib/utils/calendar.ts', import.meta.url),
	'utf8'
);
const calendarCode = ts.transpileModule(calendarSource, {
	compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
}).outputText;
const calendar = {};
new Function('require', 'exports', calendarCode)(
	(name) => (name === 'date-fns' ? dates : locale),
	calendar
);
new Function('require', 'exports', compiled)(() => calendar, exports);
const parse = (query) =>
	exports.calendarRouteFilters(
		new URL(`https://tenant.example/staff/calendar?${query}`),
		'2026-10-01'
	);
test('calendar committed filters produce one stable date/filter owner', () => {
	const a = parse(
		'academicYearId=year&academicTermId=term&month=2026-11&q=%20meeting%20&categoryId=cat&tagId=tag&audience=parent&visibility=private'
	);
	assert.equal(a.month, '2026-11-01');
	assert.deepEqual(a.filters, {
		from: '2026-11-01',
		to: '2026-12-12',
		q: 'meeting',
		categoryId: 'cat',
		tagId: 'tag',
		audience: 'parent',
		visibility: 'private'
	});
	assert.equal(
		a.key,
		parse(
			'visibility=private&audience=parent&tagId=tag&categoryId=cat&q=meeting&month=2026-11&academicTermId=term&academicYearId=year'
		).key
	);
	assert.notEqual(a.key, parse('month=2026-11').key);
	assert.equal(parse('academicYearId=other&month=2026-11').key, parse('month=2026-11').key);
});
test('calendar malformed month and unsupported enums fall back without false context', () => {
	const result = parse('month=2026-13&audience=admin&visibility=hidden');
	assert.equal(result.month, '2026-10-01');
	assert.equal('academicYearId' in result.filters, false);
	assert.equal(result.filters.audience, undefined);
	assert.equal(result.filters.visibility, undefined);
});

const requestSource = await readFile(
	new URL('../../src/lib/utils/calendar-request-filters.ts', import.meta.url),
	'utf8'
);
const requestCode = ts.transpileModule(requestSource, {
	compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
}).outputText;
const requestFilters = {};
new Function('exports', requestCode)(requestFilters);
const parseRequests = (query) =>
	requestFilters.calendarRequestFilters(
		new URL(`https://tenant.example/staff/calendar/requests?${query}`)
	);
test('review defaults to pending and normalizes old approved links while own history keeps every status', () => {
	assert.equal(parseRequests('review=true').status, 'pending');
	assert.equal(parseRequests('review=true&status=approved').status, 'pending');
	assert.equal(parseRequests('review=true&status=rejected').status, 'rejected');
	assert.equal(parseRequests('review=true&status=all').status, undefined);
	assert.equal(parseRequests('status=approved').status, 'approved');
	assert.equal(parseRequests('').status, undefined);
	assert.equal(parseRequests('offset=-1').offset, 0);
});
