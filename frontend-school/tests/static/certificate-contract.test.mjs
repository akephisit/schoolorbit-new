import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

test('certificate permission contract exposes the complete approved capability set', async () => {
	const contract = JSON.parse(
		await readFile(path.join(repoRoot, 'contracts/permissions.json'), 'utf8')
	);
	const actualCertificateCodes = contract.permissions
		.filter((permission) => permission.module === 'certificate')
		.map((permission) => `${permission.module}.${permission.action}.${permission.scope}`);
	const expected = [
		'certificate.read.own',
		'certificate.read.organization_unit',
		'certificate.read.school',
		'certificate.create.organization_unit',
		'certificate.create.school',
		'certificate.update.organization_unit',
		'certificate.update.school',
		'certificate.delete.organization_unit',
		'certificate.delete.school',
		'certificate.submit.organization_unit',
		'certificate.submit.school',
		'certificate.issue.school',
		'certificate.revoke.school',
		'certificate.download.organization_unit',
		'certificate.download.school'
	];

	assert.deepEqual(actualCertificateCodes.toSorted(), expected.toSorted());
});
