import { pathToFileURL } from 'node:url';

const API_ROOT = 'https://console.neon.tech/api/v2';
const SUSPEND_TIMEOUT_SECONDS = 300;
const MAX_DIAGNOSTIC_LENGTH = 300;

function scalar(value, fallback) {
    return ['string', 'number', 'boolean'].includes(typeof value) ? String(value) : fallback;
}

export function sanitizeDiagnosticValue(value, secrets = []) {
    let sanitized = scalar(value, 'unspecified');
    for (const secret of secrets) {
        if (secret) sanitized = sanitized.replaceAll(secret, '[redacted]');
    }
    sanitized = sanitized
        .replace(/\b(?:postgres(?:ql)?|https?):\/\/[^\s"'<>]+/giu, '[redacted-url]')
        .replace(/[\u0000-\u001f\u007f]+/gu, ' ')
        .replace(/\s+/gu, ' ')
        .trim();
    return (sanitized || 'unspecified').slice(0, MAX_DIAGNOSTIC_LENGTH);
}

function parseErrorDetails(body) {
    try {
        const parsed = JSON.parse(body);
        const nested = parsed?.error && typeof parsed.error === 'object' ? parsed.error : {};
        return {
            code: scalar(parsed?.code ?? nested.code, 'unknown'),
            message: scalar(parsed?.message ?? nested.message, 'response omitted')
        };
    } catch {
        return { code: 'unknown', message: 'non-JSON response omitted' };
    }
}

function validateEnvironment(env) {
    const required = [
        'NEON_TEST_API_KEY',
        'NEON_TEST_PROJECT_ID',
        'NEON_TEST_PARENT_BRANCH_ID',
        'NEON_DIAGNOSTIC_BRANCH_NAME',
        'NEON_BRANCH_EXPIRES_AT'
    ];
    for (const name of required) {
        if (!env[name]) throw new Error(`missing ${name}`);
    }
    if (!/^[a-z0-9-]{1,60}$/u.test(env.NEON_TEST_PROJECT_ID)) {
        throw new Error('invalid NEON_TEST_PROJECT_ID');
    }
    if (!/^br-[a-z0-9-]+$/u.test(env.NEON_TEST_PARENT_BRANCH_ID)) {
        throw new Error('invalid NEON_TEST_PARENT_BRANCH_ID');
    }
    if (!/^[a-z0-9-]{1,63}$/u.test(env.NEON_DIAGNOSTIC_BRANCH_NAME)) {
        throw new Error('invalid NEON_DIAGNOSTIC_BRANCH_NAME');
    }
    if (
        !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u.test(env.NEON_BRANCH_EXPIRES_AT) ||
        Number.isNaN(Date.parse(env.NEON_BRANCH_EXPIRES_AT))
    ) {
        throw new Error('invalid NEON_BRANCH_EXPIRES_AT');
    }
}

export async function diagnoseNeonBranchCreate({
    env = process.env,
    fetchImpl = globalThis.fetch,
    stderr = process.stderr
} = {}) {
    try {
        validateEnvironment(env);
    } catch (error) {
        stderr.write(`ERROR: Neon diagnostic configuration: ${error.message}\n`);
        return 1;
    }

    const secrets = [env.NEON_TEST_API_KEY];
    const projectUrl = `${API_ROOT}/projects/${env.NEON_TEST_PROJECT_ID}`;
    const headers = {
        accept: 'application/json',
        authorization: `Bearer ${env.NEON_TEST_API_KEY}`,
        'content-type': 'application/json'
    };
    const requestBody = {
        branch: {
            name: env.NEON_DIAGNOSTIC_BRANCH_NAME,
            parent_id: env.NEON_TEST_PARENT_BRANCH_ID,
            expires_at: env.NEON_BRANCH_EXPIRES_AT
        },
        endpoints: [{ type: 'read_write', suspend_timeout_seconds: SUSPEND_TIMEOUT_SECONDS }]
    };
    let createdBranchId;

    try {
        const response = await fetchImpl(`${projectUrl}/branches`, {
            method: 'POST',
            headers,
            body: JSON.stringify(requestBody)
        });
        const responseBody = await response.text();
        if (!response.ok) {
            const details = parseErrorDetails(responseBody);
            const code = sanitizeDiagnosticValue(details.code, secrets);
            const message = sanitizeDiagnosticValue(details.message, secrets);
            stderr.write(
                `ERROR: Neon diagnostic create rejected: status=${response.status} code=${code} message=${message}\n`
            );
            return 1;
        }

        let parsed;
        try {
            parsed = JSON.parse(responseBody);
        } catch {
            stderr.write(
                `ERROR: Neon diagnostic create returned invalid JSON: status=${response.status}\n`
            );
            return 1;
        }
        if (!/^br-[a-z0-9-]+$/u.test(parsed?.branch?.id ?? '')) {
            stderr.write('ERROR: Neon diagnostic create returned an invalid branch ID\n');
            return 1;
        }
        createdBranchId = parsed.branch.id;
        stderr.write(
            'ERROR: Neon diagnostic create unexpectedly succeeded; investigating the pinned action path\n'
        );
    } catch (error) {
        const message = sanitizeDiagnosticValue(error?.message, secrets);
        stderr.write(
            `ERROR: Neon diagnostic request failed: status=network_error code=unknown message=${message}\n`
        );
    } finally {
        if (createdBranchId) {
            try {
                const cleanup = await fetchImpl(`${projectUrl}/branches/${createdBranchId}`, {
                    method: 'DELETE',
                    headers: {
                        accept: 'application/json',
                        authorization: headers.authorization
                    }
                });
                if (![200, 204].includes(cleanup.status)) {
                    stderr.write(
                        `ERROR: Neon diagnostic branch cleanup failed: status=${cleanup.status}\n`
                    );
                }
            } catch (error) {
                const message = sanitizeDiagnosticValue(error?.message, secrets);
                stderr.write(
                    `ERROR: Neon diagnostic branch cleanup request failed: message=${message}\n`
                );
            }
        }
    }

    return 1;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
    process.exitCode = await diagnoseNeonBranchCreate();
}
