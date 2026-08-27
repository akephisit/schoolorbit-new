import assert from 'node:assert/strict';
import { access, readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const repoRoot = path.resolve(projectRoot, '..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

async function sourceFiles(directory) {
	const entries = await readdir(path.join(projectRoot, directory), { withFileTypes: true });
	const files = [];
	for (const entry of entries) {
		const relativePath = path.join(directory, entry.name);
		if (entry.isDirectory()) {
			if (relativePath === 'src/lib/api/generated') continue;
			files.push(...(await sourceFiles(relativePath)));
		} else if (/\.(?:ts|svelte)$/.test(entry.name)) {
			files.push(relativePath);
		}
	}
	return files;
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

async function importScopedYear() {
	const source = await readProjectFile('src/lib/academic-context/scoped-year.ts');
	const compiled = ts.transpileModule(source, {
		compilerOptions: {
			module: ts.ModuleKind.ESNext,
			target: ts.ScriptTarget.ES2022,
			verbatimModuleSyntax: true
		},
		fileName: 'scoped-year.ts'
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

test('scoped academic year resolver repairs missing and unauthorized URLs', async () => {
	const { resolveScopedAcademicYearUrl } = await importScopedYear();
	const options = contextOptions();

	const missing = resolveScopedAcademicYearUrl(options, new URL('https://school.test/student'));
	assert.equal(missing.academicYearId, 'year-active');
	assert.equal(missing.replaceUrl?.searchParams.get('academicYearId'), 'year-active');

	const unauthorized = resolveScopedAcademicYearUrl(
		options,
		new URL('https://school.test/student?academicYearId=year-other&academicTermId=term-other')
	);
	assert.equal(unauthorized.academicYearId, 'year-active');
	assert.equal(unauthorized.replaceUrl?.searchParams.get('academicYearId'), 'year-active');
	assert.equal(unauthorized.replaceUrl?.searchParams.has('academicTermId'), false);

	const empty = resolveScopedAcademicYearUrl(
		{ ...options, activeAcademicYearId: null, activeAcademicTermId: null, years: [], terms: [] },
		new URL('https://school.test/student')
	);
	assert.deepEqual(empty, { academicYearId: null, replaceUrl: null });
});

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
	assert.match(switcher, /bind:value=\{yearSelectValue\}/);
	assert.match(switcher, /bind:value=\{termSelectValue\}/);
	assert.match(switcher, /resetSelectValues\(\)/);
	assert.match(switcher, /ทั้งปี/);
	assert.match(switcher, /ปีการศึกษา/);
	assert.match(switcher, /ภาคเรียน/);
	for (const label of ['กำลังวางแผน', 'พร้อมใช้งาน', 'กำลังใช้งาน', 'ปิดแล้ว']) {
		assert.match(switcher, new RegExp(label));
	}
	assert.doesNotMatch(`${layout}\n${header}\n${switcher}`, /activate|is_active/i);
});

test('desktop academic context triggers stay compact while dropdowns retain statuses', async () => {
	const switcher = await readProjectFile(
		'src/lib/components/layout/AcademicContextSwitcher.svelte'
	);
	const desktopStart = switcher.indexOf('class="hidden h-11');
	const desktopEnd = switcher.indexOf('<Sheet.Root', desktopStart);
	assert.ok(desktopStart >= 0 && desktopEnd > desktopStart);
	const desktop = switcher.slice(desktopStart, desktopEnd);
	const triggers = [...desktop.matchAll(/<Select\.Trigger[\s\S]*?<\/Select\.Trigger>/g)].map(
		(match) => match[0]
	);
	const contents = [...desktop.matchAll(/<Select\.Content[\s\S]*?<\/Select\.Content>/g)].map(
		(match) => match[0]
	);

	assert.equal(triggers.length, 2);
	for (const trigger of triggers) {
		assert.doesNotMatch(trigger, /บริบทงาน/);
		assert.doesNotMatch(trigger, /statusLabels/);
		assert.doesNotMatch(trigger, /<Badge/);
	}
	assert.ok(contents.length >= 2);
	for (const content of contents) {
		assert.match(content, /statusLabels/);
		assert.match(content, /<Badge/);
	}
});

test('existing staff academic consumers declare their exact context requirement', async () => {
	const routes = [
		['staff', 'year_required'],
		['staff/students', 'year_required'],
		['staff/academic/assessments', 'term_required'],
		['staff/academic/timetable', 'term_required'],
		['staff/academic/periods', 'year_required'],
		['staff/academic/exam-schedules', 'term_required'],
		['staff/academic/question-bank', 'none'],
		['staff/academic/supervision', 'term_optional'],
		['staff/academic/admission', 'year_required'],
		['staff/timetable', 'term_required'],
		['staff/exams', 'term_required']
	];

	for (const [route, requirement] of routes) {
		const metadata = await readProjectFile(`src/routes/(app)/${route}/+page.ts`);
		assert.match(
			metadata,
			new RegExp(`academicContext:\\s*['"]${requirement}['"]`),
			`${route} must declare ${requirement}`
		);
	}
});

test('staff student views consume the selected academic year without legacy paging fields', async () => {
	const listPage = await readProjectFile('src/routes/(app)/staff/students/+page.svelte');
	const detailPage = await readProjectFile('src/routes/(app)/staff/students/[id]/+page.svelte');
	const editPage = await readProjectFile('src/routes/(app)/staff/students/[id]/edit/+page.svelte');

	for (const source of [listPage, detailPage, editPage]) {
		assert.match(source, /getAcademicContextStore/);
		assert.match(source, /academicYearId/);
	}
	assert.doesNotMatch(listPage, /page_size\s*:|total_pages|class_room/);
	assert.match(listPage, /result\.page_size/);
	assert.match(listPage, /homeroom/);
});

test('student dashboard and profile select only authorized academic years', async () => {
	const dashboard = await readProjectFile('src/routes/(app)/student/+page.svelte');
	const profile = await readProjectFile('src/routes/(app)/student/profile/+page.svelte');
	const selector = await readProjectFile(
		'src/lib/components/academic-context/ScopedAcademicYearSelect.svelte'
	);

	for (const source of [dashboard, profile]) {
		assert.match(source, /listMyAcademicContextOptions/);
		assert.match(source, /resolveScopedAcademicYearUrl/);
		assert.match(source, /getOwnProfile\(selectedYearId\)/);
		assert.match(source, /ScopedAcademicYearSelect/);
		assert.match(source, /ยังไม่มีประวัติปีการศึกษาสำหรับบัญชีนี้/);
	}
	assert.match(selector, /years:\s*AcademicYearOption\[\]/);
	assert.match(selector, /onchange:\s*\(academicYearId:\s*string\)\s*=>\s*void/);
});

test('parent and child pages select only linked academic years', async () => {
	const parentHome = await readProjectFile('src/routes/(app)/parent/+page.svelte');
	const childDetail = await readProjectFile('src/routes/(app)/parent/student/[id]/+page.svelte');
	const childTimetable = await readProjectFile(
		'src/routes/(app)/parent/student/[id]/timetable/+page.svelte'
	);

	assert.match(parentHome, /listParentAcademicContextOptions/);
	assert.match(parentHome, /resolveScopedAcademicYearUrl/);
	assert.match(parentHome, /getOwnParentProfile\(selectedYearId\)/);
	assert.match(parentHome, /ScopedAcademicYearSelect/);

	for (const source of [childDetail, childTimetable]) {
		assert.match(source, /listChildAcademicContextOptions\(studentId\)/);
		assert.match(source, /getChildProfile\(studentId, selectedYearId\)/);
		assert.match(source, /academicYearId/);
	}
	assert.match(childDetail, /resolveScopedAcademicYearUrl/);
	assert.match(childDetail, /ScopedAcademicYearSelect/);
	assert.match(childTimetable, /loadTimetable\(selectedTermId, current\)/);
});

test('student and parent history selectors use learner-scoped academic context endpoints', async () => {
	const api = await readProjectFile('src/lib/api/academic-context.ts');
	const parentsApi = await readProjectFile('src/lib/api/parents.ts');
	const app = await readFile(path.join(repoRoot, 'backend-school/src/app.rs'), 'utf8');
	const coreHandlers = await readFile(
		path.join(repoRoot, 'backend-school/src/modules/academic/core/handlers.rs'),
		'utf8'
	);
	const coreService = await readFile(
		path.join(repoRoot, 'backend-school/src/modules/academic/core/services/context.rs'),
		'utf8'
	);
	const parentHandlers = await readFile(
		path.join(repoRoot, 'backend-school/src/modules/parents/handlers.rs'),
		'utf8'
	);

	assert.match(api, /listMyAcademicContextOptions/);
	assert.match(api, /\/api\/me\/academic-context\/options/);
	assert.match(api, /listParentAcademicContextOptions/);
	assert.match(api, /\/api\/parent\/academic-context\/options/);
	assert.match(api, /listChildAcademicContextOptions/);
	assert.match(
		api,
		/\/api\/parent\/students\/\$\{encodeURIComponent\(studentId\)\}\/academic-context\/options/
	);
	assert.match(app, /"\/api\/me\/academic-context\/options"/);
	assert.match(app, /"\/api\/parent\/academic-context\/options"/);
	assert.match(app, /"\/api\/parent\/students\/\{student_id\}\/academic-context\/options"/);
	assert.match(coreHandlers, /pub async fn list_my_context_options/);
	assert.match(parentHandlers, /pub async fn get_child_academic_context_options/);
	assert.match(parentHandlers, /pub async fn get_parent_academic_context_options/);
	assert.match(coreService, /pub async fn list_options_for_student/);
	assert.match(coreService, /pub async fn list_options_for_parent/);
	assert.match(coreService, /student_academic_years/);
	assert.match(coreService, /student_id\s*=\s*\$1/);
	assert.match(parentsApi, /operations\['getParentChildTimetable'\]\['parameters'\]\['query'\]/);
	assert.match(parentsApi, /const query = \{ academicTermId:/);
	assert.match(parentsApi, /\{ query \}/);
	assert.doesNotMatch(parentsApi, /\?academicTermId=/);
	assert.doesNotMatch(parentsApi, /academicSemesterId|academic_semester_id|TimetableEntryDto/);
});

test('admission round listing is scoped by the selected academic year', async () => {
	const api = await readProjectFile('src/lib/api/admission.ts');
	const handlers = await readFile(
		path.join(repoRoot, 'backend-school/src/modules/admission/handlers/rounds.rs'),
		'utf8'
	);
	const service = await readFile(
		path.join(repoRoot, 'backend-school/src/modules/admission/services/round_service.rs'),
		'utf8'
	);

	assert.match(
		api,
		/listRounds\(academicYearId:\s*string,\s*options:\s*ApiRequestOptions\s*=\s*\{\}\)/
	);
	assert.match(api, /\/api\/admission\/rounds\?academicYearId=/);
	assert.match(handlers, /struct AdmissionRoundQuery/);
	assert.match(handlers, /Query\(query\): Query<AdmissionRoundQuery>/);
	assert.match(service, /WHERE ar\.academic_year_id = \$1/);
});

test('calendar consumers use explicit year and optional term contexts', async () => {
	const api = await readProjectFile('src/lib/api/calendar.ts');
	const academicCoreApi = await readProjectFile('src/lib/api/academic-core.ts');
	const staffMetadata = await readProjectFile('src/routes/(app)/staff/calendar/+page.ts');
	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
	const studentPage = await readProjectFile('src/routes/(app)/student/calendar/+page.svelte');
	const parentPage = await readProjectFile(
		'src/routes/(app)/parent/student/[id]/calendar/+page.svelte'
	);

	assert.match(staffMetadata, /academicContext:\s*['"]term_optional['"]/);
	for (const operationId of [
		'listCalendarEvents',
		'listMyCalendarEvents',
		'getParentChildCalendarEvents',
		'listPublicCalendarEvents'
	]) {
		assert.match(api, new RegExp(`operations\\['${operationId}'\\]`));
	}
	assert.match(api, /apiClient\.get<CalendarEventDto\[]>\('\/api\/calendar\/events',\s*\{/);
	assert.match(api, /query:\s*\{\s*\.\.\.filters\s*\}/);
	assert.doesNotMatch(api, /URLSearchParams|params\.set|calendarQuery|publicCalendarQuery/);
	assert.doesNotMatch(`${academicCoreApi}\n${api}`, /academic_year_id|category_id|tag_id/);
	assert.doesNotMatch(api, /classRoomId/);
	assert.match(staffPage, /getAcademicContextStore/);
	assert.match(studentPage, /listMyAcademicContextOptions/);
	assert.match(studentPage, /academicYearId:\s*selectedYearId/);
	assert.match(parentPage, /listChildAcademicContextOptions/);
	assert.match(parentPage, /academicYearId:\s*selectedYearId/);
});

test('student activity registration uses learner term context and canonical delivery groups', async () => {
	const apiPath = path.join(projectRoot, 'src/lib/api/student-activities.ts');
	const apiExists = await access(apiPath).then(
		() => true,
		() => false
	);
	assert.equal(apiExists, true, 'student activity API wrapper must exist');

	const api = await readFile(apiPath, 'utf8');
	const page = await readProjectFile('src/routes/(app)/student/activities/+page.svelte');
	assert.match(api, /components\['schemas'\]/);
	assert.match(api, /operations\['listMyActivityRegistrations'\]/);
	assert.match(api, /\/api\/me\/activity-registrations/);
	assert.match(api, /academicTermId/);
	assert.match(page, /listMyAcademicContextOptions/);
	assert.match(page, /listMyActivityRegistrations/);
	assert.match(page, /academicTermId:\s*selectedTermId/);
	assert.doesNotMatch(page, /listActivitySlots|listActivityGroups|getMyActivityEnrollments/);
	assert.doesNotMatch(page, /Promise\.all/);
});

test('public calendar discovers its explicit academic context without authentication', async () => {
	const api = await readProjectFile('src/lib/api/academic-context.ts');
	const view = await readProjectFile('src/lib/components/calendar/PublicCalendarView.svelte');
	const app = await readFile(path.join(repoRoot, 'backend-school/src/app.rs'), 'utf8');
	const handlers = await readFile(
		path.join(repoRoot, 'backend-school/src/modules/academic/core/handlers.rs'),
		'utf8'
	);

	assert.match(api, /listPublicAcademicContextOptions/);
	assert.match(api, /\/api\/public\/academic-context\/options/);
	assert.match(view, /listPublicAcademicContextOptions/);
	assert.match(view, /academicYearId:\s*selectedYearId/);
	assert.match(app, /"\/api\/public\/academic-context\/options"/);
	assert.match(handlers, /pub async fn list_public_context_options/);
});

test('manual frontend sources contain no legacy academic wrapper, path, or wire vocabulary', async () => {
	await assert.rejects(access(path.join(projectRoot, 'src/lib/api/academic.ts')));
	const files = await sourceFiles('src');
	const violations = [];
	const forbidden = [
		/\$lib\/api\/academic['"]/,
		/\/api\/academic\/(?:semesters|structure|classrooms|enrollments|planning\/courses|subjects|study-plans)/,
		/\b(?:academic_semester_id|semester_id|classroom_course_id|student_class_enrollment_id|activity_slot_id)\b/,
		new RegExp(
			`\\b(?:${[
				['semester', 'Id'].join(''),
				['classroom', 'CourseId'].join(''),
				['student', 'ClassEnrollment'].join('')
			].join('|')})\\b`
		)
	];

	for (const file of files) {
		const source = await readProjectFile(file);
		if (forbidden.some((pattern) => pattern.test(source))) violations.push(file);
	}
	assert.deepEqual(violations, []);
});
