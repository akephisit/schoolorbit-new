import { setTimeout as delay } from 'node:timers/promises';
import { pathToFileURL } from 'node:url';

const DEFAULT_ASSET_MAX_ATTEMPTS = 12;
const DEFAULT_ASSET_RETRY_DELAY_MS = 5_000;
const DEFAULT_REQUEST_TIMEOUT_MS = 15_000;
const DEFAULT_MOUNT_MAX_ATTEMPTS = 3;
const DEFAULT_MOUNT_RETRY_DELAY_MS = 5_000;

const boundedReason = (error) => {
	if (error instanceof Error && /^[a-z0-9_]+$/.test(error.message)) return error.message;
	return 'unexpected_error';
};

const positiveInteger = (value, name) => {
	const parsed = Number.parseInt(String(value), 10);
	if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`invalid_${name}`);
	return parsed;
};

const request = async (url, timeoutMs, fetchImplementation) => {
	const response = await fetchImplementation(url, {
		redirect: 'follow',
		signal: AbortSignal.timeout(timeoutMs)
	});
	if (!response.ok) throw new Error(`http_${response.status}`);
	return response;
};

const immutableAssetUrls = (html, documentUrl, expectedOrigin) => {
	const urls = new Set();
	const attributes = html.matchAll(/(?:src|href)=["']([^"']+)["']/gi);
	for (const match of attributes) {
		const asset = new URL(match[1], documentUrl);
		if (!asset.pathname.startsWith('/_app/immutable/')) continue;
		if (!/\.(?:js|css)$/.test(asset.pathname)) continue;
		if (asset.origin !== expectedOrigin) throw new Error('asset_origin_mismatch');
		urls.add(asset.href);
	}
	if (urls.size === 0) throw new Error('immutable_assets_missing');
	return [...urls];
};

const verifyAssets = async ({ origin, requestTimeoutMs, fetchImplementation }) => {
	const rootUrl = new URL('/', origin);
	const response = await request(rootUrl, requestTimeoutMs, fetchImplementation);
	const documentUrl = new URL(response.url || rootUrl);
	if (documentUrl.origin !== rootUrl.origin) throw new Error('document_origin_mismatch');
	const assets = immutableAssetUrls(await response.text(), documentUrl, rootUrl.origin);
	for (const assetUrl of assets) {
		const assetResponse = await request(assetUrl, requestTimeoutMs, fetchImplementation);
		if (new URL(assetResponse.url || assetUrl).origin !== rootUrl.origin) {
			throw new Error('asset_redirect_origin_mismatch');
		}
		try {
			await assetResponse.arrayBuffer();
		} catch {
			throw new Error('asset_body_incomplete');
		}
	}
};

const verifyApplicationMount = async (origin) => {
	const { chromium } = await import('playwright');
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage();
		let pageErrorCount = 0;
		page.on('pageerror', () => {
			pageErrorCount += 1;
		});
		const response = await page.goto(origin, {
			waitUntil: 'domcontentloaded',
			timeout: 60_000
		});
		if (!response?.ok()) throw new Error('mount_document_failed');
		await page.waitForFunction(
			() => document.documentElement.dataset.schoolorbitAppMounted === 'true',
			undefined,
			{ timeout: 30_000 }
		);
		if (pageErrorCount > 0) throw new Error('mount_page_error');
	} catch (error) {
		if (error instanceof Error && /^[a-z0-9_]+$/.test(error.message)) throw error;
		throw new Error('mount_failed');
	} finally {
		await browser.close();
	}
};

const waitForCondition = async ({ phase, maxAttempts, retryDelayMs, check, onAttemptFailure }) => {
	for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
		try {
			await check();
			return attempt;
		} catch (error) {
			const reason = boundedReason(error);
			onAttemptFailure({ phase, attempt, maxAttempts, reason });
			if (attempt === maxAttempts) {
				throw new Error(`${phase}_readiness_failed_after_${maxAttempts}_attempts_${reason}`);
			}
			await delay(retryDelayMs);
		}
	}
	throw new Error(`${phase}_readiness_failed`);
};

export const verifyTenantWorkerReadiness = async ({
	origin,
	assetMaxAttempts = DEFAULT_ASSET_MAX_ATTEMPTS,
	assetRetryDelayMs = DEFAULT_ASSET_RETRY_DELAY_MS,
	requestTimeoutMs = DEFAULT_REQUEST_TIMEOUT_MS,
	mountMaxAttempts = DEFAULT_MOUNT_MAX_ATTEMPTS,
	mountRetryDelayMs = DEFAULT_MOUNT_RETRY_DELAY_MS,
	fetchImplementation = globalThis.fetch,
	mountCheck = () => verifyApplicationMount(origin),
	onAttemptFailure = ({ phase, attempt, maxAttempts, reason }) => {
		console.error(`Tenant ${phase} readiness attempt ${attempt}/${maxAttempts} failed: ${reason}`);
	}
}) => {
	const tenantUrl = new URL(origin);
	const options = {
		assetMaxAttempts: positiveInteger(assetMaxAttempts, 'asset_max_attempts'),
		assetRetryDelayMs: positiveInteger(assetRetryDelayMs, 'asset_retry_delay_ms'),
		requestTimeoutMs: positiveInteger(requestTimeoutMs, 'request_timeout_ms'),
		mountMaxAttempts: positiveInteger(mountMaxAttempts, 'mount_max_attempts'),
		mountRetryDelayMs: positiveInteger(mountRetryDelayMs, 'mount_retry_delay_ms')
	};

	const assetAttempts = await waitForCondition({
		phase: 'asset',
		maxAttempts: options.assetMaxAttempts,
		retryDelayMs: options.assetRetryDelayMs,
		check: () =>
			verifyAssets({
				origin: tenantUrl,
				requestTimeoutMs: options.requestTimeoutMs,
				fetchImplementation
			}),
		onAttemptFailure
	});
	const mountAttempts = await waitForCondition({
		phase: 'mount',
		maxAttempts: options.mountMaxAttempts,
		retryDelayMs: options.mountRetryDelayMs,
		check: mountCheck,
		onAttemptFailure
	});

	return { assetAttempts, mountAttempts };
};

const isDirectInvocation =
	process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;

if (isDirectInvocation) {
	try {
		const result = await verifyTenantWorkerReadiness({
			origin: process.env.TENANT_ORIGIN,
			assetMaxAttempts: process.env.TENANT_ASSET_MAX_ATTEMPTS ?? DEFAULT_ASSET_MAX_ATTEMPTS,
			assetRetryDelayMs: process.env.TENANT_ASSET_RETRY_DELAY_MS ?? DEFAULT_ASSET_RETRY_DELAY_MS,
			requestTimeoutMs: process.env.TENANT_REQUEST_TIMEOUT_MS ?? DEFAULT_REQUEST_TIMEOUT_MS,
			mountMaxAttempts: process.env.TENANT_MOUNT_MAX_ATTEMPTS ?? DEFAULT_MOUNT_MAX_ATTEMPTS,
			mountRetryDelayMs: process.env.TENANT_MOUNT_RETRY_DELAY_MS ?? DEFAULT_MOUNT_RETRY_DELAY_MS
		});
		console.log(
			`Tenant Worker is ready after ${result.assetAttempts} asset attempt(s) and ${result.mountAttempts} mount attempt(s)`
		);
	} catch (error) {
		console.error(boundedReason(error));
		process.exitCode = 1;
	}
}
