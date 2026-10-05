import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, chmod, rm } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
const repo = path.resolve(import.meta.dirname, '../..');
async function compatibilityScript() {
    const text = await readFile(path.join(repo, '.github/workflows/backend-school-neon-compatibility.yml'), 'utf8');
    const step = text.split('      - name: Run direct-endpoint compatibility tests\n')[1]?.split('\n      - name:')[0];
    const body = step?.split('        run: |\n')[1];
    assert.ok(body, 'the compatibility step must have an executable shell body');
    return body.split('\n').map(line => line.startsWith('          ') ? line.slice(10) : line).join('\n');
}
async function runStub(mode, scope = 'full') {
    const root = await mkdtemp(path.join(tmpdir(), 'schoolorbit-neon-selection-'));
    try {
        const executable = path.join(root, 'cargo');
        await writeFile(executable, `#!/usr/bin/env node\nimport { appendFileSync } from 'node:fs';\nappendFileSync(process.env.SYNTHETIC_CARGO_ARGUMENTS,JSON.stringify(process.argv.slice(2))+'\\n');\nconst mode=process.env.SYNTHETIC_CARGO_MODE;\nconsole.log(mode==='zero'?'test result: ok. 0 passed; 0 failed; 0 ignored;':'test result: ok. 2 passed; 0 failed; 0 ignored;');\nprocess.exit(mode==='failure'?47:0);\n`);
        await chmod(executable, 0o755);
        const argumentsFile = path.join(root, 'arguments.jsonl');
        const result = spawnSync('bash', ['-c', await compatibilityScript()], {
            cwd: path.join(repo, 'backend-school'), encoding: 'utf8',
            env: { ...process.env, PATH: root + path.delimiter + process.env.PATH,
                TEST_DATABASE_URL: 'postgres://synthetic:synthetic@synthetic.invalid/synthetic',
                SYNTHETIC_CARGO_ARGUMENTS: argumentsFile, SYNTHETIC_CARGO_MODE: mode,
                NEON_COMPATIBILITY_SCOPE: scope }
        });
        assert.ifError(result.error);
        const calls = (await readFile(argumentsFile, 'utf8')).trim().split('\n').map(line => JSON.parse(line));
        return { ...result, calls };
    } finally { await rm(root, { recursive: true, force: true }); }
}
test('compatibility refuses successful Cargo commands that selected no tests', async () => {
    const result = await runStub('zero');
    assert.notEqual(result.status, 0, 'empty compatibility coverage must fail');
});
test('compatibility preserves Cargo failures and accepts actual passing test output', async () => {
    assert.equal((await runStub('failure')).status, 47);
    const passing = await runStub('positive');
    assert.equal(passing.status, 0, passing.stderr);
    assert.equal(passing.calls.length, 5);
});
test('every declared compatibility selection discovers real tests in its current Rust owner', async () => {
    const calls = [...(await runStub('positive')).calls, ...(await runStub('positive', 'timetable-subject-groups')).calls];
    const missing = [];
    for (const args of calls) {
        const separator = args.indexOf('--');
        const selection = separator < 0 ? args : args.slice(0, separator);
        const result = spawnSync('cargo', [...selection, '--', '--list'], {
            cwd: path.join(repo, 'backend-school'), encoding: 'utf8', maxBuffer: 4 * 1024 * 1024
        });
        assert.ifError(result.error);
        assert.equal(result.status, 0, result.stderr);
        if (!result.stdout.split('\n').some(line => line.endsWith(': test'))) missing.push(selection.join(' '));
    }
    assert.deepEqual(missing, [], 'each compatibility target must run at least one real test');
});

test('timetable subject group scope selects its real database regression and rejects empty results', async () => {
    const passing = await runStub('positive', 'timetable-subject-groups');
    assert.equal(passing.status, 0, passing.stderr);
    assert.equal(passing.calls.length, 1);
    assert.deepEqual(passing.calls[0], ['test', 'modules::academic::core::schema_tests::migration_060_subject_group_projections_support_timetable_load', '--bin', 'backend-school', '--', '--exact', '--nocapture']);
    assert.notEqual((await runStub('zero', 'timetable-subject-groups')).status, 0);
    assert.equal((await runStub('failure', 'timetable-subject-groups')).status, 47);
});
