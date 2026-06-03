import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

async function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('project rules require a single JSON API response envelope', async () => {
	const source = await readRepoFile('.rules');

	assert.match(source, /API Response Contract/);
	assert.match(source, /success:\s*true/);
	assert.match(source, /data:\s*T/);
	assert.match(source, /success:\s*false/);
	assert.match(source, /error:\s*string/);
});

test('backend auth success handlers return enveloped data', async () => {
	const source = await readRepoFile('backend-school/src/modules/auth/handlers.rs');

	assert.doesNotMatch(source, /Json\(user_response\)/);
	assert.doesNotMatch(source, /Json\(profile_response\)/);
	assert.doesNotMatch(source, /Json\(LoginResponse\s*\{/);
	assert.match(source, /ApiResponse::with_message\(\s*LoginData\s*\{[\s\S]*?\buser:/);
	assert.match(source, /ApiResponse::ok\(user_response\)/);
	assert.match(source, /ApiResponse::ok\(profile_response\)/);
});

test('backend app errors return the shared error envelope', async () => {
	const errorSource = await readRepoFile('backend-school/src/error.rs');
	const responseSource = await readRepoFile('backend-school/src/api_response.rs');

	assert.match(responseSource, /struct\s+ApiErrorResponse/);
	assert.match(responseSource, /success:\s*false/);
	assert.match(responseSource, /pub\s+error:\s+String/);
	assert.match(errorSource, /ApiErrorResponse::new\(error_message\)/);
	assert.doesNotMatch(errorSource, /json!\s*\(\s*\{/);
});

test('frontend auth consumes the shared envelope through apiClient', async () => {
	const source = await readRepoFile('frontend-school/src/lib/api/auth.ts');

	assert.match(source, /import\s+\{[^}]*\bapiClient\b[^}]*\}\s+from\s+['"]\$lib\/api\/client['"]/);
	assert.doesNotMatch(source, /\bfetch\s*\(/);
	assert.doesNotMatch(source, /\b(getRaw|postRaw|putRaw)\b/);
	assert.match(source, /\.data\?\.user/);
});

test('user role assignment API contract stays aligned across backend and frontend', async () => {
	const backendModels = await readRepoFile('backend-school/src/modules/staff/models.rs');
	const backendService = await readRepoFile(
		'backend-school/src/modules/staff/services/user_role_service.rs'
	);
	const frontendApi = await readRepoFile('frontend-school/src/lib/api/roles.ts');
	const frontendComponent = await readRepoFile(
		'frontend-school/src/lib/components/UserRoleManager.svelte'
	);

	assert.match(backendModels, /struct\s+UserRoleAssignmentResponse/);
	assert.match(backendModels, /pub\s+role:\s+Role/);
	assert.match(backendService, /Result<Vec<UserRoleAssignmentResponse>,\s*AppError>/);
	assert.doesNotMatch(backendService, /Result<Vec<Role>/);
	assert.match(backendService, /FROM user_roles ur/);
	assert.match(backendService, /ur\.role_id/);
	assert.match(backendService, /LEFT JOIN role_permissions rp/);
	assert.match(backendService, /AS role_permissions/);
	assert.match(backendService, /role:\s+Role\s*\{/);

	assert.match(frontendApi, /interface\s+UserRoleAssignment/);
	assert.match(frontendApi, /role:\s+Role/);
	assert.match(frontendApi, /getUserRoles\(userId:\s*string\):\s*Promise<ApiResponse<UserRoleAssignment\[\]>>/);
	assert.doesNotMatch(frontendApi, /interface\s+UserRole\s*\{/);

	assert.match(frontendComponent, /type\s+UserRoleAssignment/);
	assert.match(frontendComponent, /userRole\.role/);
	assert.doesNotMatch(frontendComponent, /getRoleById\(userRole\.role_id\)/);
});
