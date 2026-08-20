import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

test('certificate workspace is permission-derived and route-backed', async () => {
	const meta = await readProjectFile('src/routes/(app)/staff/certificates/+page.ts');
	const layout = await readProjectFile(
		'src/routes/(app)/staff/certificates/[campaignId]/+layout.svelte'
	);

	assert.match(meta, /PERMISSIONS\.CERTIFICATE_READ_ORGANIZATION_UNIT/);
	assert.match(meta, /PERMISSIONS\.CERTIFICATE_READ_SCHOOL/);
	assert.doesNotMatch(meta, /PERMISSION_MODULES\.CERTIFICATE/);
	assert.match(layout, /\/templates|\/recipients|\/requests|\/issued/);
	assert.doesNotMatch(layout, /certificate\.(read|create|update|delete)\./);
});

test('campaign screens use generated contracts and capability-driven controls', async () => {
	const form = await readProjectFile(
		'src/lib/components/certificates/CertificateCampaignForm.svelte'
	);
	const overview = await readProjectFile(
		'src/routes/(app)/staff/certificates/[campaignId]/overview/+page.svelte'
	);
	const api = await readProjectFile('src/lib/api/certificates.ts');

	assert.match(form, /AcademicYearLookupItem/);
	assert.match(form, /OrganizationUnitLookupItem/);
	assert.match(form, /กิจกรรมระดับโรงเรียน/);
	assert.doesNotMatch(form, /code\s*!==\s*['"]SCHOOL['"]\s*\?\s*true/);
	assert.match(overview, /capabilities\.can(Update|ChangeStatus|Delete)/);
	assert.match(api, /components\['schemas'\]/);
});
