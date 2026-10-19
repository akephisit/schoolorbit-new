import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {requirePreparedTlsGate, requireTlsRuntimePlan, validateDnsResumeId} from '../lib/pipeline-api-tls.mjs';

const sha = 'a'.repeat(40);
const job = name => ({name, conclusion: 'success', head_sha: sha, run_attempt: 2});
test('TLS gate accepts successful exact latest-attempt builds even when rollout is held later', () => {
  requirePreparedTlsGate([job('plan'), job('Pipeline gate'), job('prepare / Prepare backend-school')], sha, 2);
  requirePreparedTlsGate([job('plan'), job('Pipeline gate')], sha, 2);
});
test('TLS gate rejects missing, ambiguous, failed and stale preparation', () => {
  for (const jobs of [[], [job('plan')], [job('plan'), job('Pipeline gate'), job('Pipeline gate')],
    [job('plan'), {...job('Pipeline gate'), run_attempt: 1}],
    [job('plan'), job('Pipeline gate'), {...job('prepare / Prepare backend-school'), conclusion: 'failure'}]]) {
    assert.throws(() => requirePreparedTlsGate(jobs, sha, 2));
  }
});
test('renewal permits older accepted source with no queued runtime change and rejects pending runtime work', () => {
  requireTlsRuntimePlan({deploy: []}, 'renew');
  assert.throws(() => requireTlsRuntimePlan({deploy: ['backend-school']}, 'renew'));
  assert.throws(() => requireTlsRuntimePlan({deploy: ['backend-school']}, 'migrate-origin-ca'), /Unknown TLS operation/);
  assert.throws(() => requireTlsRuntimePlan({deploy: []}, 'migrate-origin-ca'), /Unknown TLS operation/);
  assert.throws(() => requireTlsRuntimePlan({deploy: []}, 'other'));
  for (const mode of ['dns-only', 'dns-rollback']) {
    requireTlsRuntimePlan({deploy: []}, mode);
    assert.throws(() => requireTlsRuntimePlan({deploy: ['frontend-school']}, mode));
  }
});

test('certificate renewal and explicit API DNS operations retain the shared production lock', () => {
  const read = relative => readFileSync(new URL(`../../${relative}`, import.meta.url), 'utf8');
  const maintenance = read('.github/workflows/maintenance.yml');
  const operations = read('.github/workflows/operations.yml');
  assert.match(maintenance, /group: schoolorbit-production\n  cancel-in-progress: false/);
  assert.match(maintenance, /cron: 47 19 \* \* \*/);
  assert.match(maintenance, /github\.event\.schedule == '23 19 \* \* 6'/);
  assert.match(operations, /api-dns:\n[\s\S]*?group: schoolorbit-production\n      cancel-in-progress: false/);
  assert.doesNotMatch(operations, /migrate-api-tls|migrate-origin-ca/);
  const wrapper = read('scripts/lib/pipeline-remote/api-tls-operation.sh');
  assert.match(wrapper, /bash "\$maintenance" enter/);
  for (const source of [wrapper, read('scripts/lib/pipeline-remote/maintenance.sh')]) {
    assert.doesNotMatch(source, /migrate-origin-ca|enter-origin-ca-migration|origin_ca_rsa_root|--cacert|--insecure/);
  }
  assert.doesNotMatch(read('scripts/lib/pipeline-remote/api-tls.sh'), /cloudflare-origin|origin_ca_rsa_root|enter-origin-ca-migration/);
});

test('existing origin DNS cutover checks public port reachability and authenticated full context from the external runner', () => {
  const workflow = readFileSync(new URL('../../.github/workflows/api-tls.yml', import.meta.url), 'utf8');
  const external = workflow.indexOf('name: Verify external direct HTTPS');
  const remote = workflow.indexOf('name: Upload shared certificate');
  assert.ok(external > 0 && external < remote);
  const step = workflow.slice(external, remote);
  assert.match(step, /if: inputs.mode == 'dns-only'/);
  assert.match(step, /inputs.resume_id == ''/);
  assert.match(step, /SMOKE_RESOLVE_IP: \$\{\{ secrets\.SERVER_IP \}\}/);
  assert.match(step, /SMOKE_CA_CERT: ""/);
  assert.match(step, /SMOKE_REQUIRE_AUTH: "true"/);
  assert.match(step, /SMOKE_ACADEMIC_CONTEXT: "true"/);
  assert.match(step, /run: bash scripts\/smoke_test\.sh/);
});

test('resume transport is validated before remote actions and retains external read-only TLS verification', () => {
  validateDnsResumeId('dns-only', '123-1');
  validateDnsResumeId('dns-only', '');
  assert.throws(() => validateDnsResumeId('dns-only', '../request'));
  assert.throws(() => validateDnsResumeId('dns-rollback', '123-1'));
  const workflow = readFileSync(new URL('../../.github/workflows/api-tls.yml', import.meta.url), 'utf8');
  const check = workflow.slice(workflow.indexOf('name: Verify external public TLS'), workflow.indexOf('name: Upload shared certificate'));
  assert.match(check, /inputs.resume_id != ''/);
  assert.match(check, /--resolve "\$host:443:\$TARGET_ORIGIN_IP"/);
  assert.doesNotMatch(check, /SMOKE_RELEASE_PROBE_TOKEN|--insecure|--cacert/);
});
