import { execFile } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(import.meta.dirname, '../../../..');

export async function renderProxy(template, baseDomain = 'example.test') {
	const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-proxy-test-'));
	const output = path.join(temporary, 'rendered.conf');

	try {
		await execFileAsync(path.join(repoRoot, 'scripts/render_nginx_config.sh'), [
			path.join(repoRoot, template),
			output,
			baseDomain
		]);
		return await readFile(output, 'utf8');
	} finally {
		await rm(temporary, { recursive: true, force: true });
	}
}
