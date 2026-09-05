import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { access, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(import.meta.dirname, '../../..');
const readRepo = (file) => readFile(path.join(repoRoot, file), 'utf8');

test('school session runtime is required and isolated from admin JWT', async () => {
	for (const file of ['docker-compose.yml', 'podman-compose.yml']) {
		const compose = await readRepo(file);
		const adminStart = compose.indexOf('  backend-admin:');
		const schoolStart = compose.indexOf('  backend-school:');
		const schoolEnd = compose.indexOf('  clamd:', schoolStart);
		assert.ok(adminStart >= 0 && schoolStart > adminStart && schoolEnd > schoolStart);

		const admin = compose.slice(adminStart, schoolStart);
		const school = compose.slice(schoolStart, schoolEnd);
		assert.match(admin, /JWT_SECRET[^\n]*\$\{JWT_SECRET/);
		assert.match(school, /JWT_SECRET[^\n]*\$\{SCHOOL_ROLLBACK_JWT_SECRET/);
		assert.doesNotMatch(school, /JWT_SECRET[^\n]*\$\{JWT_SECRET/);
		assert.match(school, /SESSION_HMAC_KEY[^\n]*\$\{SESSION_HMAC_KEY/);
		assert.match(school, /BASE_DOMAIN[^\n]*\$\{BASE_DOMAIN/);
		assert.match(school, /TRUSTED_PROXY_CIDRS/);
		assert.match(school, /SCHOOL_ALLOWED_DEV_ORIGINS/);
		if (file === 'podman-compose.yml') {
			assert.match(school, /SCHOOL_ALLOWED_DEV_ORIGINS:\s*\$\{SCHOOL_ALLOWED_DEV_ORIGINS\}/);
			assert.doesNotMatch(school, /SCHOOL_ALLOWED_DEV_ORIGINS[^\n]*\$\{[^\n]*:-\}/);
		}
	}

	const config = await readRepo('scripts/lib/schoolorbit-installer/config.sh');
	const vps = await readRepo('scripts/lib/schoolorbit-installer/vps.sh');
	assert.match(config, /SO_REQUIRED_SECRETS[\s\S]*SESSION_HMAC_KEY/);
	assert.match(config, /SO_REQUIRED_SECRETS[\s\S]*SCHOOL_ROLLBACK_JWT_SECRET/);
	assert.match(vps, /_dotenv_line SESSION_HMAC_KEY/);
	assert.match(vps, /_dotenv_line SCHOOL_ROLLBACK_JWT_SECRET/);
});

test('backend-school deployment validates session runtime before compose activation', async () => {
	const workflow = await readRepo('.github/workflows/deploy-backend-school.yml');
	const validatorStart = workflow.indexOf('            runtime_env_value() {');
	const exportDefinition = workflow.indexOf('            export_school_compose_env() {');
	const composeActivation = workflow.indexOf(
		'            cp "$runtime_source" "${runtime_compose}.next"'
	);

	assert.ok(validatorStart >= 0, 'runtime environment decoder must exist');
	assert.ok(
		composeActivation > validatorStart,
		'session runtime validation must run before the canonical Compose file is activated'
	);
	const guard = workflow.slice(validatorStart, composeActivation);
	for (const name of [
		'JWT_SECRET',
		'SESSION_HMAC_KEY',
		'SCHOOL_ROLLBACK_JWT_SECRET',
		'BASE_DOMAIN',
		'TRUSTED_PROXY_CIDRS',
		'SCHOOL_ALLOWED_DEV_ORIGINS'
	]) {
		assert.match(guard, new RegExp(`runtime_env_value ${name}`));
	}
	assert.match(guard, /\$\{#session_hmac_key\}[^\n]*-lt 32/);
	assert.match(guard, /\$\{#school_rollback_jwt_secret\}[^\n]*-lt 32/);
	assert.match(guard, /session_hmac_key[^\n]*school_rollback_jwt_secret/);
	assert.match(guard, /school_rollback_jwt_secret[^\n]*admin_jwt_secret/);
	assert.match(guard, /runtime_base_domain[^\n]*base_domain/);
	assert.doesNotMatch(
		guard,
		/echo[^\n]*(?:session_hmac_key|school_rollback_jwt_secret|admin_jwt_secret)/
	);

	assert.ok(exportDefinition > validatorStart, 'the decoded runtime bridge must be defined');
	const exportDefinitionEnd = workflow.indexOf('            }', exportDefinition);
	const exportBridge = workflow.slice(exportDefinition, exportDefinitionEnd);
	for (const name of [
		'SESSION_HMAC_KEY',
		'SCHOOL_ROLLBACK_JWT_SECRET',
		'BASE_DOMAIN',
		'TRUSTED_PROXY_CIDRS',
		'SCHOOL_ALLOWED_DEV_ORIGINS'
	]) {
		assert.match(exportBridge, new RegExp(`export ${name}=`));
	}

	const firstExport = workflow.indexOf(
		'            export_school_compose_env\n',
		exportDefinitionEnd
	);
	const composeDryRun = workflow.indexOf('            podman-compose -f', firstExport);
	const firstUnset = workflow.indexOf('            unset_school_compose_env\n', composeDryRun);
	const backendCompose = workflow.indexOf('            compose_up_quiet --no-deps backend-school');
	const secondExport = workflow.lastIndexOf(
		'            export_school_compose_env\n',
		backendCompose
	);
	const secondUnset = workflow.indexOf('            unset_school_compose_env\n', backendCompose);
	assert.ok(
		firstExport > exportDefinitionEnd && composeDryRun > firstExport && firstUnset > composeDryRun,
		'the canonical dry run must use and then clear decoded school runtime values'
	);
	assert.ok(
		secondExport > firstUnset && backendCompose > secondExport && secondUnset > backendCompose,
		'the backend replacement must use and then clear decoded school runtime values'
	);
});

test('the resolved production topology has one owner and private backend ports', async () => {
	const { stdout } = await execFileAsync(
		'docker',
		[
			'compose',
			'--env-file',
			'scripts/tests/installer/fixtures/runtime.env',
			'-f',
			'podman-compose.yml',
			'config',
			'--format',
			'json'
		],
		{ cwd: repoRoot }
	);
	const topology = JSON.parse(stdout);

	for (const standalone of [
		'backend-admin/docker-compose.yml',
		'backend-school/docker-compose.yml'
	]) {
		await assert.rejects(access(path.join(repoRoot, standalone)));
	}
	assert.deepEqual(Object.keys(topology.services).sort(), [
		'backend-admin',
		'backend-school',
		'clamd',
		'nginx'
	]);
	assert.equal(topology.networks['schoolorbit-net'].name, 'schoolorbit-web');
	assert.equal(
		topology.networks['file-platform-internal'].name,
		'schoolorbit-file-platform-internal'
	);
	assert.equal(topology.networks['clamav-egress'].name, 'schoolorbit-clamav-egress');
	assert.equal(topology.volumes.clamav_signatures.name, 'schoolorbit-clamav-signatures');
	assert.equal(topology.services.nginx.depends_on, undefined);
	assert.equal(topology.services['backend-school'].depends_on, undefined);
	for (const [service, target] of [
		['backend-admin', 8080],
		['backend-school', 8081]
	]) {
		assert.deepEqual(topology.services[service].ports, [
			{
				mode: 'ingress',
				host_ip: '127.0.0.1',
				target,
				published: String(target),
				protocol: 'tcp'
			}
		]);
	}
});

test('local and production clamd allow 3 GiB for concurrent signature reloads', async () => {
	for (const [file, extraArguments] of [
		['docker-compose.yml', []],
		['podman-compose.yml', ['--env-file', 'scripts/tests/installer/fixtures/runtime.env']]
	]) {
		const { stdout } = await execFileAsync(
			'docker',
			['compose', ...extraArguments, '-f', file, 'config', '--format', 'json'],
			{ cwd: repoRoot }
		);
		const topology = JSON.parse(stdout);

		assert.equal(
			topology.services.clamd.mem_limit,
			String(3 * 1024 * 1024 * 1024),
			`${file} must preserve enough memory for concurrent ClamAV database reloads`
		);
	}
});

test('backend-school deployment reuses only an exact ClamAV runtime and verifies health', async () => {
	const workflow = await readRepo('.github/workflows/deploy-backend-school.yml');
	const scannerStart = workflow.indexOf(
		'# The scanner gets an isolated container network and no published port.'
	);
	const scannerEnd = workflow.indexOf('\n            jq_image=', scannerStart);

	assert.ok(scannerStart >= 0 && scannerEnd > scannerStart);
	const scannerDeployment = workflow.slice(scannerStart, scannerEnd);
	assert.match(workflow, /source: [^\n]*scripts\/clamd_runtime_matches\.sh/);
	assert.match(workflow, /clamd_matcher="\$deployment_root\/scripts\/clamd_runtime_matches\.sh"/);
	const orderedMarkers = [
		'clamd_image=docker.io/clamav/clamav-debian:1.5.3',
		'podman pull "$clamd_image"',
		'if clamd_match_output="$("$clamd_matcher" "$clamd_image" schoolorbit-clamd)"; then',
		`printf '%s\\n' "$clamd_match_output"`,
		`printf 'clamd_action=recreated reason=%s\\n' "$clamd_reason"`,
		'if podman container exists schoolorbit-clamd; then',
		'podman stop schoolorbit-clamd',
		'podman rm schoolorbit-clamd',
		'compose_up_quiet clamd',
		'expected_clamd_memory_bytes=$((3 * 1024 * 1024 * 1024))',
		`clamd_memory_bytes="$(podman inspect --format '{{.HostConfig.Memory}}' schoolorbit-clamd)"`,
		'if [ "$clamd_memory_bytes" != "$expected_clamd_memory_bytes" ]; then',
		'scanner_ready=false'
	];
	let previousIndex = -1;
	for (const marker of orderedMarkers) {
		const markerIndex = scannerDeployment.indexOf(marker);
		assert.ok(markerIndex > previousIndex, `${marker} must appear in deployment order`);
		previousIndex = markerIndex;
	}
	assert.match(scannerDeployment, /else[\s\S]*podman stop schoolorbit-clamd[\s\S]*fi/);
	assert.doesNotMatch(scannerDeployment, /podman volume (?:rm|prune)/);
});

test('backend-school replacement force-removes the stale container without touching dependencies', async () => {
	const workflow = await readRepo('.github/workflows/deploy-backend-school.yml');
	const replacementStart = workflow.indexOf(
		'# Recreate backend-school only; do not restart unrelated services.'
	);
	const readinessStart = workflow.indexOf(
		'# /ready checks control plane, both R2 buckets, and clamd.',
		replacementStart
	);

	assert.ok(replacementStart >= 0 && readinessStart > replacementStart);
	const replacement = workflow.slice(replacementStart, readinessStart);
	const orderedMarkers = [
		'if podman container exists schoolorbit-backend-school; then',
		'podman rm --force schoolorbit-backend-school'
	];
	let previousIndex = -1;
	for (const marker of orderedMarkers) {
		const markerIndex = replacement.indexOf(marker);
		assert.ok(markerIndex > previousIndex, `${marker} must appear in replacement order`);
		previousIndex = markerIndex;
	}
	const staleContainerCheck = replacement.indexOf(
		'if podman container exists schoolorbit-backend-school; then',
		previousIndex
	);
	const staleContainerError = replacement.indexOf(
		'echo "Stale backend-school container remains after forced removal"',
		staleContainerCheck
	);
	const composeUp = replacement.indexOf(
		'compose_up_quiet --no-deps backend-school',
		staleContainerError
	);
	assert.ok(
		staleContainerCheck > previousIndex,
		'the forced removal must verify the container is gone'
	);
	assert.ok(
		staleContainerError > staleContainerCheck,
		'a stale container must fail the deployment'
	);
	assert.ok(
		composeUp > staleContainerError,
		'compose must run only after stale-container verification'
	);
	assert.doesNotMatch(replacement, /podman update .*schoolorbit-backend-school/);
	assert.doesNotMatch(replacement, /podman stop .*schoolorbit-backend-school/);
	assert.doesNotMatch(replacement, /podman rm schoolorbit-backend-school \|\| true/);
	assert.doesNotMatch(replacement, /^\s*compose_up_quiet backend-school\s*$/m);
});

test('backend-school migration failure reports only bounded deployment diagnostics', async () => {
	const workflow = await readRepo('.github/workflows/deploy-backend-school.yml');
	const diagnosticStart = workflow.indexOf('            print_migration_verification_failure() {');
	const verificationFailure = workflow.indexOf(
		'              echo "Tenant migration verification failed; maintenance remains enabled"'
	);

	assert.ok(diagnosticStart >= 0, 'migration verification must define a bounded diagnostic');
	assert.ok(
		verificationFailure > diagnosticStart,
		'migration diagnostics must be available before verification can fail'
	);
	const diagnostic = workflow.slice(diagnosticStart, verificationFailure);
	assert.match(diagnostic, /tenant_migration_summary latest_version=/);
	assert.match(diagnostic, /tenant_migration_result subdomain=/);
	assert.match(diagnostic, /error_code=/);
	assert.match(diagnostic, /scan\("ACADEMIC_\[A-Z0-9_\]\+"\)/);
	assert.match(diagnostic, /academicDiagnostics\.currentTeacherConflicts\[\]\?/);
	assert.match(diagnostic, /tenant_academic_teacher_conflict subdomain=/);
	assert.match(diagnostic, /teacher_id=/);
	assert.match(diagnostic, /timetable_version_id=/);
	assert.match(diagnostic, /day=/);
	assert.match(diagnostic, /bell_schedule_period_id=/);
	assert.match(diagnostic, /entry_count=/);
	assert.match(diagnostic, /group_code_count=/);
	assert.match(diagnostic, /entry_ids=/);
	assert.match(diagnostic, /group_codes=/);
	assert.match(diagnostic, /tojson/);
	assert.match(diagnostic, /print_migration_verification_failure < "\$migration_response"/);
	assert.doesNotMatch(diagnostic, /error=\\\(\.error/);
	assert.doesNotMatch(diagnostic, /join\(","\)/);
	assert.doesNotMatch(diagnostic, /displayName|firstName|lastName/);

	const hostileGroupCodes = JSON.stringify(['SAFE', 'FORGED\nstatus=success']);
	assert.equal(hostileGroupCodes.includes('\n'), false);
});

test('backend-school deployment repairs the admin network alias before maintenance activation', async () => {
	const workflow = await readRepo('.github/workflows/deploy-backend-school.yml');
	const adminNetworkRepair = workflow.indexOf(
		'            reconnect_backend_network schoolorbit-backend-admin backend-admin'
	);
	const maintenanceActivation = workflow.indexOf(
		'            cp "$maintenance_proxy_source" "$proxy_target"'
	);

	assert.ok(
		adminNetworkRepair >= 0,
		'the admin container alias must be repaired before Nginx reloads'
	);
	assert.ok(
		maintenanceActivation > adminNetworkRepair,
		'the admin network alias must be repaired before the maintenance proxy is activated'
	);
});

test('backend workflows use the recoverable shared network alias helper', async () => {
	for (const workflowPath of [
		'.github/workflows/deploy-backend-admin.yml',
		'.github/workflows/deploy-backend-school.yml'
	]) {
		const workflow = await readRepo(workflowPath);
		const uploadedHelper = workflow.indexOf(
			'scripts/lib/schoolorbit-installer/remote/ensure_container_network.sh'
		);
		const sourcedHelper = workflow.indexOf('. "$network_helper_source"');
		const firstRepair = workflow.indexOf('reconnect_backend_network ');

		assert.ok(uploadedHelper >= 0, `${workflowPath} must upload the shared network helper`);
		assert.ok(sourcedHelper > uploadedHelper, `${workflowPath} must source the uploaded helper`);
		assert.ok(firstRepair > sourcedHelper, `${workflowPath} must load the helper before using it`);
		assert.doesNotMatch(
			workflow,
			/podman network disconnect -f schoolorbit-web "\$container"[^\n]*\|\| true/
		);
	}
});

test('the proxy renderer substitutes only a validated base domain', async (t) => {
	const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-nginx-'));
	t.after(() => rm(temporary, { recursive: true, force: true }));
	const output = path.join(temporary, 'school.conf');

	await execFileAsync(path.join(repoRoot, 'scripts/render_nginx_config.sh'), [
		path.join(repoRoot, 'nginx-configs/school-api.conf.template'),
		output,
		'example.test'
	]);

	const rendered = await readFile(output, 'utf8');
	assert.match(rendered, /server_name school-api\.example\.test;/);
	assert.match(rendered, /\(\[\\w-\]\+\\\.\)\?example\\\.test/);
	assert.match(rendered, /ssl_certificate \/etc\/nginx\/ssl\/schoolorbit-origin\.pem;/);
	assert.match(rendered, /ssl_certificate_key \/etc\/nginx\/ssl\/schoolorbit-origin\.key;/);
	assert.doesNotMatch(rendered, /\$\{BASE_DOMAIN(?:_REGEX)?\}/);
	assert.doesNotMatch(rendered, /schoolorbit\.app/);
});

test('the proxy renderer rejects an invalid domain without replacing its output', async (t) => {
	const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-nginx-invalid-'));
	t.after(() => rm(temporary, { recursive: true, force: true }));
	const output = path.join(temporary, 'school.conf');
	await writeFile(output, 'known-good\n');

	await assert.rejects(
		execFileAsync(path.join(repoRoot, 'scripts/render_nginx_config.sh'), [
			path.join(repoRoot, 'nginx-configs/school-api.conf.template'),
			output,
			'Example Test'
		]),
		(error) => error.code === 64 && error.stderr === 'Invalid base domain\n'
	);
	assert.equal(await readFile(output, 'utf8'), 'known-good\n');
});

test('school proxy permits and exposes the memory-only CSRF header', async () => {
	for (const file of [
		'nginx-configs/school-api.conf.template',
		'nginx-configs/school-api.maintenance.conf.template'
	]) {
		const source = await readRepo(file);
		const allowHeaderLines = source
			.split('\n')
			.filter((line) => line.includes('Access-Control-Allow-Headers'));
		assert.ok(allowHeaderLines.length > 0, `${file} must define allowed CORS headers`);
		for (const line of allowHeaderLines) assert.match(line, /X-CSRF-Token/);
		assert.match(source, /Access-Control-Expose-Headers[^\n]*X-CSRF-Token/);
	}
});

test('backend workflows deploy the canonical target and verify the selected origin', async () => {
	const originRootInstaller = await readRepo(
		'scripts/lib/schoolorbit-installer/remote/install_origin_root.sh'
	);
	assert.match(
		originRootInstaller,
		/https:\/\/developers\.cloudflare\.com\/ssl\/static\/origin_ca_rsa_root\.pem/
	);
	assert.match(
		originRootInstaller,
		/91a8a5567efa6bf941162aa806b3ba476aaddf7867640e53053b35fb225a5dae/
	);

	const workflowPortCounts = new Map([
		['.github/workflows/deploy-backend-admin.yml', 2],
		['.github/workflows/deploy-backend-school.yml', 4]
	]);
	for (const [file, expectedPortCount] of workflowPortCounts) {
		const workflow = await readRepo(file);
		assert.match(workflow, /podman-compose\.yml/);
		assert.match(workflow, /scripts\/render_nginx_config\.sh/);
		assert.match(workflow, /scripts\/lib\/schoolorbit-installer\/remote\/install_origin_root\.sh/);
		assert.match(workflow, /"\$origin_root_installer" "\$origin_root"/);
		assert.match(workflow, /deployment_id/);
		assert.match(workflow, /RUNTIME_DEPLOY_ENABLED/);
		assert.match(workflow, /--resolve/);
		assert.match(workflow, /cloudflare-origin-rsa-root\.pem/);
		assert.match(workflow, /\/opt\/stack\/deployment/);
		assert.match(workflow, /podman-compose -f "\$\{runtime_compose\}\.next" --dry-run up -d/);
		assert.match(
			workflow,
			/legacy_proxy_target=\/opt\/stack\/nginx\/conf\.d\/(?:admin|school)-api\.\$\{base_domain\}\.conf/
		);
		assert.match(workflow, /proxy_previous_target="\$proxy_target"/);
		assert.match(workflow, /rm -f "\$proxy_previous_target"/);
		assert.match(workflow, /compose_up_quiet\(\)/);
		assert.match(workflow, /podman-compose -f "\$runtime_compose" up -d "\$@" >\/dev\/null 2>&1/);
		assert.match(workflow, /reconnect_backend_network\(\)/);
		assert.match(workflow, /validate_nginx_config_with_retry\(\)/);
		assert.ok(
			(workflow.match(/validate_nginx_config_with_retry/g) ?? []).length >= 3,
			`${file} must retry Nginx validation during activation and recovery`
		);
		assert.match(
			workflow,
			/schoolorbit_ensure_container_network_aliases \\\n\s+schoolorbit-web "\$container" "\$service_alias" "\$container" schoolorbit-nginx/
		);
		assert.match(workflow, /podman rm schoolorbit-nginx >\/dev\/null 2>&1 \|\| true/);
		assert.match(workflow, /timeout 180 bash/);
		assert.match(workflow, /grep -lF "server_name/);
		assert.match(workflow, /group: deploy-schoolorbit-runtime/);
		assert.equal(
			(workflow.match(/port: \$\{\{ secrets\.SERVER_PORT \}\}/g) ?? []).length,
			expectedPortCount
		);
		assert.doesNotMatch(workflow, /backend-(?:admin|school)\/docker-compose\.yml/);
		assert.doesNotMatch(workflow, /file-platform-runtime/);
		assert.doesNotMatch(workflow, /"\$\{runtime_compose\}\.next" config/);
		assert.doesNotMatch(workflow, /curl[^\n]*https:\/\/(?:admin-api|school-api)\.schoolorbit\.app/);
	}
});

test('backend image workflows use distinct BuildKit cache scopes', async () => {
	const workflowScopes = new Map([
		['.github/workflows/deploy-backend-admin.yml', 'backend-admin'],
		['.github/workflows/deploy-backend-school.yml', 'backend-school']
	]);

	assert.equal(new Set(workflowScopes.values()).size, workflowScopes.size);
	for (const [file, scope] of workflowScopes) {
		const workflow = await readRepo(file);
		assert.ok(workflow.includes(`cache-from: type=gha,scope=${scope}`));
		assert.ok(workflow.includes(`cache-to: type=gha,scope=${scope},mode=max`));
		assert.ok(workflow.includes('- name: Summarize Docker cache scope'));
		assert.ok(workflow.includes(`'- Scope: ${scope}'`));
		assert.ok(workflow.includes('Docker build record'));
		assert.ok(workflow.includes('>> "$GITHUB_STEP_SUMMARY"'));
	}
});

test('backend runtime images use deterministic builders without ownership copy-up', async () => {
	const images = new Map([
		['backend-admin', 'backend-admin'],
		['backend-school', 'backend-school']
	]);

	for (const [directory, binary] of images) {
		const dockerfile = await readRepo(`${directory}/Dockerfile`);
		const runtimeStart = dockerfile.indexOf('FROM debian:bookworm-slim AS runtime');
		const runtime = dockerfile.slice(runtimeStart);
		const userCreate = runtime.indexOf('useradd -m -u 1000 appuser');
		const binaryCopy = runtime.indexOf(
			`COPY --chown=1000:1000 --from=builder /app/target/release/${binary} /app/${binary}`
		);

		assert.match(dockerfile, /^# syntax=docker\/dockerfile:1\.10$/m);
		assert.match(dockerfile, /FROM rust:1\.98\.0-slim-bookworm AS base/);
		assert.match(dockerfile, /cargo install cargo-chef --version 0\.1\.78 --locked/);
		assert.match(dockerfile, /sccache-v0\.17\.0-x86_64-unknown-linux-musl\.tar\.gz/);
		assert.match(
			dockerfile,
			/--checksum=sha256:67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006/
		);
		assert.match(dockerfile, /--mount=type=secret,id=sccache_gha_url,env=ACTIONS_RESULTS_URL/);
		assert.match(dockerfile, /--mount=type=secret,id=sccache_gha_token,env=ACTIONS_RUNTIME_TOKEN/);
		assert.match(dockerfile, /SCCACHE_GHA_ENABLED=on/);
		assert.match(dockerfile, new RegExp(`SCCACHE_GHA_CACHE_TO=schoolorbit-${binary}`));
		assert.match(dockerfile, /SCCACHE_IGNORE_SERVER_IO_ERROR=1/);
		assert.match(dockerfile, new RegExp(`cargo build --release --bin ${binary} --timings`));
		assert.match(dockerfile, /FROM scratch AS build-timings/);
		assert.match(
			dockerfile,
			/COPY --from=builder \/app\/target\/cargo-timings\/cargo-timing\.html \/cargo-timing\.html/
		);
		assert.ok(userCreate >= 0, `${directory} must create the runtime user`);
		assert.ok(binaryCopy > userCreate, `${directory} must create the runtime user before copying`);
		assert.match(runtime, /COPY --chown=1000:1000 migrations \.\/migrations/);
		assert.doesNotMatch(runtime, /RUN[^\n]*chown|chown -R/);
		assert.doesNotMatch(runtime, /sccache|cargo-timing/);
	}
});

test('backend workflows export Cargo timing artifacts with secret-mounted sccache credentials', async () => {
	const workflows = new Map([
		['.github/workflows/deploy-backend-admin.yml', 'backend-admin'],
		['.github/workflows/deploy-backend-school.yml', 'backend-school']
	]);

	for (const [file, backend] of workflows) {
		const workflow = await readRepo(file);

		assert.match(workflow, /uses: actions\/github-script@v8/);
		assert.match(
			workflow,
			/core\.exportVariable\('ACTIONS_RESULTS_URL', process\.env\.ACTIONS_RESULTS_URL \|\| ''\)/
		);
		assert.match(
			workflow,
			/core\.exportVariable\('ACTIONS_RUNTIME_TOKEN', process\.env\.ACTIONS_RUNTIME_TOKEN \|\| ''\)/
		);
		assert.match(workflow, /secret-envs:\s*\|\s*\n\s*sccache_gha_url=ACTIONS_RESULTS_URL/);
		assert.match(workflow, /sccache_gha_token=ACTIONS_RUNTIME_TOKEN/);
		assert.match(workflow, /target: build-timings/);
		assert.match(workflow, /push: false/);
		assert.match(
			workflow,
			new RegExp(`outputs: type=local,dest=\\$\\{\\{ runner\\.temp \\}\\}/cargo-timings-${backend}`)
		);
		assert.match(workflow, /uses: actions\/upload-artifact@v6/);
		assert.match(workflow, new RegExp(`name: cargo-timings-${backend}`));
		assert.match(
			workflow,
			new RegExp(`path: \\$\\{\\{ runner\\.temp \\}\\}/cargo-timings-${backend}/cargo-timing\\.html`)
		);
		assert.match(workflow, /retention-days: 7/);
		assert.doesNotMatch(workflow, /build-args:[^\n]*(?:ACTIONS_RESULTS_URL|ACTIONS_RUNTIME_TOKEN)/);
		assert.doesNotMatch(workflow, /^\s+secrets:\s*\|[\s\S]*ACTIONS_RUNTIME_TOKEN/m);
	}
});

test('backend workflows clean only bounded SchoolOrbit image history after acceptance', async () => {
	const workflowBoundaries = new Map([
		[
			'.github/workflows/deploy-backend-admin.yml',
			'            [ -z "$proxy_backup" ] || rm -f "$proxy_backup"'
		],
		[
			'.github/workflows/deploy-backend-school.yml',
			'            podman tag "${backend_image}:${{ github.sha }}" "${backend_image}:rollback"'
		]
	]);

	for (const [file, acceptanceMarker] of workflowBoundaries) {
		const workflow = await readRepo(file);
		const acceptance = workflow.indexOf(acceptanceMarker);
		const cleanup = workflow.indexOf('"$image_cleanup"', acceptance + acceptanceMarker.length);

		assert.match(workflow, /source: [^\n]*scripts\/prune_runtime_images\.sh/);
		assert.match(
			workflow,
			/image_cleanup="\$deployment_root\/scripts\/prune_runtime_images\.sh"/
		);
		assert.ok(acceptance >= 0, `${file} must retain its acceptance boundary`);
		assert.ok(cleanup > acceptance, `${file} must clean images only after acceptance`);
		assert.match(
			workflow.slice(cleanup),
			/"\$image_cleanup" ghcr\.io\/akephisit\/schoolorbit-backend-(?:admin|school) 3/
		);
		assert.doesNotMatch(workflow, /podman (?:system|volume|container|image) prune/);
	}
});

test('backend workflows emit bounded deployment phase timings', async () => {
	for (const file of [
		'.github/workflows/deploy-backend-admin.yml',
		'.github/workflows/deploy-backend-school.yml'
	]) {
		const workflow = await readRepo(file);
		assert.match(workflow, /scripts\/lib\/schoolorbit-installer\/remote\/deployment_timing\.sh/);
		assert.match(workflow, /\. "\$timing_helper_source"/);
		assert.match(workflow, /schoolorbit_timer_now/);
		assert.match(workflow, /schoolorbit_timer_report image_pull/);
		assert.match(workflow, /schoolorbit_timer_report backend_readiness/);
		assert.doesNotMatch(workflow, /deployment_timing[^\n]*(?:SECRET|TOKEN|PASSWORD|KEY|Env)/i);
	}
});

test('GHCR retention is bounded, dry-run by default, and isolated from deployment secrets', async () => {
	const workflow = await readRepo('.github/workflows/ghcr-retention.yml');
	const retention = await readRepo('scripts/prune_ghcr_versions.mjs');

	assert.match(workflow, /schedule:\s*\n\s*- cron:/);
	assert.match(workflow, /workflow_dispatch:[\s\S]*dry_run:[\s\S]*default: true/);
	assert.match(workflow, /permissions:\s*\n\s*contents: read\s*\n\s*packages: write/);
	assert.match(workflow, /GHCR_RETENTION_ENABLED/);
	assert.match(workflow, /github\.event_name == 'schedule'/);
	assert.match(workflow, /inputs\.dry_run == false/);
	assert.match(workflow, /schoolorbit-backend-admin/);
	assert.match(workflow, /schoolorbit-backend-school/);
	assert.match(workflow, /node scripts\/prune_ghcr_versions\.mjs/);
	assert.match(workflow, /--keep 30/);
	assert.match(workflow, /--execute/);
	assert.match(workflow, /GITHUB_TOKEN: \$\{\{ secrets\.GITHUB_TOKEN \}\}/);
	assert.doesNotMatch(
		workflow,
		/(?:SERVER_|SSH_|DATABASE_|R2_|CLOUDFLARE_|JWT_|INTERNAL_API_SECRET)/
	);

	assert.match(retention, /\^\[0-9a-f\]\{40\}\$/);
	assert.match(retention, /tags\.includes\('latest'\)/);
	assert.match(retention, /MAX_DELETIONS = 100/);
	assert.match(retention, /method: 'DELETE'/);
	assert.match(retention, /method = 'GET'/);
});

test('API contract runs artifact backend and frontend gates in independent jobs', async () => {
	const workflow = await readRepo('.github/workflows/api-contract.yml');
	const jobsStart = workflow.indexOf('\njobs:\n');
	assert.ok(jobsStart >= 0);
	const jobs = workflow.slice(jobsStart + '\njobs:\n'.length);
	const jobNames = [...jobs.matchAll(/^ {2}([a-z][a-z0-9_-]*):\s*$/gm)].map((match) => match[1]);
	assert.deepEqual(jobNames, ['artifacts', 'backend', 'frontend']);
	assert.doesNotMatch(jobs, /^ {4}needs:/gm);

	const jobBlock = (name, nextName) => {
		const start = jobs.indexOf(`  ${name}:\n`);
		assert.ok(start >= 0, `missing ${name} job`);
		const end = nextName ? jobs.indexOf(`\n  ${nextName}:\n`, start) : jobs.length;
		assert.ok(end > start, `invalid ${name} job boundary`);
		return jobs.slice(start, end);
	};
	const artifacts = jobBlock('artifacts', 'backend');
	const backend = jobBlock('backend', 'frontend');
	const frontend = jobBlock('frontend');

	for (const command of [
		'npm run test:api-contracts',
		'npm run check:api-contracts',
		'env -i PATH="$PATH" HOME="$HOME" cargo run --quiet --bin backend-school -- export-openapi'
	]) {
		assert.ok(artifacts.includes(command), `artifacts must retain ${command}`);
	}
	for (const command of [
		'cargo fmt --all -- --check',
		'cargo test api_contract::tests --bin backend-school',
		'cargo test structured_logging --test static_architecture',
		'cargo check --bin backend-school'
	]) {
		assert.ok(backend.includes(command), `backend must retain ${command}`);
	}
	for (const command of [
		'node --test tests/static/api-response-contract.test.mjs',
		'npm run check'
	]) {
		assert.ok(frontend.includes(command), `frontend must retain ${command}`);
	}

	for (const nodeJob of [artifacts, frontend]) {
		assert.match(nodeJob, /uses: actions\/setup-node@v6/);
		assert.match(nodeJob, /node-version: "22"/);
		assert.match(nodeJob, /cache: npm/);
		assert.match(nodeJob, /cache-dependency-path: frontend-school\/package-lock\.json/);
		assert.match(nodeJob, /working-directory: frontend-school\n\s+run: npm ci/);
	}
	assert.doesNotMatch(backend, /uses: actions\/setup-node@v6/);

	for (const rustJob of [artifacts, backend]) {
		assert.match(rustJob, /uses: dtolnay\/rust-toolchain@stable/);
		assert.match(rustJob, /uses: Swatinem\/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32/);
		assert.match(rustJob, /id: rust_cache/);
		assert.match(rustJob, /shared-key: backend-school-contracts/);
		assert.match(rustJob, /workspaces: backend-school -> target/);
		assert.match(rustJob, /steps\.rust_cache\.outputs\.cache-hit/);
		assert.match(rustJob, />> "\$GITHUB_STEP_SUMMARY"/);
	}
	assert.match(artifacts, /save-if: \$\{\{ github\.ref == 'refs\/heads\/main' \}\}/);
	assert.match(backend, /save-if: "false"/);
	assert.doesNotMatch(frontend, /Swatinem\/rust-cache/);
	assert.equal(
		(jobs.match(/save-if: \$\{\{ github\.ref == 'refs\/heads\/main' \}\}/g) ?? []).length,
		1
	);
	assert.equal((jobs.match(/save-if: "false"/g) ?? []).length, 1);

	const rules = await readRepo('.rules');
	assert.match(
		rules,
		/API Contract runs artifact, backend, and frontend validation in independent jobs without `needs`/
	);
});

test('Permission Contract keeps its cached validation gates unchanged', async () => {
	const workflow = await readRepo('.github/workflows/permission-contract.yml');
	assert.match(workflow, /^ {2}verify:\s*$/m);
	assert.match(workflow, /uses: Swatinem\/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32/);
	assert.match(workflow, /shared-key: backend-school-contracts/);
	assert.match(workflow, /workspaces: backend-school -> target/);
	assert.match(workflow, /save-if: \$\{\{ github\.ref == 'refs\/heads\/main' \}\}/);
	assert.match(workflow, /steps\.rust_cache\.outputs\.cache-hit/);
	assert.match(workflow, /cache: npm/);
	for (const command of [
		'node scripts/generate-permissions.mjs --check',
		'node --test scripts/tests/generate-permissions.test.mjs',
		'cargo fmt --all -- --check',
		'cargo check --bin backend-school',
		'cargo test --test static_architecture',
		'npm run test:static',
		'npm run check'
	]) {
		assert.ok(workflow.includes(command), `Permission Contract must retain ${command}`);
	}
});

test('frontend deployments keep environment values out of committed Worker configuration', async () => {
	const wrangler = JSON.parse(await readRepo('frontend-admin/wrangler.json'));
	assert.equal(wrangler.account_id, undefined);
	assert.equal(wrangler.vars, undefined);

	const admin = await readRepo('.github/workflows/deploy-frontend-admin.yml');
	assert.match(admin, /secrets:\s*\|\s*\n\s*INTERNAL_API_SECRET/);
	assert.match(admin, /vars\.BACKEND_ADMIN_URL/);
	assert.match(admin, /vars\.BACKEND_SCHOOL_URL/);
	assert.match(admin, /vars\.BASE_DOMAIN/);
	assert.match(admin, /vars\.CLOUDFLARE_ACCOUNT_ID/);
	assert.match(admin, /wrangler\.deploy\.json/);
	assert.match(admin, /FRONTEND_DEPLOY_ENABLED/);
	const adminWorkerDeploy = admin.slice(admin.indexOf('- name: Deploy frontend-admin Worker'));
	assert.match(adminWorkerDeploy, /PUBLIC_API_URL: \$\{\{ vars\.BACKEND_ADMIN_URL \}\}/);
	assert.match(adminWorkerDeploy, /BACKEND_SCHOOL_URL: \$\{\{ vars\.BACKEND_SCHOOL_URL \}\}/);

	for (const file of [
		'.github/workflows/deploy-all-schools.yml',
		'.github/workflows/deploy-school-tenant.yml'
	]) {
		const workflow = await readRepo(file);
		assert.match(workflow, /vars\.BASE_DOMAIN/);
		assert.match(workflow, /vars\.BACKEND_SCHOOL_URL/);
		assert.match(workflow, /vars\.CLOUDFLARE_ACCOUNT_ID/);
		assert.match(workflow, /jq -n/);
		assert.doesNotMatch(workflow, /\.schoolorbit\.app\/\*/);
		assert.doesNotMatch(
			workflow,
			/secrets\.(?:BACKEND_SCHOOL_URL|VAPID_PUBLIC_KEY|CLOUDFLARE_ACCOUNT_ID)/
		);
	}
});

test('runtime diagnostics expose container state without environment or application logs', async () => {
	const workflow = await readRepo('.github/workflows/runtime-diagnostics.yml');

	assert.match(workflow, /workflow_dispatch/);
	assert.match(workflow, /State\.ExitCode/);
	assert.match(workflow, /State\.OOMKilled/);
	assert.match(workflow, /NetworkSettings\.Networks/);
	assert.match(workflow, /\.Aliases/);
	assert.match(workflow, /podman port schoolorbit-nginx/);
	assert.match(workflow, /podman exec schoolorbit-nginx nginx -t/);
	assert.match(workflow, /nginx:stable-alpine nginx -t/);
	assert.match(workflow, /pg_stat_activity/);
	assert.match(workflow, /'nginx_restarts=[^']*'\s*\\\s*schoolorbit-nginx\s*\|\| true/);
	assert.match(workflow, /if \[ -r \/opt\/stack\/\.env \]; then/);
	assert.match(workflow, /if \[ -n "\$database_url" \]; then/);
	assert.match(workflow, /database_activity=status_unavailable reason=database_url_missing/);
	assert.match(workflow, /podman info --format '\{\{\.Store\.GraphRoot\}\}'/);
	assert.match(workflow, /df -P "\$graph_root"/);
	assert.match(workflow, /podman system df/);
	assert.match(workflow, /podman system df -v/);
	assert.match(workflow, /runtime_sha_images repository=/);
	assert.doesNotMatch(workflow, /Config\.Env/);
	assert.doesNotMatch(workflow, /podman logs/);
	assert.doesNotMatch(workflow, /curl[^\n]*-[^\n]*k/);
});

test('installer CI enforces shell provider topology and workflow guards', async () => {
	const workflow = await readRepo('.github/workflows/installer.yml');

	assert.match(workflow, /runs-on: ubuntu-24\.04/);
	for (const path of [
		'scripts/schoolorbit-installer',
		'scripts/lib/schoolorbit-installer/**',
		'scripts/tests/installer/**',
		'podman-compose.yml',
		'nginx-configs/**',
		'.github/workflows/**',
		'.rules',
		'docs/OPERATIONS.md',
		'docs/PODMAN_SETUP.md',
		'docs/TESTING.md'
	]) {
		assert.ok(workflow.includes(path), `installer workflow must watch ${path}`);
	}
	for (const check of [
		'shellcheck scripts/schoolorbit-installer',
		'shfmt -d -i 4 -ci scripts/schoolorbit-installer',
		'bats scripts/tests/installer',
		'node --test frontend-school/tests/static/deployment-installer.test.mjs',
		'podman-compose -f podman-compose.yml --dry-run up -d',
		'rhysd/actionlint:1.7.7'
	]) {
		assert.ok(workflow.includes(check), `installer workflow must run ${check}`);
	}
});

test('Cockpit management stays loopback-only, secret-safe, and documented', async () => {
	const [
		installer,
		bootstrap,
		cockpitRemote,
		cloudflareTunnel,
		phases,
		operations,
		setup,
		testing,
		rules
	] = await Promise.all([
		readRepo('scripts/schoolorbit-installer'),
		readRepo('scripts/lib/schoolorbit-installer/remote/bootstrap.sh'),
		readRepo('scripts/lib/schoolorbit-installer/remote/configure_cockpit.sh'),
		readRepo('scripts/lib/schoolorbit-installer/cloudflare_tunnel.sh'),
		readRepo('scripts/lib/schoolorbit-installer/phases.sh'),
		readRepo('docs/OPERATIONS.md'),
		readRepo('docs/PODMAN_SETUP.md'),
		readRepo('docs/TESTING.md'),
		readRepo('.rules')
	]);

	assert.match(installer, /source "\$INSTALLER_LIBRARY\/cloudflare_tunnel\.sh"/);
	assert.match(installer, /configure-cockpit --resume RUN_ID/);
	assert.match(installer, /rollback-cockpit --run-id RUN_ID/);
	assert.match(bootstrap, /SCHOOLORBIT_CLOUDFLARED_VERSION=2026\.7\.3/);
	assert.match(bootstrap, /049777d30f9bf93da6df8bbe31383460eb2aa51a832c6551824d56f9fcc55974/);
	assert.match(bootstrap, /d3ea7d22dd337b465da33d6bc1c4b3cfd381407447a2a7d29542c19783430db3/);
	assert.doesNotMatch(bootstrap, /ufw allow (?:9090|"?\$\{?COCKPIT)/);
	assert.match(cockpitRemote, /ListenStream=127\.0\.0\.1:9090/);
	assert.match(cockpitRemote, /--token-file \/etc\/cloudflared\/schoolorbit-cockpit\.token/);
	assert.match(cockpitRemote, /disallowed_content.*root/);
	assert.match(cloudflareTunnel, /http:\/\/127\.0\.0\.1:9090/);
	assert.doesNotMatch(cloudflareTunnel, /DELETE[^\n]*cfd_tunnel/);
	assert.match(phases, /ROLLBACK COCKPIT \$SO_CF_COCKPIT_HOSTNAME/);

	for (const value of [
		'configure-cockpit',
		'rollback-cockpit',
		'server.schoolorbit.app',
		'SCHOOLORBIT_SERVER_PASSWORD',
		'schoolorbit',
		'127.0.0.1:9090'
	]) {
		assert.ok(operations.includes(value), `operations must contain ${value}`);
		assert.ok(setup.includes(value), `setup must contain ${value}`);
	}
	assert.match(operations, /public login|publicly reachable login/i);
	assert.match(setup, /Cloudflare Tunnel/);
	assert.doesNotMatch(setup, /https:\/\/<server-ip>:9090/);
	for (const batsFile of ['cockpit_provider.bats', 'cockpit_remote.bats']) {
		assert.ok(testing.includes(batsFile), `testing must contain ${batsFile}`);
	}
	assert.match(rules, /Cockpit management/);
});

test('durable operations docs describe the guarded replacement VPS path', async () => {
	const [operations, setup, adminReadme] = await Promise.all([
		readRepo('docs/OPERATIONS.md'),
		readRepo('docs/PODMAN_SETUP.md'),
		readRepo('frontend-admin/README.md')
	]);

	for (const value of [
		'RUNTIME_DEPLOY_ENABLED',
		'FRONTEND_DEPLOY_ENABLED',
		'migrate-vps --resume',
		'rollback-dns --run-id',
		'CUTOVER',
		'ROLLBACK',
		'Origin CA',
		'certificate_expiry'
	]) {
		assert.ok(operations.includes(value), `operations must contain ${value}`);
	}
	assert.match(setup, /schoolorbit-installer migrate-vps/);
	assert.match(setup, /Full \(strict\)/);
	assert.match(setup, /schoolorbit-web/);
	assert.doesNotMatch(setup, /certbot/i);
	assert.match(adminReadme, /repository variables/);
	assert.match(adminReadme, /Worker secret binding/);
	assert.match(adminReadme, /never owns production credentials or URLs/);
});
