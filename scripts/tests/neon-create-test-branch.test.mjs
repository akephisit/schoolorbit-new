import assert from 'node:assert/strict';
import test from 'node:test';

import { createNeonTestBranch } from '../neon-create-test-branch.mjs';

const validEnv = {
    GITHUB_OUTPUT: '/tmp/schoolorbit-github-output',
    NEON_TEST_API_KEY: 'neon-test-secret-value',
    NEON_TEST_PROJECT_ID: 'quiet-test-12345678',
    NEON_TEST_PARENT_BRANCH_ID: 'br-parent-abc123',
    NEON_TEST_DATABASE: 'neondb',
    NEON_TEST_ROLE: 'neondb_owner',
    NEON_BRANCH_NAME: 'schoolorbit-test-123-1',
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

function outputCollector() {
    const values = [];
    return {
        appendOutput: async (name, value) => values.push([name, value]),
        values
    };
}

test('creates a branch without a suspension override and publishes a masked direct URL', async () => {
    const calls = [];
    const outputs = outputCollector();
    const stdout = captureWriter();
    const stderr = captureWriter();
    const directUrl =
        'postgresql://neondb_owner:generated-password@ep-direct.us-east-2.aws.neon.tech/neondb?sslmode=require';
    const fetchImpl = async (url, options) => {
        calls.push({ url, options });
        return options.method === 'POST'
            ? response(201, { branch: { id: 'br-created-abc123' } })
            : response(200, { uri: directUrl });
    };

    const exitCode = await createNeonTestBranch({
        env: validEnv,
        fetchImpl,
        stderr: stderr.stream,
        stdout: stdout.stream,
        appendOutput: outputs.appendOutput
    });

    assert.equal(exitCode, 0);
    assert.equal(stderr.output(), '');
    assert.equal(calls.length, 2);
    assert.equal(calls[0].options.method, 'POST');
    const createBody = JSON.parse(calls[0].options.body);
    assert.deepEqual(createBody, {
        branch: {
            name: validEnv.NEON_BRANCH_NAME,
            parent_id: validEnv.NEON_TEST_PARENT_BRANCH_ID,
            expires_at: validEnv.NEON_BRANCH_EXPIRES_AT
        },
        endpoints: [{ type: 'read_write' }]
    });
    assert.equal(JSON.stringify(createBody).includes('suspend_timeout'), false);

    assert.equal(calls[1].options.method, 'GET');
    const connectionRequest = new URL(calls[1].url);
    assert.equal(connectionRequest.pathname, '/api/v2/projects/quiet-test-12345678/connection_uri');
    assert.deepEqual(Object.fromEntries(connectionRequest.searchParams), {
        branch_id: 'br-created-abc123',
        database_name: 'neondb',
        role_name: 'neondb_owner',
        pooled: 'false'
    });
    assert.deepEqual(outputs.values, [
        ['created', 'true'],
        ['branch_id', 'br-created-abc123'],
        ['db_url', directUrl]
    ]);
    assert.equal(stdout.output(), `::add-mask::${directUrl}\n`);
});

test('reports only a bounded sanitized create rejection', async () => {
    const outputs = outputCollector();
    const stderr = captureWriter();
    const leakedUrl = 'postgresql://owner:password@db.example/neondb';
    const fetchImpl = async () =>
        response(412, {
            code: 'PRECONDITION_FAILED',
            message: `rejected ${leakedUrl} ${validEnv.NEON_TEST_API_KEY}\n${'x'.repeat(600)}`
        });

    const exitCode = await createNeonTestBranch({
        env: validEnv,
        fetchImpl,
        stderr: stderr.stream,
        stdout: captureWriter().stream,
        appendOutput: outputs.appendOutput
    });

    assert.equal(exitCode, 1);
    assert.deepEqual(outputs.values, []);
    assert.match(stderr.output(), /status=412/);
    assert.match(stderr.output(), /code=PRECONDITION_FAILED/);
    assert.match(stderr.output(), /\[redacted-url\]/);
    assert.doesNotMatch(stderr.output(), /postgresql:\/\/|password|neon-test-secret-value/);
    assert.ok(stderr.output().length <= 500);
});

test('publishes ownership before a sanitized connection URI failure', async () => {
    const calls = [];
    const outputs = outputCollector();
    const stderr = captureWriter();
    const fetchImpl = async (url, options) => {
        calls.push({ url, options });
        if (options.method === 'POST') {
            return response(201, { branch: { id: 'br-owned-before-uri' } });
        }
        return response(404, {
            code: 'NOT_FOUND',
            message: 'database missing at https://console.neon.tech/internal'
        });
    };

    const exitCode = await createNeonTestBranch({
        env: validEnv,
        fetchImpl,
        stderr: stderr.stream,
        stdout: captureWriter().stream,
        appendOutput: outputs.appendOutput
    });

    assert.equal(exitCode, 1);
    assert.deepEqual(outputs.values, [
        ['created', 'true'],
        ['branch_id', 'br-owned-before-uri']
    ]);
    assert.match(stderr.output(), /connection URI rejected: status=404 code=NOT_FOUND/);
    assert.match(stderr.output(), /\[redacted-url\]/);
    assert.doesNotMatch(stderr.output(), /https:\/\//);
});

test('rejects malformed configuration before contacting Neon', async () => {
    let called = false;
    const outputs = outputCollector();
    const stderr = captureWriter();
    const exitCode = await createNeonTestBranch({
        env: { ...validEnv, NEON_TEST_PARENT_BRANCH_ID: 'development' },
        fetchImpl: async () => {
            called = true;
            return response(500);
        },
        stderr: stderr.stream,
        stdout: captureWriter().stream,
        appendOutput: outputs.appendOutput
    });

    assert.equal(exitCode, 1);
    assert.equal(called, false);
    assert.deepEqual(outputs.values, []);
    assert.match(stderr.output(), /invalid NEON_TEST_PARENT_BRANCH_ID/);
    assert.doesNotMatch(stderr.output(), /neon-test-secret-value/);
});
