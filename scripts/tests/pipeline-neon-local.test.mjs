import assert from 'node:assert/strict';
import test from 'node:test';
import { runNeonCompatibility } from '../test_neon_compatibility.mjs';

function fixture({ creationFailure = false, testFailure = false, cleanupFailure = false } = {}) {
    const commands = [], deletions = [];
    const url = 'postgresql://test:private@ep-local.neon.tech/empty?sslmode=require';
    return { commands, deletions, options: {
        env: { NEON_TEST_PROJECT_ID: 'empty-test', NEON_TEST_API_KEY: 'private-key' },
        create: async ({ appendOutput, stdout, env }) => {
            assert.match(env.NEON_BRANCH_NAME, /^schoolorbit-test-/);
            assert.ok(Date.parse(env.NEON_BRANCH_EXPIRES_AT) > Date.now());
            stdout.write(`::add-mask::${url}\n`);
            await appendOutput('created', 'true'); await appendOutput('branch_id', 'br-owned');
            if (creationFailure) return 1;
            await appendOutput('db_url', url); return 0;
        },
        run: async (command, args, env) => {
            assert.ok(args.every(arg => !arg.includes(url)), 'URL must stay out of process arguments');
            assert.equal(env.TEST_DATABASE_URL, url); commands.push(command);
            assert.equal(env.NEON_TEST_API_KEY, undefined, 'tests must not inherit the branch-management key');
            return command === 'bash' && testFailure ? 47 : 0;
        },
        fetchImpl: async (endpoint, options) => {
            assert.ok(endpoint.endsWith('/projects/empty-test/branches/br-owned'));
            assert.equal(options.method, 'DELETE'); deletions.push(endpoint);
            return { status: cleanupFailure ? 500 : 204 };
        }
    } };
}

for (const scenario of [{}, { creationFailure: true }, { testFailure: true }])
    test(`local Neon deletes only its owned branch after ${JSON.stringify(scenario)}`, async () => {
        const f = fixture(scenario);
        assert.equal(await runNeonCompatibility(f.options), scenario.creationFailure ? 1 : scenario.testFailure ? 47 : 0);
        assert.deepEqual(f.commands, scenario.creationFailure ? [] : ['psql', 'bash']);
        assert.equal(f.deletions.length, 1);
    });

test('failed local Neon cleanup cannot report a passing run', async () => {
    const f = fixture({ cleanupFailure: true });
    await assert.rejects(runNeonCompatibility(f.options), /cleanup failed/);
});

test('an unknown local Neon selection fails before creating any branch', async () => {
    const f = fixture(); f.options.env.NEON_COMPATIBILITY_SCOPE = 'unknown';
    await assert.rejects(runNeonCompatibility(f.options), /Unknown/);
    assert.deepEqual(f.commands, []); assert.deepEqual(f.deletions, []);
});

test('interrupted local Neon creation still cleans up the owned branch before returning', async () => {
    const f = fixture(); const create = f.options.create;
    f.options.create = async (options) => {
        const status = await create(options);
        process.emit('SIGTERM');
        return status;
    };
    assert.equal(await runNeonCompatibility(f.options), 143);
    assert.deepEqual(f.commands, []); assert.equal(f.deletions.length, 1);
});
