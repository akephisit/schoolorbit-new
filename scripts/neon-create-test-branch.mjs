import { appendFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

const API_ROOT = 'https://console.neon.tech/api/v2';
const MAX_DIAGNOSTIC_LENGTH = 300;

function scalar(value, fallback) {
    return ['string', 'number', 'boolean'].includes(typeof value) ? String(value) : fallback;
}

export function sanitizeNeonDiagnostic(value, secrets = []) {
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
        'GITHUB_OUTPUT',
        'NEON_TEST_API_KEY',
        'NEON_TEST_PROJECT_ID',
        'NEON_TEST_PARENT_BRANCH_ID',
        'NEON_TEST_DATABASE',
        'NEON_TEST_ROLE',
        'NEON_BRANCH_NAME',
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
    for (const name of ['NEON_TEST_DATABASE', 'NEON_TEST_ROLE']) {
        if (!/^[A-Za-z_][A-Za-z0-9_$-]{0,62}$/u.test(env[name])) {
            throw new Error(`invalid ${name}`);
        }
    }
    if (!/^[a-z0-9-]{1,63}$/u.test(env.NEON_BRANCH_NAME)) {
        throw new Error('invalid NEON_BRANCH_NAME');
    }
    if (
        !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u.test(env.NEON_BRANCH_EXPIRES_AT) ||
        Number.isNaN(Date.parse(env.NEON_BRANCH_EXPIRES_AT))
    ) {
        throw new Error('invalid NEON_BRANCH_EXPIRES_AT');
    }
}

function parseDirectDatabaseUrl(responseBody) {
    let uri;
    try {
        uri = JSON.parse(responseBody)?.uri;
    } catch {
        throw new Error('connection URI response is not valid JSON');
    }
    if (typeof uri !== 'string' || /[\r\n]/u.test(uri)) {
        throw new Error('connection URI response is missing a scalar URI');
    }
    let parsed;
    try {
        parsed = new URL(uri);
    } catch {
        throw new Error('connection URI response is not a valid URL');
    }
    if (
        !['postgres:', 'postgresql:'].includes(parsed.protocol) ||
        !parsed.hostname ||
        !parsed.username ||
        !parsed.password ||
        /-pooler(?:\.|$)/u.test(parsed.hostname)
    ) {
        throw new Error('connection URI response is not a direct authenticated PostgreSQL URL');
    }
    return uri;
}

function workflowCommandValue(value) {
    return value.replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A');
}

export async function createNeonTestBranch(options = {}) {
    const env = options.env ?? process.env;
    const fetchImpl = options.fetchImpl ?? globalThis.fetch;
    const stderr = options.stderr ?? process.stderr;
    const stdout = options.stdout ?? process.stdout;
    const appendOutput =
        options.appendOutput ??
        (async (name, value) => {
            await appendFile(env.GITHUB_OUTPUT, `${name}=${value}\n`, 'utf8');
        });

    try {
        validateEnvironment(env);
    } catch (error) {
        stderr.write(`ERROR: Neon branch configuration: ${error.message}\n`);
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
            name: env.NEON_BRANCH_NAME,
            parent_id: env.NEON_TEST_PARENT_BRANCH_ID,
            expires_at: env.NEON_BRANCH_EXPIRES_AT
        },
        endpoints: [{ type: 'read_write' }]
    };

    let createResponse;
    try {
        createResponse = await fetchImpl(`${projectUrl}/branches`, {
            method: 'POST',
            headers,
            body: JSON.stringify(requestBody)
        });
    } catch (error) {
        const message = sanitizeNeonDiagnostic(error?.message, secrets);
        stderr.write(
            `ERROR: Neon branch create request failed: status=network_error code=unknown message=${message}\n`
        );
        return 1;
    }

    const createBody = await createResponse.text();
    if (!createResponse.ok) {
        const details = parseErrorDetails(createBody);
        const code = sanitizeNeonDiagnostic(details.code, secrets);
        const message = sanitizeNeonDiagnostic(details.message, secrets);
        stderr.write(
            `ERROR: Neon branch create rejected: status=${createResponse.status} code=${code} message=${message}\n`
        );
        return 1;
    }

    let branchId;
    try {
        branchId = JSON.parse(createBody)?.branch?.id;
    } catch {
        stderr.write(
            `ERROR: Neon branch create returned invalid JSON: status=${createResponse.status}\n`
        );
        return 1;
    }
    if (!/^br-[a-z0-9-]+$/u.test(branchId ?? '')) {
        stderr.write('ERROR: Neon branch create returned an invalid branch ID\n');
        return 1;
    }

    try {
        await appendOutput('created', 'true');
        await appendOutput('branch_id', branchId);
    } catch {
        stderr.write('ERROR: could not publish Neon branch ownership outputs\n');
        return 1;
    }

    const connectionUrl = new URL(`${projectUrl}/connection_uri`);
    connectionUrl.search = new URLSearchParams({
        branch_id: branchId,
        database_name: env.NEON_TEST_DATABASE,
        role_name: env.NEON_TEST_ROLE,
        pooled: 'false'
    }).toString();

    let connectionResponse;
    try {
        connectionResponse = await fetchImpl(connectionUrl.toString(), {
            method: 'GET',
            headers: {
                accept: 'application/json',
                authorization: headers.authorization
            }
        });
    } catch (error) {
        const message = sanitizeNeonDiagnostic(error?.message, secrets);
        stderr.write(
            `ERROR: Neon connection URI request failed: status=network_error code=unknown message=${message}\n`
        );
        return 1;
    }

    const connectionBody = await connectionResponse.text();
    if (!connectionResponse.ok) {
        const details = parseErrorDetails(connectionBody);
        const code = sanitizeNeonDiagnostic(details.code, secrets);
        const message = sanitizeNeonDiagnostic(details.message, secrets);
        stderr.write(
            `ERROR: Neon connection URI rejected: status=${connectionResponse.status} code=${code} message=${message}\n`
        );
        return 1;
    }

    let databaseUrl;
    try {
        databaseUrl = parseDirectDatabaseUrl(connectionBody);
    } catch (error) {
        stderr.write(`ERROR: ${error.message}\n`);
        return 1;
    }

    stdout.write(`::add-mask::${workflowCommandValue(databaseUrl)}\n`);
    try {
        await appendOutput('db_url', databaseUrl);
    } catch {
        stderr.write('ERROR: could not publish the masked Neon database URL\n');
        return 1;
    }
    return 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
    process.exitCode = await createNeonTestBranch();
}
