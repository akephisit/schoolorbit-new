import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const projectRoot = path.resolve(import.meta.dirname, '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

async function importRouteContext() {
	const source = await readProjectFile('src/lib/academic-context/route-context.ts');
	const compiled = ts.transpileModule(source, {
		compilerOptions: {
			module: ts.ModuleKind.ESNext,
			target: ts.ScriptTarget.ES2022,
			verbatimModuleSyntax: true
		},
		fileName: 'route-context.ts'
	}).outputText;
	const moduleUrl = `data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`;
	return import(moduleUrl);
}

function contextOptions() {
	return {
		activeAcademicYearId: 'year-active',
		activeAcademicTermId: 'term-active',
		years: [
			{
				id: 'year-active',
				name: 'ปีการศึกษา 2570',
				year: 2570,
				status: 'active',
				startDate: '2027-05-01',
				endDate: '2028-03-31'
			},
			{
				id: 'year-planning',
				name: 'ปีการศึกษา 2571',
				year: 2571,
				status: 'planning',
				startDate: '2028-05-01',
				endDate: '2029-03-31'
			}
		],
		terms: [
			{
				id: 'term-active',
				academicYearId: 'year-active',
				name: 'ภาคเรียนที่ 1',
				code: '1',
				sequence: 1,
				termType: 'regular',
				status: 'active',
				startDate: '2027-05-01',
				endDate: '2027-10-31',
				includedInYearResult: true,
				blocksYearClosure: true
			},
			{
				id: 'term-planning',
				academicYearId: 'year-planning',
				name: 'ภาคฤดูร้อน',
				code: 'summer',
				sequence: 1,
				termType: 'summer',
				status: 'planning',
				startDate: '2029-03-01',
				endDate: '2029-04-15',
				includedInYearResult: false,
				blocksYearClosure: true
			}
		]
	};
}

test('academic context files own the typed read-only contract', async () => {
	const api = await readProjectFile('src/lib/api/academic-context.ts');
	const types = await readProjectFile('src/lib/academic-context/types.ts');
	const store = await readProjectFile('src/lib/academic-context/store.ts');

	assert.match(api, /components\['schemas'\]/);
	assert.match(api, /AcademicContextOptions/);
	assert.match(
		api,
		/apiClient\.get<AcademicContextOptionsResponse>\(\s*'\/api\/academic\/context\/options'/
	);
	assert.doesNotMatch(api, /apiClient\.(post|put|patch|delete)/);
	assert.doesNotMatch(api, /activate|is_active|semester/i);

	for (const requirement of ['none', 'year_required', 'term_required', 'term_optional']) {
		assert.match(types, new RegExp(`'${requirement}'`));
	}
	assert.match(types, /academicYearId:\s*string \| null/);
	assert.match(types, /academicTermId:\s*string \| null/);
	assert.match(store, /registerAcademicContextDirtySource/);
	assert.match(store, /createAcademicContextStore/);
	assert.match(store, /createContext<AcademicContextStore>/);
	assert.doesNotMatch(store, /localStorage|sessionStorage|activate|is_active/i);
});

test('route requirements are validated, inherited, and staff-only', async () => {
	const { createAcademicContextRouteResolver } = await importRouteContext();
	const resolveRequirement = createAcademicContextRouteResolver({
		'/src/routes/(app)/staff/academic/delivery/+page.ts': {
			_meta: { academicContext: 'term_required' }
		},
		'/src/routes/(app)/staff/academic/catalog/+page.ts': {
			_meta: { academicContext: 'none' }
		},
		'/src/routes/(app)/student/timetable/+page.ts': {
			_meta: { academicContext: 'term_required' }
		},
		'/src/routes/(app)/staff/profile/+page.ts': {
			_meta: {}
		}
	});

	assert.equal(resolveRequirement('/(app)/staff/academic/delivery'), 'term_required');
	assert.equal(resolveRequirement('/(app)/staff/academic/delivery/offering-1'), 'term_required');
	assert.equal(resolveRequirement('/(app)/staff/academic/catalog'), 'none');
	assert.equal(resolveRequirement('/(app)/student/timetable'), 'none');
	assert.equal(resolveRequirement('/(app)/staff/profile'), 'none');
	assert.equal(resolveRequirement(null), 'none');

	assert.throws(
		() =>
			createAcademicContextRouteResolver({
				'/src/routes/(app)/staff/academic/broken/+page.ts': {
					_meta: { academicContext: 'sometimes' }
				}
			}),
		/Invalid academic context requirement/
	);
});

test('URL resolution preserves deep links and enforces term ownership', async () => {
	const { resolveAcademicContextUrl } = await importRouteContext();
	const options = contextOptions();

	const missing = resolveAcademicContextUrl(
		'term_required',
		options,
		new URL('https://school.test/staff/academic/delivery?view=grid#offerings')
	);
	assert.equal(missing.status, 'ready');
	assert.deepEqual(missing.selected, {
		academicYearId: 'year-active',
		academicTermId: 'term-active'
	});
	assert.equal(missing.replaceUrl.searchParams.get('view'), 'grid');
	assert.equal(missing.replaceUrl.searchParams.get('academicYearId'), 'year-active');
	assert.equal(missing.replaceUrl.searchParams.get('academicTermId'), 'term-active');
	assert.equal(missing.replaceUrl.hash, '#offerings');

	const inconsistent = resolveAcademicContextUrl(
		'term_required',
		options,
		new URL(
			'https://school.test/staff/academic/delivery?academicYearId=year-planning&academicTermId=term-active'
		)
	);
	assert.equal(inconsistent.status, 'unavailable');
	assert.equal(inconsistent.replaceUrl, null);

	const optional = resolveAcademicContextUrl(
		'term_optional',
		options,
		new URL('https://school.test/staff/academic/supervision?academicYearId=year-active')
	);
	assert.equal(optional.status, 'ready');
	assert.equal(optional.selected.academicTermId, null);

	const hidden = resolveAcademicContextUrl(
		'none',
		options,
		new URL(
			'https://school.test/staff/profile?academicYearId=year-active&academicTermId=term-active'
		)
	);
	assert.equal(hidden.status, 'hidden');
	assert.deepEqual(hidden.selected, { academicYearId: null, academicTermId: null });
	assert.equal(hidden.replaceUrl, null);
});

test('route discovery, layout initialization, and responsive topbar remain explicit', async () => {
	const routeContext = await readProjectFile('src/lib/academic-context/route-context.ts');
	const layout = await readProjectFile('src/routes/(app)/+layout.svelte');
	const header = await readProjectFile('src/lib/components/layout/Header.svelte');
	const switcher = await readProjectFile(
		'src/lib/components/layout/AcademicContextSwitcher.svelte'
	);

	assert.match(routeContext, /import\.meta\.glob\('\/src\/routes\/\(app\)\/\*\*\/\+page\.ts'/);
	assert.match(routeContext, /eager:\s*true/);
	assert.match(layout, /authStatus === 'authenticated'/);
	assert.match(layout, /setAcademicContextStore/);
	assert.match(layout, /academicContext\.sync/);
	assert.match(layout, /academicContext\.reset/);
	assert.match(header, /<AcademicContextSwitcher/);
	assert.match(switcher, /<Select\.Root/);
	assert.match(switcher, /<Sheet\.Root/);
	assert.match(switcher, /<AlertDialog\.Root/);
	assert.match(switcher, /ทั้งปี/);
	assert.match(switcher, /ปีการศึกษา/);
	assert.match(switcher, /ภาคเรียน/);
	for (const label of ['กำลังวางแผน', 'พร้อมใช้งาน', 'กำลังใช้งาน', 'ปิดแล้ว']) {
		assert.match(switcher, new RegExp(label));
	}
	assert.doesNotMatch(`${layout}\n${header}\n${switcher}`, /activate|is_active/i);
});
