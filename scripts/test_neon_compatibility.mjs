#!/usr/bin/env node
import { randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { createNeonTestBranch, sanitizeNeonDiagnostic } from './neon-create-test-branch.mjs';

const root = path.resolve(import.meta.dirname, '..');
const scopes = ['full', 'timetable-subject-groups', 'course-zero-periods'];

// Local-only owner: the connection URL never goes into a command line, file or log.
export async function runNeonCompatibility({ env = process.env, create = createNeonTestBranch,
    fetchImpl = fetch, run, stderr = process.stderr } = {}) {
    const scope = env.NEON_COMPATIBILITY_SCOPE || 'full';
    if (!scopes.includes(scope)) throw new Error('Unknown Neon compatibility selection');
    const outputs = {};
    let interrupted = 0;
    let child;
    const stop = (signal) => {
        interrupted = signal === 'SIGINT' ? 130 : 143;
        if (child?.pid) {
            try { process.kill(-child.pid, signal); }
            catch (error) { if (error.code !== 'ESRCH') throw error; }
        }
    };
    const onInt = () => stop('SIGINT'), onTerm = () => stop('SIGTERM');
    process.on('SIGINT', onInt); process.on('SIGTERM', onTerm);
    const execute = run || ((command, args, childEnv) => new Promise((resolve, reject) => {
        child = spawn(command, args, { cwd: path.join(root, 'backend-school'), env: childEnv, stdio: 'inherit', detached: true });
        child.once('error', reject);
        child.once('exit', (code, signal) => { child = undefined; resolve(interrupted || (signal ? 1 : code)); });
    }));
    let status = 1;
    try {
        status = await create({ env: { ...env, GITHUB_OUTPUT: 'local-memory',
            NEON_BRANCH_NAME: `schoolorbit-test-${randomUUID()}`,
            NEON_BRANCH_EXPIRES_AT: new Date(Date.now() + 2 * 60 * 60 * 1000).toISOString().replace(/\.\d{3}Z$/, 'Z') },
            appendOutput: async (key, value) => { outputs[key] = value; },
            stdout: { write() {} }, stderr, fetchImpl });
        if (status !== 0 || interrupted) return interrupted || status;
        if (outputs.created !== 'true' || !/^br-[a-z0-9-]+$/.test(outputs.branch_id || '') || !outputs.db_url)
            throw new Error('Fresh disposable branch ownership is missing');
        const childEnv = { ...env, TEST_DATABASE_URL: outputs.db_url, PGDATABASE: outputs.db_url,
            NEON_COMPATIBILITY_SCOPE: scope };
        delete childEnv.NEON_TEST_API_KEY;
        status = await execute('psql', ['--no-psqlrc', '--set=ON_ERROR_STOP=1', '--command',
            'CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public; CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;'], childEnv);
        if (status !== 0 || interrupted) return interrupted || status;
        return await execute('bash', [path.join(root, 'scripts/test_neon_compatibility.sh')], childEnv);
    } finally {
        try {
            if (outputs.created === 'true' && /^br-[a-z0-9-]+$/.test(outputs.branch_id || '')) {
                const response = await fetchImpl(`https://console.neon.tech/api/v2/projects/${env.NEON_TEST_PROJECT_ID}/branches/${outputs.branch_id}`, {
                    method: 'DELETE', headers: { authorization: `Bearer ${env.NEON_TEST_API_KEY}` },
                    signal: AbortSignal.timeout(15000)
                });
                if (![200, 204].includes(response.status)) throw new Error(`Neon cleanup failed (HTTP ${response.status}); branch expires in two hours`);
            }
        } finally { process.off('SIGINT', onInt); process.off('SIGTERM', onTerm); }
    }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
    try { process.exitCode = await runNeonCompatibility(); }
    catch (error) { console.error(sanitizeNeonDiagnostic(error.message, [process.env.NEON_TEST_API_KEY])); process.exitCode = 1; }
}
