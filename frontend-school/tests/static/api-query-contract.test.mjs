import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const projectRoot = path.resolve(import.meta.dirname, '../..');

async function importTypescript(relativePath) {
	const source = await readFile(path.join(projectRoot, relativePath), 'utf8');
	const output = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
		fileName: relativePath
	}).outputText;
	return import(
		`data:text/javascript;base64,${Buffer.from(output).toString('base64')}#${Date.now()}`
	);
}

let apiWrapperImportRevision = 0;

async function importApiWrapper(relativePath) {
	globalThis.__schoolOrbitApiCalls = [];
	globalThis.__schoolOrbitApiResponseData = [];
	const clientModule = `
		export class ApiClientError extends Error {}
		export const apiClient = {
			get(endpoint, options) {
				globalThis.__schoolOrbitApiCalls.push({ method: 'get', endpoint, options });
				return Promise.resolve({
					success: true,
					data: globalThis.__schoolOrbitApiResponseData,
					status: 200
				});
			}
		};
		export function requireApiData(response, fallback) {
			if (!response.success || response.data === undefined) throw new Error(fallback);
			return response.data;
		}
	`;
	const clientUrl = `data:text/javascript;base64,${Buffer.from(clientModule).toString('base64')}`;
	const source = (await readFile(path.join(projectRoot, relativePath), 'utf8')).replace(
		/(['"])\$lib\/api\/client\1/g,
		`'${clientUrl}'`
	);
	const output = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
		fileName: relativePath
	}).outputText;
	apiWrapperImportRevision += 1;
	return import(
		`data:text/javascript;base64,${Buffer.from(output).toString('base64')}#${apiWrapperImportRevision}`
	);
}

test('appendApiQuery encodes defined scalars and existing queries', async () => {
	const { appendApiQuery } = await importTypescript('src/lib/api/query.ts');
	assert.equal(
		appendApiQuery('/api/students', {
			academicYearId: 'year/1',
			page: 2,
			activeOnly: false
		}),
		'/api/students?academicYearId=year%2F1&page=2&activeOnly=false'
	);
	assert.equal(
		appendApiQuery('/api/students?view=compact', { pageSize: 20 }),
		'/api/students?view=compact&pageSize=20'
	);
});

test('appendApiQuery repeats arrays and omits absent values', async () => {
	const { appendApiQuery } = await importTypescript('src/lib/api/query.ts');
	assert.equal(
		appendApiQuery('/api/students', {
			search: undefined,
			status: null,
			tagId: ['tag-a', 'tag-b']
		}),
		'/api/students?tagId=tag-a&tagId=tag-b'
	);
});

test('appendApiQuery rejects non-scalar query values', async () => {
	const { appendApiQuery } = await importTypescript('src/lib/api/query.ts');
	assert.throws(() => appendApiQuery('/api/students', { filter: { status: 'active' } }));
});

test('generated API exposes repaired academic query operations', async () => {
	const generated = await readFile(
		path.join(projectRoot, 'src/lib/api/generated/school-api.ts'),
		'utf8'
	);
	for (const operationId of [
		'listStudents',
		'getStudent',
		'getStudentProfile',
		'getParentProfile',
		'getParentChildProfile',
		'listParentAcademicContextOptions',
		'listCalendarEvents',
		'listMyCalendarEvents',
		'getParentChildCalendarEvents',
		'listPublicCalendarEvents',
		'listLearningGroupsForTerm',
		'listPlacementsForAcademicYear',
		'listHomeroomAdvisorsForAcademicYear',
		'listStudyProgramOptionsForAcademicYear',
		'getCurriculumProgramWorkspace',
		'getAcademicSetupWorkspace'
	]) {
		assert.match(generated, new RegExp(`\\b${operationId}: \\{`));
	}

	const listStudents = generated.match(/\n\tlistStudents: \{[\s\S]*?\n\t\};/)?.[0];
	assert.ok(listStudents, 'listStudents operation block must exist');
	assert.match(listStudents, /academicYearId:\s*string/);
	assert.match(listStudents, /pageSize\?:\s*number/);
	assert.doesNotMatch(listStudents, /academic_year_id|page_size/);

	for (const operationId of [
		'getStudent',
		'getStudentProfile',
		'getParentProfile',
		'getParentChildProfile',
		'listCalendarEvents',
		'listMyCalendarEvents',
		'getParentChildCalendarEvents',
		'listPublicCalendarEvents'
	]) {
		const operation = generated.match(
			new RegExp(`\\n\\t${operationId}: \\{[\\s\\S]*?\\n\\t\\};`)
		)?.[0];
		assert.ok(operation, `${operationId} operation block must exist`);
		assert.match(operation, /academicYearId:\s*string/);
		assert.doesNotMatch(operation, /academic_year_id|category_id|tag_id/);
	}

	assert.match(generated, /StudentListResponse:/);
	assert.match(generated, /homeroom:/);

	for (const [operationId, queryName] of [
		['listLearningGroupsForTerm', 'academicTermId'],
		['listPlacementsForAcademicYear', 'academicYearId'],
		['listHomeroomAdvisorsForAcademicYear', 'academicYearId'],
		['listStudyProgramOptionsForAcademicYear', 'academicYearId']
	]) {
		const operation = generated.match(
			new RegExp(`\\n\\t${operationId}: \\{[\\s\\S]*?\\n\\t\\};`)
		)?.[0];
		assert.ok(operation, `${operationId} operation block must exist`);
		assert.match(operation, new RegExp(`${queryName}:\\s*string`));
		assert.doesNotMatch(operation, /academic_year_id|academic_term_id/);
	}
});

