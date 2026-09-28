import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const readRoute = (route, file) =>
	readFile(
		new URL(
			`../../src/routes/(app)/staff/academic/admission/[id]/applications/${route}/${file}`,
			import.meta.url
		),
		'utf8'
	);

test('application list owns its primary read in the route and keeps filters URL-backed', async () => {
	const [route, page] = await Promise.all([
		readRoute('', '+page.ts'),
		readRoute('', '+page.svelte')
	]);
	assert.match(route, /listApplications\(/);
	assert.match(route, /requestFetch: fetch/);
	assert.match(route, /url\.searchParams/);
	assert.match(page, /data\.applications/);
	assert.doesNotMatch(page, /onMount\(loadApps\)/);
});

test('application detail starts only its selected detail, leaving track options action-lazy', async () => {
	const [route, page] = await Promise.all([
		readRoute('[appId]', '+page.ts'),
		readRoute('[appId]', '+page.svelte')
	]);
	assert.match(route, /getApplication\(/);
	assert.match(route, /requestFetch: fetch/);
	assert.doesNotMatch(route, /listTracks\(/);
	assert.match(page, /data\.applicationResult/);
	assert.match(page, /listTracks\(/);
	assert.doesNotMatch(page, /Promise\.all\(\[\s*getApplication/);
	assert.doesNotMatch(page, /\bonMount\b/);
});

test('staff application list no longer sends decrypted national IDs per row', async () => {
	const service = await readFile(
		new URL(
			'../../../backend-school/crates/school-admission/src/services/application_service.rs',
			import.meta.url
		),
		'utf8'
	);
	const list = service.slice(
		service.indexOf('pub struct AppListRow'),
		service.indexOf('pub async fn get_application_with_documents')
	);
	assert.doesNotMatch(list, /pub national_id:/);
	assert.doesNotMatch(list, /aa\.national_id\s*,/);
	assert.doesNotMatch(list, /decrypt_national_id\(&mut row\.national_id\)/);
	const page = await readRoute('', '+page.svelte');
	assert.doesNotMatch(page, /app\.nationalId/);
});

test('national-ID search has a permission-gated POST contract and no GET query hash', async () => {
	const [service, handler, routes, contract, api] = await Promise.all([
		readFile(
			new URL(
				'../../../backend-school/crates/school-admission/src/services/application_service.rs',
				import.meta.url
			),
			'utf8'
		),
		readFile(
			new URL(
				'../../../backend-school/src/modules/admission/handlers/applications.rs',
				import.meta.url
			),
			'utf8'
		),
		readFile(new URL('../../../backend-school/src/modules/admission.rs', import.meta.url), 'utf8'),
		readFile(new URL('../../src/lib/api/generated/school-api.ts', import.meta.url), 'utf8'),
		readFile(new URL('../../src/lib/api/admission.ts', import.meta.url), 'utf8')
	]);
	const list = service.slice(
		service.indexOf('pub async fn list_applications('),
		service.indexOf('pub async fn get_application_with_documents')
	);
	assert.match(list, /search_applications_by_identifier\(/);
	assert.doesNotMatch(list, /hash_required\(search\)/);
	assert.match(list, /aa\.application_number =/);
	assert.match(handler, /search_applications_by_identifier[\s\S]*?ADMISSION_READ_ALL/);
	assert.match(routes, /search-by-identifier[\s\S]*?post\(/);
	assert.match(contract, /searchAdmissionApplicationsByIdentifier/);
	assert.match(api, /Schemas\['SearchApplicationByIdentifierRequest'\]/);
});
