import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { parse, stringify } from 'yaml';

export async function expandCompositeSteps(root, steps, ancestors = new Set()) {
	const expanded = [];
	for (const step of steps) {
		if (!step.uses?.startsWith('./.github/actions/')) {
			expanded.push(step);
			continue;
		}
		const directory = path.resolve(root, step.uses);
		if (!directory.startsWith(path.join(root, '.github/actions') + path.sep))
			throw new Error('Local action must stay in the repository action directory');
		if (ancestors.has(directory)) throw new Error('Recursive composite action');
		const action = parse(await readFile(path.join(directory, 'action.yml'), 'utf8'));
		if (action.runs?.using === 'composite') {
			expanded.push(
				...(await expandCompositeSteps(root, action.runs.steps, new Set([...ancestors, directory])))
			);
		} else expanded.push(step);
	}
	return expanded;
}

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
