import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');

function extractLocationBlocks(source) {
	const blocks = [];
	const locationPattern = /^\s*location\s+(?:(=|\^~|~\*?)\s+)?([^\s{]+)\s*\{/gm;

	for (const match of source.matchAll(locationPattern)) {
		let depth = 1;
		let cursor = match.index + match[0].length;
		while (cursor < source.length && depth > 0) {
			if (source[cursor] === '{') depth += 1;
			if (source[cursor] === '}') depth -= 1;
			cursor += 1;
		}
		assert.equal(depth, 0, `unterminated nginx location ${match[2]}`);
		blocks.push({
			modifier: match[1] ?? '',
			pattern: match[2],
			body: source.slice(match.index + match[0].length, cursor - 1)
		});
	}

	return blocks;
}

function resolveLocation(blocks, requestPath) {
	const exact = blocks.find((block) => block.modifier === '=' && block.pattern === requestPath);
	if (exact) return exact;

	const prefixes = blocks
		.filter(
			(block) =>
				block.modifier !== '~' && block.modifier !== '~*' && requestPath.startsWith(block.pattern)
		)
		.sort((left, right) => right.pattern.length - left.pattern.length);
	const longestPrefix = prefixes[0];
	if (longestPrefix?.modifier === '^~') return longestPrefix;

	const regex = blocks.find(
		(block) =>
			(block.modifier === '~' || block.modifier === '~*') &&
			new RegExp(block.pattern, block.modifier === '~*' ? 'i' : '').test(requestPath)
	);
	return regex ?? longestPrefix;
}

test('notification stream resolves to the unbuffered credentialed SSE proxy', async () => {
	const source = await readFile(
		path.join(repoRoot, 'nginx-configs/school-api.schoolorbit.app.conf'),
		'utf8'
	);
	const location = resolveLocation(extractLocationBlocks(source), '/api/notifications/stream');

	assert.ok(location, 'notification stream must resolve to an nginx location');
	assert.equal(location.modifier, '=');
	assert.equal(location.pattern, '/api/notifications/stream');
	assert.match(location.body, /proxy_pass\s+http:\/\/schoolorbit-backend-school:8081;/);
	assert.match(location.body, /proxy_buffering\s+off;/);
	assert.match(location.body, /proxy_cache\s+off;/);
	assert.match(location.body, /proxy_http_version\s+1\.1;/);
	assert.match(location.body, /proxy_set_header\s+Connection\s+"";/);
	assert.match(location.body, /chunked_transfer_encoding\s+on;/);
	assert.match(location.body, /proxy_read_timeout\s+24h;/);
	assert.match(location.body, /proxy_send_timeout\s+24h;/);
	assert.match(
		location.body,
		/add_header\s+'Access-Control-Allow-Origin'\s+\$allow_origin\s+always;/
	);
	assert.match(location.body, /add_header\s+'Access-Control-Allow-Credentials'\s+'true'\s+always;/);
});
