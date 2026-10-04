import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { spawn } from 'node:child_process';

const workflow = readFileSync('.github/workflows/deploy-school-release.yml', 'utf8');
const body = workflow.match(/migration_completion_filter="([\s\S]*?)"\n\s*if ! podman/)?.[1];
assert.ok(body, 'exercise the migration filter actually used by the release');
const filter = body.replaceAll('\\"', '"').replaceAll('\\$', '$');

async function accepts(report) {
    return new Promise((resolve, reject) => {
        const child = spawn('podman', ['run', '--rm', '-i', 'ghcr.io/jqlang/jq:1.7.1', '-e', filter], {
            stdio: ['pipe', 'ignore', 'pipe']
        });
        let error = '';
        child.stderr.on('data', chunk => error += chunk);
        child.on('error', reject);
        child.on('close', code => {
            if (code === 0 || code === 1 || code === 4) resolve(code === 0);
            else reject(new Error(`Migration filter failed (${code}): ${error}`));
        });
        child.stdin.end(JSON.stringify(report));
    });
}

function validReport(version = 88) {
    const school = {
        migration_version: version, migration_status: 'migrated', migration_error: null,
        academicCoreCutover: { migrationVersion: 45, status: 'cleanupCompleted', passed: true, checks: [{ passed: true }] },
        gradebookResultsCutover: { migrationVersion: 60, status: 'cutoverCompleted', passed: true, checks: [{ passed: true }] },
        deliveryTimetableCutover: { migrationVersion: 88, status: 'cutoverCompleted', passed: true, checks: Array.from({ length: 30 }, () => ({ passed: true })) }
    };
    return {
        latest_version: version, total_schools: 2, migrated: 2, pending: 0, failed: 0, outdated: 0,
        schools: [structuredClone(school), structuredClone(school)]
    };
}

test('fully migrated tenants pass without a retired personnel report', async () => {
    assert.equal(await accepts(validReport()), true);
    assert.equal(await accepts(validReport(89)), true, 'future SQL migrations use the same completion gate');
});

test('migration completion refuses incomplete, failed or inconsistent tenant coverage', async () => {
    const changes = [
        r => { r.schools.pop(); },
        r => { r.total_schools = 0; r.migrated = 0; r.schools = []; },
        ...['pending', 'failed', 'outdated'].map(key => r => { r[key] = 1; }),
        r => { r.migrated = 1; },
        r => { r.schools[1].migration_version = 87; },
        r => { r.schools[1].migration_version = 89; },
        r => { r.schools[1].migration_status = 'failed'; },
        r => { r.schools[1].migration_error = 'synthetic_failure'; }
    ];
    for (const change of changes) {
        const report = validReport();
        change(report);
        assert.equal(await accepts(report), false);
    }
});

test('retiring personnel hooks preserves the other domain audit gates', async () => {
    for (const key of ['academicCoreCutover', 'gradebookResultsCutover', 'deliveryTimetableCutover']) {
        for (const change of [
            a => { a.passed = false; },
            a => { a.status = 'pending'; },
            a => { a.checks[0].passed = false; },
            a => { a.migrationVersion = 0; }
        ]) {
            const report = validReport();
            change(report.schools[1][key]);
            assert.equal(await accepts(report), false);
        }
    }
});

test('delivery cutover requires both complete reconciliation audits before release', async () => {
    for (const change of [
        r => { delete r.schools[1].deliveryTimetableCutover; },
        r => { r.schools[1].deliveryTimetableCutover = null; },
        r => { r.schools[1].deliveryTimetableCutover.migrationVersion = 87; },
        r => { r.schools[1].deliveryTimetableCutover.checks = []; },
        r => { r.schools[1].deliveryTimetableCutover.checks.pop(); },
        r => { r.schools[1].deliveryTimetableCutover.checks.push({ passed: true }); },
        r => { delete r.schools[1].deliveryTimetableCutover.checks[0].passed; }
    ]) {
        const report = validReport();
        change(report);
        assert.equal(await accepts(report), false);
    }
});
