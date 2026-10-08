import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { parse, stringify } from 'yaml';

// Source guards follow the deployed implementation rather than a stale inline body.
export async function readWorkflowSource(root, file) {
	const source = await readFile(path.join(root, file), 'utf8');
	if (!file.startsWith('.github/workflows/')) return source;
	const workflow = parse(source);
	for (const job of Object.values(workflow.jobs || {})) {
		for (const step of job.steps || []) {
			if (!step.with?.script?.startsWith('bash /opt/stack/deployment/scripts/')) continue;
			const file = step.with.script.replace('bash /opt/stack/deployment/', '').split(' ')[0];
			let body = await readFile(path.join(root, file), 'utf8');
			for (const [name, value] of Object.entries(step.env || {})) {
				if (typeof value === 'string' && value.startsWith('${{'))
					body = body.replaceAll('${' + name + '}', value);
			}
			step.with.script = body.replace(/^#!.*\n/, '');
		}
	}
	return stringify(workflow, { lineWidth: 0 });
}
