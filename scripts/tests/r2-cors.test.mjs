import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const root = resolve(import.meta.dirname, '../..');
const workflow = await readFile(join(root, '.github/workflows/deploy-backend-school.yml'), 'utf8');
const start = workflow.indexOf('            private_cors_origin=');
const end = workflow.indexOf('            unset AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY', start);
assert.ok(start >= 0 && end > start);
const script = workflow.slice(start, end).replace(/^ {12}/gm, '');
const desired = { CORSRules: [{
    AllowedOrigins: ['https://*.example.test'], AllowedMethods: ['GET', 'HEAD'],
    AllowedHeaders: ['Range'],
    ExposeHeaders: ['Accept-Ranges', 'Content-Length', 'Content-Range', 'Content-Type', 'ETag'],
    MaxAgeSeconds: 3600,
}] };

async function run(t, scenario, policy = desired) {
    const directory = await mkdtemp(join(tmpdir(), 'schoolorbit-cors-'));
    t.after(() => rm(directory, { recursive: true, force: true }));
    await writeFile(join(directory, 'policy.json'), JSON.stringify(policy));
    const boundary = join(directory, 'aws.mjs');
    await writeFile(boundary, `
        import { readFileSync, writeFileSync, appendFileSync, existsSync } from 'node:fs';
        import { join } from 'node:path';
        const dir = process.env.CORS_TEST_DIR;
        const scenario = process.env.CORS_TEST_SCENARIO;
        const args = process.argv.slice(2);
        const action = args[1];
        appendFileSync(join(dir, 'calls'), action + '\\n');
        const written = existsSync(join(dir, 'written'));
        if (args[0] !== 's3api' || args[args.indexOf('--bucket') + 1] !== 'private-test') process.exit(90);
        if (action === 'get-bucket-cors') {
            if (scenario === 'read-error' || (scenario === 'verify-error' && written)) {
                process.stderr.write('AccessDenied test-only-sensitive-marker'); process.exit(1);
            }
            if (scenario === 'missing' && !written) {
                process.stderr.write('An error occurred (NoSuchCORSConfiguration) when calling the GetBucketCors operation'); process.exit(1);
            }
            if (scenario === 'invalid-json') { process.stdout.write('{broken'); process.exit(0); }
            const policy = JSON.parse(readFileSync(join(dir, 'policy.json')));
            const query = args[args.indexOf('--query') + 1];
            if (args.includes('--query')) {
                process.stdout.write((query.endsWith('AllowedOrigins') ? policy.CORSRules[0].AllowedOrigins : policy.CORSRules[0].AllowedMethods).join('\\t'));
            } else process.stdout.write(JSON.stringify(policy));
        } else if (action === 'put-bucket-cors') {
            if (scenario === 'write-error') { process.stderr.write('test-only-sensitive-marker'); process.exit(1); }
            writeFileSync(join(dir, 'written'), 'yes');
            if (scenario !== 'mismatch') writeFileSync(join(dir, 'policy.json'), args[args.indexOf('--cors-configuration') + 1]);
        } else process.exit(91);
    `);
    const result = spawnSync('bash', ['-c', `
        set -euo pipefail
        base_domain=example.test
        private_bucket=private-test
        r2_cors_helper_source="$CORS_TEST_ROOT/scripts/reconcile_r2_cors.sh"
        r2_cli() { "$CORS_TEST_NODE" "$CORS_TEST_BOUNDARY" "$@"; }
        ${script}
    `], { encoding: 'utf8', env: { ...process.env,
        CORS_TEST_ROOT: root, CORS_TEST_DIR: directory, CORS_TEST_SCENARIO: scenario,
        CORS_TEST_NODE: process.execPath, CORS_TEST_BOUNDARY: boundary,
    } });
    assert.ifError(result.error);
    const calls = (await readFile(join(directory, 'calls'), 'utf8')).trim().split('\n');
    assert.doesNotMatch(result.stdout + result.stderr, /test-only-sensitive-marker/);
    return { ...result, calls, policy: JSON.parse(await readFile(join(directory, 'policy.json'), 'utf8')) };
}

test('matching policy is read once without rewriting', async (t) => {
    const result = await run(t, 'match');
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(result.calls, ['get-bucket-cors']);
});
test('array ordering does not cause a CORS write', async (t) => {
    const policy = structuredClone(desired);
    policy.CORSRules[0].AllowedMethods.reverse();
    policy.CORSRules[0].ExposeHeaders.reverse();
    const result = await run(t, 'match', policy);
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(result.calls, ['get-bucket-cors']);
});
for (const scenario of ['drift', 'missing']) {
    test(`${scenario} is written and read back for verification`, async (t) => {
        const result = await run(t, scenario, { CORSRules: [] });
        assert.equal(result.status, 0, result.stderr);
        assert.deepEqual(result.calls, ['get-bucket-cors', 'put-bucket-cors', 'get-bucket-cors']);
        assert.deepEqual(result.policy, desired);
    });
}
for (const field of ['AllowedOrigins', 'AllowedHeaders', 'ExposeHeaders', 'MaxAgeSeconds']) {
    test(`drift in ${field} is reconciled, not just methods and origin`, async (t) => {
        const policy = structuredClone(desired);
        policy.CORSRules[0][field] = field === 'MaxAgeSeconds' ? 1 : ['unexpected'];
        const result = await run(t, 'drift', policy);
        assert.equal(result.status, 0, result.stderr);
        assert.deepEqual(result.calls, ['get-bucket-cors', 'put-bucket-cors', 'get-bucket-cors']);
        assert.deepEqual(result.policy, desired);
    });
}
for (const scenario of ['read-error', 'invalid-json', 'invalid-shape']) {
    test(`${scenario} fails closed without writing`, async (t) => {
        const result = await run(t, scenario, { unexpected: true });
        assert.notEqual(result.status, 0);
        assert.deepEqual(result.calls, ['get-bucket-cors']);
    });
}
for (const scenario of ['write-error', 'verify-error', 'mismatch']) {
    test(`${scenario} cannot declare reconciliation successful`, async (t) => {
        const result = await run(t, scenario, { CORSRules: [] });
        assert.notEqual(result.status, 0);
        assert.deepEqual(result.calls, scenario === 'write-error'
            ? ['get-bucket-cors', 'put-bucket-cors']
            : ['get-bucket-cors', 'put-bucket-cors', 'get-bucket-cors']);
    });
}
