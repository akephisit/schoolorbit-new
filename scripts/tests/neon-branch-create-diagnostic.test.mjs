import assert from 'node:assert/strict';
import test from 'node:test';

import { diagnoseNeonBranchCreate } from '../neon-branch-create-diagnostic.mjs';

const validEnv = {
    NEON_TEST_API_KEY: 'neon-test-secret-value',
    NEON_TEST_PROJECT_ID: 'quiet-test-12345678',
    NEON_TEST_PARENT_BRANCH_ID: 'br-parent-abc123',
    NEON_DIAGNOSTIC_BRANCH_NAME: 'schoolorbit-diagnostic-123-1',
    NEON_BRANCH_EXPIRES_AT: '2026-08-11T15:15:45Z'
};

function captureWriter() {
    let output = '';
    return {
        stream: {
            write(chunk) {
                output += String(chunk);
                return true;
            }
        },
        output: () => output
    };
}

function response(status, body = '') {
    return {
        ok: status >= 200 && status < 300,
        status,
        text: async () => (typeof body === 'string' ? body : JSON.stringify(body))
    };
}

test('reports only a bounded sanitized Neon rejection', async () => {
    const calls = [];
    const stderr = captureWriter();
    const leakedUrl = 'postgresql://owner:password@db.example/neondb';
    const longSuffix = 'x'.repeat(600);
    const fetchImpl = async (url, options) => {
        calls.push({ url, options });
        return response(412, {
            code: 'PRECONDITION_FAILED',
            message: `unsupported ${leakedUrl} ${validEnv.NEON_TEST_API_KEY}\n${longSuffix}`
        });
    };

    const exitCode = await diagnoseNeonBranchCreate({
        env: validEnv,
        fetchImpl,
        stderr: stderr.stream
    });

    assert.equal(exitCode, 1);
    assert.equal(calls.length, 1);
    assert.equal(
        calls[0].url,
        'https://console.neon.tech/api/v2/projects/quiet-test-12345678/branches'
    );
    assert.equal(calls[0].options.method, 'POST');
    assert.equal(
        calls[0].options.headers.authorization,
        `Bearer ${validEnv.NEON_TEST_API_KEY}`
    );
    assert.deepEqual(JSON.parse(calls[0].options.body), {
        branch: {
            name: validEnv.NEON_DIAGNOSTIC_BRANCH_NAME,
            parent_id: validEnv.NEON_TEST_PARENT_BRANCH_ID,
            expires_at: validEnv.NEON_BRANCH_EXPIRES_AT
        },
        endpoints: [{ type: 'read_write', suspend_timeout_seconds: 300 }]
    });

    const output = stderr.output();
    assert.match(output, /status=412/);
    assert.match(output, /code=PRECONDITION_FAILED/);
    assert.match(output, /\[redacted-url\]/);
    assert.doesNotMatch(output, /postgresql:\/\//);
    assert.doesNotMatch(output, /password|neon-test-secret-value/);
    assert.ok(output.length <= 500);
});

test('deletes the exact diagnostic branch when create unexpectedly succeeds', async () => {
    const calls = [];
    const stderr = captureWriter();
    const fetchImpl = async (url, options) => {
        calls.push({ url, options });
        if (options.method === 'POST') {
            return response(201, {
                branch: { id: 'br-diagnostic-created' },
                uri: 'postgresql://owner:password@db.example/neondb'
            });
        }
        return response(204);
    };

    const exitCode = await diagnoseNeonBranchCreate({
        env: validEnv,
        fetchImpl,
        stderr: stderr.stream
    });

    assert.equal(exitCode, 1);
    assert.equal(calls.length, 2);
    assert.equal(calls[1].options.method, 'DELETE');
    assert.equal(
        calls[1].url,
        'https://console.neon.tech/api/v2/projects/quiet-test-12345678/branches/br-diagnostic-created'
    );
    assert.match(stderr.output(), /diagnostic create unexpectedly succeeded/);
    assert.doesNotMatch(stderr.output(), /postgresql:\/\/|password|neon-test-secret-value/);
});

test('rejects malformed configuration before contacting Neon', async () => {
    let called = false;
    const stderr = captureWriter();
    const exitCode = await diagnoseNeonBranchCreate({
        env: { ...validEnv, NEON_TEST_PARENT_BRANCH_ID: 'development' },
        fetchImpl: async () => {
            called = true;
            return response(500);
        },
        stderr: stderr.stream
    });

    assert.equal(exitCode, 1);
    assert.equal(called, false);
    assert.match(stderr.output(), /invalid NEON_TEST_PARENT_BRANCH_ID/);
    assert.doesNotMatch(stderr.output(), /neon-test-secret-value/);
});
