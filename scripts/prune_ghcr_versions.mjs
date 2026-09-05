#!/usr/bin/env node

import { pathToFileURL } from 'node:url';

const ALLOWED_PACKAGES = new Set([
	'schoolorbit-backend-admin',
	'schoolorbit-backend-school'
]);
const SHA_TAG = /^[0-9a-f]{40}$/;
const OWNER = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$/;
const MAX_DELETIONS = 100;
const API_VERSION = '2026-03-10';

function normalizeVersion(version) {
	if (version === null || typeof version !== 'object' || Array.isArray(version)) {
		throw new Error('GHCR inventory contains an invalid version');
	}
	if (!Number.isSafeInteger(version.id) || version.id <= 0) {
		throw new Error('GHCR inventory contains an invalid version ID');
	}
	if (typeof version.created_at !== 'string' || !Number.isFinite(Date.parse(version.created_at))) {
		throw new Error('GHCR inventory contains an invalid creation time');
	}
	if (
		version.metadata?.package_type !== 'container' ||
		!Array.isArray(version.metadata?.container?.tags) ||
		!version.metadata.container.tags.every(
			(tag) => typeof tag === 'string' && tag.length > 0 && tag.length <= 256
		)
	) {
		throw new Error('GHCR inventory contains invalid container metadata');
	}

	const tags = [...version.metadata.container.tags];
	if (new Set(tags).size !== tags.length) {
		throw new Error('GHCR inventory contains duplicate tags');
	}

	return {
		id: version.id,
		createdAt: version.created_at,
		createdAtEpoch: Date.parse(version.created_at),
		tags
	};
}

function versionSignature(version) {
	return JSON.stringify({
		id: version.id,
		createdAt: version.createdAt,
		tags: [...version.tags].sort()
	});
}

function compareNewestFirst(left, right) {
	return right.createdAtEpoch - left.createdAtEpoch || right.id - left.id;
}

function compareOldestFirst(left, right) {
	return left.createdAtEpoch - right.createdAtEpoch || left.id - right.id;
}

export function selectDeletionCandidates(versions, keep) {
	if (!Array.isArray(versions)) throw new Error('GHCR inventory must be an array');
	if (!Number.isSafeInteger(keep) || keep < 1 || keep > 100) {
		throw new Error('GHCR retention count must be from 1 through 100');
	}

	const normalized = versions.map(normalizeVersion);
	const ids = new Set();
	for (const version of normalized) {
		if (ids.has(version.id)) throw new Error('GHCR inventory contains duplicate version IDs');
		ids.add(version.id);
	}

	const releases = normalized
		.filter((version) => version.tags.some((tag) => SHA_TAG.test(tag)))
		.sort(compareNewestFirst);
	const retainedIds = new Set(releases.slice(0, keep).map(({ id }) => id));

	return releases
		.filter(
			(version) =>
				!retainedIds.has(version.id) &&
				!version.tags.includes('latest') &&
				version.tags.length > 0 &&
				version.tags.every((tag) => SHA_TAG.test(tag))
		)
		.sort(compareOldestFirst)
		.map((version) => ({
			id: version.id,
			createdAt: version.createdAt,
			safeTags: [...version.tags].sort(),
			signature: versionSignature(version)
		}));
}

function validateTarget({ owner, packageName, keep, token, apiBase }) {
	if (typeof owner !== 'string' || !OWNER.test(owner)) {
		throw new Error('Invalid GHCR owner');
	}
	if (!ALLOWED_PACKAGES.has(packageName)) {
		throw new Error('Unsupported GHCR package');
	}
	if (!Number.isSafeInteger(keep) || keep < 1 || keep > 100) {
		throw new Error('GHCR retention count must be from 1 through 100');
	}
	if (typeof token !== 'string' || token.length === 0 || /[\r\n]/.test(token)) {
		throw new Error('GITHUB_TOKEN is unavailable');
	}

	let parsedApiBase;
	try {
		parsedApiBase = new URL(apiBase);
	} catch {
		throw new Error('Invalid GitHub API base URL');
	}
	const loopback = ['127.0.0.1', 'localhost', '[::1]'].includes(parsedApiBase.hostname);
	if (
		parsedApiBase.username ||
		parsedApiBase.password ||
		(parsedApiBase.protocol !== 'https:' && !(parsedApiBase.protocol === 'http:' && loopback))
	) {
		throw new Error('Invalid GitHub API base URL');
	}
	return parsedApiBase.href.replace(/\/$/, '');
}

function validateLinkHeader(link, { page }) {
	if (link === null) return;
	const relations = new Map();
	for (const part of link.split(',')) {
		const match = part.trim().match(/^<([^>]+)>;\s*rel="(first|prev|next|last)"$/);
		if (!match || relations.has(match[2])) throw new Error('GHCR pagination is malformed');
		let target;
		try {
			target = new URL(match[1]);
		} catch {
			throw new Error('GHCR pagination is malformed');
		}
		// GitHub may canonicalize the path or add endpoint-specific query fields.
		// We never follow this URL; requests still use the validated package path above.
		if (!/^[1-9][0-9]*$/.test(target.searchParams.get('page') ?? '')) {
			throw new Error('GHCR pagination is malformed');
		}
		relations.set(match[2], Number(target.searchParams.get('page')));
	}
	if (relations.has('next') && relations.get('next') !== page + 1) {
		throw new Error('GHCR pagination is malformed');
	}
}