test('academic batch wrappers send generated camelCase queries and preserve abort signals', async () => {
	const controller = new AbortController();
	const options = { signal: controller.signal };
	const academicCore = await importApiWrapper('src/lib/api/academic-core.ts');
	const delivery = await importApiWrapper('src/lib/api/learning-delivery.ts');

	for (const [call, endpoint, query] of [
		[
			() => delivery.listLearningGroupsForTerm('term-1', options),
			'/api/academic/learning-groups',
			{ academicTermId: 'term-1' }
		],
		[
			() => academicCore.listPlacementsForAcademicYear('year-1', options),
			'/api/academic/placements',
			{ academicYearId: 'year-1' }
		],
		[
			() => academicCore.listHomeroomAdvisorsForAcademicYear('year-1', options),
			'/api/academic/homeroom-advisors',
			{ academicYearId: 'year-1' }
		],
		[
			() => academicCore.listStudyProgramOptionsForAcademicYear('year-1', options),
			'/api/academic/study-program-options',
			{ academicYearId: 'year-1' }
		]
	]) {
		globalThis.__schoolOrbitApiResponseData = [];
		await call();
		assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
			method: 'get',
			endpoint,
			options: { signal: controller.signal, query }
		});
	}

	globalThis.__schoolOrbitApiResponseData = {};
	await academicCore.getCurriculumProgramWorkspace('version/1', options);
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/academic/curriculum-versions/version%2F1/program-workspace',
		options
	});
	await academicCore.getAcademicSetupWorkspace(options);
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/academic/setup/workspace',
		options
	});

	const wrapperSource = `${await readFile(path.join(projectRoot, 'src/lib/api/academic-core.ts'), 'utf8')}\n${await readFile(path.join(projectRoot, 'src/lib/api/learning-delivery.ts'), 'utf8')}`;
	assert.doesNotMatch(wrapperSource, /academic_year_id|academic_term_id/);
	assert.doesNotMatch(wrapperSource, /listStudyProgramOptionsForYear/);
});

test('grade-level wrapper sends a generated camelCase query object', async () => {
	const academicCore = await importApiWrapper('src/lib/api/academic-core.ts');
	await academicCore.listGradeLevelOptions('year-1');
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/lookup/grade-levels',
		options: { query: { academicYearId: 'year-1' } }
	});
});

test('calendar wrappers send generated query objects through the central transport', async () => {
	const calendar = await importApiWrapper('src/lib/api/calendar.ts');
	const filters = {
		academicYearId: 'year-1',
		academicTermId: 'term-1',
		from: '2026-08-01',
		to: '2026-08-31',
		categoryId: 'category-1',
		tagId: 'tag-1',
		audience: 'student',
		visibility: 'private',
		q: 'สอบ'
	};

	await calendar.listCalendarEvents(filters);
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/calendar/events',
		options: { query: filters }
	});

	await calendar.listMyCalendarEvents(filters);
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/me/calendar/events',
		options: { query: filters }
	});

	await calendar.listChildCalendarEvents('student/1', filters);
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/parent/students/student%2F1/calendar/events',
		options: { query: filters }
	});

	const publicFilters = {
		academicYearId: 'year-1',
		academicTermId: 'term-1',
		from: '2026-08-01',
		to: '2026-08-31',
		categoryId: 'category-1',
		tagId: 'tag-1',
		q: 'สอบ'
	};
	await calendar.listPublicCalendarEvents(publicFilters);
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/public/calendar/events',
		options: { query: publicFilters }
	});
});

test('staff student wrappers send generated academic-year queries', async () => {
	const students = await importApiWrapper('src/lib/api/students.ts');
	const query = {
		academicYearId: 'year-1',
		page: 2,
		pageSize: 20,
		search: 'สมชาย',
		status: 'active'
	};
	globalThis.__schoolOrbitApiResponseData = { items: [], page: 2, page_size: 20 };
	assert.deepEqual(await students.listStudents(query), {
		items: [],
		page: 2,
		page_size: 20
	});
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/students',
		options: { query }
	});

	globalThis.__schoolOrbitApiResponseData = {};
	await students.getStudent('student/1', 'year-1');
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/students/student%2F1',
		options: { query: { academicYearId: 'year-1' } }
	});
});

test('student own-profile wrapper sends the authorized academic year', async () => {
	const students = await importApiWrapper('src/lib/api/students.ts');
	globalThis.__schoolOrbitApiResponseData = { id: 'student-1' };

	assert.deepEqual(await students.getOwnProfile('year-1'), { id: 'student-1' });
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/student/profile',
		options: { query: { academicYearId: 'year-1' } }
	});
});

test('parent wrappers send generated linked-year and term queries', async () => {
	const parents = await importApiWrapper('src/lib/api/parents.ts');

	globalThis.__schoolOrbitApiResponseData = { id: 'parent-1' };
	assert.deepEqual(await parents.getOwnParentProfile('year-1'), { id: 'parent-1' });
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/parent/profile',
		options: { query: { academicYearId: 'year-1' } }
	});

	globalThis.__schoolOrbitApiResponseData = { id: 'student-1' };
	assert.deepEqual(await parents.getChildProfile('student/1', 'year-1'), {
		id: 'student-1'
	});
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/parent/students/student%2F1',
		options: { query: { academicYearId: 'year-1' } }
	});

	globalThis.__schoolOrbitApiResponseData = [];
	assert.deepEqual(await parents.getChildTimetable('student/1', ' term-1 '), []);
	assert.deepEqual(globalThis.__schoolOrbitApiCalls.pop(), {
		method: 'get',
		endpoint: '/api/parent/students/student%2F1/timetable',
		options: { query: { academicTermId: 'term-1' } }
	});
});