function parseCliArguments(arguments_) {
	const result = { execute: false };
	for (let index = 0; index < arguments_.length; index += 1) {
		const argument = arguments_[index];
		if (argument === '--execute') {
			result.execute = true;
			continue;
		}
		if (!['--owner', '--package', '--keep'].includes(argument)) {
			throw new Error('Unsupported GHCR retention argument');
		}
		const value = arguments_[index + 1];
		if (value === undefined) throw new Error('Missing GHCR retention argument value');
		index += 1;
		if (argument === '--owner') result.owner = value;
		if (argument === '--package') result.packageName = value;
		if (argument === '--keep') result.keep = Number(value);
	}
	return result;
}

export async function pruneGhcrVersions({
	owner,
	packageName,
	keep,
	execute = false,
	token,
	apiBase = 'https://api.github.com',
	logger = console.log,
	fetchImpl = globalThis.fetch
}) {
	const normalizedApiBase = validateTarget({ owner, packageName, keep, token, apiBase });
	if (typeof fetchImpl !== 'function') throw new Error('Fetch implementation is unavailable');
	if (typeof logger !== 'function') throw new Error('Logger is unavailable');

	const packagePath = `/users/${encodeURIComponent(owner)}/packages/container/${encodeURIComponent(packageName)}/versions`;
	const headers = {
		accept: 'application/vnd.github+json',
		authorization: `Bearer ${token}`,
		'x-github-api-version': API_VERSION,
		'user-agent': 'schoolorbit-ghcr-retention'
	};

	async function request(path, { method = 'GET', expectedStatus = 200 } = {}) {
		let response;
		try {
			response = await fetchImpl(`${normalizedApiBase}${path}`, { method, headers });
		} catch {
			throw new Error('GitHub API request failed');
		}
		if (response.status !== expectedStatus) {
			throw new Error(`GitHub API request failed with status ${response.status}`);
		}
		if (expectedStatus === 204) return { data: null, link: response.headers.get('link') };
		let data;
		try {
			data = await response.json();
		} catch {
			throw new Error('GitHub API returned invalid JSON');
		}
		return { data, link: response.headers.get('link') };
	}

	const inventory = [];
	for (let page = 1; page <= 1000; page += 1) {
		const path = `${packagePath}?per_page=100&page=${page}`;
		const { data, link } = await request(path);
		if (!Array.isArray(data) || data.length > 100) {
			throw new Error('GitHub API returned an invalid package inventory');
		}
		validateLinkHeader(link, { page });
		inventory.push(...data);
		if (data.length < 100) break;
		if (page === 1000) throw new Error('GHCR pagination exceeded its safety limit');
	}

	const normalizedInventory = inventory.map(normalizeVersion);
	const releaseCount = normalizedInventory.filter((version) =>
		version.tags.some((tag) => SHA_TAG.test(tag))
	).length;
	const allCandidates = selectDeletionCandidates(inventory, keep);
	const candidates = allCandidates.slice(0, MAX_DELETIONS);

	for (const candidate of candidates) {
		logger(
			`ghcr_retention_candidate mode=${execute ? 'execute' : 'dry-run'} package=${packageName} version_id=${candidate.id} created_at=${candidate.createdAt} tags=${candidate.safeTags.join(',')}`
		);
	}

	if (!execute) {
		logger(
			`ghcr_retention package=${packageName} releases=${releaseCount} retained=${Math.min(releaseCount, keep)} candidates=${allCandidates.length} selected=${candidates.length} deleted=0 mode=dry-run`
		);
		return { candidates: allCandidates.length, selected: candidates.length, deleted: 0 };
	}

	async function assertCandidateUnchanged(candidate) {
		const { data } = await request(`${packagePath}/${candidate.id}`);
		const current = normalizeVersion(data);
		if (
			current.tags.includes('latest') ||
			current.tags.length === 0 ||
			!current.tags.every((tag) => SHA_TAG.test(tag)) ||
			versionSignature(current) !== candidate.signature
		) {
			throw new Error(`GHCR version ${candidate.id} changed during revalidation`);
		}
	}

	for (const candidate of candidates) await assertCandidateUnchanged(candidate);

	let deleted = 0;
	for (const candidate of candidates) {
		await assertCandidateUnchanged(candidate);
		await request(`${packagePath}/${candidate.id}`, {
			method: 'DELETE',
			expectedStatus: 204
		});
		deleted += 1;
		logger(
			`ghcr_retention_deleted package=${packageName} version_id=${candidate.id} created_at=${candidate.createdAt} tags=${candidate.safeTags.join(',')}`
		);
	}

	logger(
		`ghcr_retention package=${packageName} releases=${releaseCount} retained=${Math.min(releaseCount, keep)} candidates=${allCandidates.length} selected=${candidates.length} deleted=${deleted} mode=execute`
	);
	return { candidates: allCandidates.length, selected: candidates.length, deleted };
}

async function main() {
	const arguments_ = parseCliArguments(process.argv.slice(2));
	await pruneGhcrVersions({
		...arguments_,
		token: process.env.GITHUB_TOKEN,
		apiBase: process.env.GITHUB_API_URL || 'https://api.github.com'
	});
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	main().catch((error) => {
		console.error(`GHCR retention failed: ${error.message}`);
		process.exitCode = 1;
	});
}
