import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { promisify } from 'node:util';
import { fileURLToPath, pathToFileURL } from 'node:url';

const execFileAsync = promisify(execFile);
const testDirectory = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(testDirectory, '../../..');
const skillPath = '.agents/skills/schoolorbit-workflow/SKILL.md';
const metadataPath = '.agents/skills/schoolorbit-workflow/agents/openai.yaml';
const fixturePath =
	'frontend-school/tests/static/fixtures/schoolorbit-agent-workflow-scenarios.json';
const agentConfigPath = '.codex/config.toml';
const documentationWorkflowPath = '.github/workflows/documentation.yml';
const workGraphValidator = path.join(
	repoRoot,
	'.agents/skills/schoolorbit-workflow/scripts/validate-work-graph.mjs'
);

const expectedProfiles = {
	schoolorbit_planner: ['gpt-5.6-sol', 'max', 'read-only'],
	schoolorbit_explorer: ['gpt-5.6-terra', 'xhigh', 'read-only'],
	schoolorbit_implementer: ['gpt-5.6-sol', 'xhigh', 'workspace-write'],
	schoolorbit_high_risk_implementer: ['gpt-5.6-sol', 'max', 'workspace-write'],
	schoolorbit_reviewer: ['gpt-5.6-sol', 'max', 'read-only'],
	schoolorbit_verifier: ['gpt-5.6-terra', 'high', 'workspace-write']
};

const registeredProfiles = {
	schoolorbit_planner: {
		configFile: 'agents/schoolorbit-planner.toml',
		description: 'Read-only lead planner for impact analysis and approval-ready SchoolOrbit plans.'
	},
	schoolorbit_explorer: {
		configFile: 'agents/schoolorbit-explorer.toml',
		description: 'Read-only explorer for one bounded SchoolOrbit code or documentation domain.'
	},
	schoolorbit_implementer: {
		configFile: 'agents/schoolorbit-implementer.toml',
		description: 'Implementation worker for one approved normal-risk task in an isolated worktree.'
	},
	schoolorbit_high_risk_implementer: {
		configFile: 'agents/schoolorbit-high-risk-implementer.toml',
		description: 'Implementation worker for an approved high-risk SchoolOrbit task.'
	},
	schoolorbit_reviewer: {
		configFile: 'agents/schoolorbit-reviewer.toml',
		description: 'Read-only independent reviewer for approved SchoolOrbit requirements and diffs.'
	},
	schoolorbit_verifier: {
		configFile: 'agents/schoolorbit-verifier.toml',
		description:
			'Verification worker that runs approved SchoolOrbit checks without changing source.'
	}
};

const roleInstructionMarkers = {
	schoolorbit_planner: ['planner role', 'Do not edit repository files'],
	schoolorbit_explorer: ['assigned repository domain', 'Do not propose fixes, edit files'],
	schoolorbit_implementer: ['approved normal-risk brief', 'Do not edit protected or unowned paths'],
	schoolorbit_high_risk_implementer: [
		'approved high-risk brief',
		'Never edit an applied migration or generated artifact directly'
	],
	schoolorbit_reviewer: ['approved requirements', 'Do not edit files'],
	schoolorbit_verifier: [
		'named focused and repository verification commands',
		'Never modify source, tests, configuration, snapshots, or expectations'
	]
};

const REQUIRED_HEADINGS = [
	'# SchoolOrbit Workflow',
	'## Core Contract',
	'## Readiness Gate',
	'## Workflow State Machine',
	'## Classify the Request',
	'## Discover',
	'## Plan Contract',
	'## Approval Gate',
	'## Plan Artifacts',
	'## Model Routing',
	'## Validate the Work Graph',
	'## Execute Approved Work',
	'## Integrate and Review',
	'## Verify and Finish',
	'## Status Contract',
	'## Quick Reference',
	'## Example',
	'## Common Rationalizations',
	'## Red Flags'
];

const REQUIRED_SUB_SKILLS = [
	'superpowers:brainstorming',
	'superpowers:writing-plans',
	'superpowers:dispatching-parallel-agents',
	'superpowers:using-git-worktrees',
	'superpowers:test-driven-development',
	'superpowers:systematic-debugging',
	'superpowers:requesting-code-review',
	'superpowers:verification-before-completion',
	'superpowers:finishing-a-development-branch'
];

async function repositoryFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

function workTask(id, overrides = {}) {
	return {
		id,
		wave: 1,
		dependencies: [],
		ownedPaths: [`frontend-school/tests/static/${id}.test.mjs`],
		protectedResources: [],
		risk: 'normal',
		agentProfile: 'schoolorbit_implementer',
		verification: ['node --test focused'],
		...overrides
	};
}

function workGraph(...tasks) {
	return { version: 1, tasks };
}

async function runWorkGraphSource(source) {
	const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-work-graph-'));
	const graphPath = path.join(temporary, 'graph.json');
	await writeFile(graphPath, source);
	try {
		return await execFileAsync(process.execPath, [workGraphValidator, graphPath]);
	} finally {
		await rm(temporary, { recursive: true, force: true });
	}
}

async function runWorkGraph(graph) {
	return runWorkGraphSource(JSON.stringify(graph));
}

async function assertCommandFailure(command, expectedFragment) {
	await assert.rejects(command, (error) => {
		assert.equal(error.code, 1);
		assert.ok(
			error.stderr.includes(expectedFragment),
			`expected stderr to contain ${expectedFragment}, received: ${error.stderr}`
		);
		assert.doesNotMatch(error.stderr, /\n\s+at /);
		return true;
	});
}

function frontmatter(source) {
	const match = /^---\n([\s\S]*?)\n---(?:\n|$)/.exec(source);
	assert.ok(match, 'expected YAML frontmatter');
	return Object.fromEntries(
		match[1].split('\n').map((line) => {
			const separator = line.indexOf(':');
			assert.notEqual(separator, -1, `invalid frontmatter line: ${line}`);
			return [line.slice(0, separator).trim(), line.slice(separator + 1).trim()];
		})
	);
}

function assertMarkersInOrder(source, markers) {
	let previous = -1;
	for (const marker of markers) {
		const position = source.indexOf(marker);
		assert.ok(position > previous, `expected ${marker} after the previous marker`);
		previous = position;
	}
}

function normalizeWhitespace(source) {
	return source.replace(/\s+/g, ' ').trim();
}

function escapeRegExp(value) {
	return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function tomlString(source, key) {
	const match = new RegExp(`^${escapeRegExp(key)}\\s*=\\s*"([^"]*)"\\s*$`, 'm').exec(source);
	assert.ok(match, `missing TOML string: ${key}`);
	return match[1];
}

function tomlInteger(source, key) {
	const match = new RegExp(`^${escapeRegExp(key)}\\s*=\\s*(\\d+)\\s*$`, 'm').exec(source);
	assert.ok(match, `missing TOML integer: ${key}`);
	return Number(match[1]);
}

function tomlBoolean(source, key) {
	const match = new RegExp(`^${escapeRegExp(key)}\\s*=\\s*(true|false)\\s*$`, 'm').exec(source);
	assert.ok(match, `missing TOML boolean: ${key}`);
	return match[1] === 'true';
}

function tomlSection(source, name) {
	const heading = new RegExp(`^\\[${escapeRegExp(name)}\\]\\s*$`, 'm');
	const match = heading.exec(source);
	assert.ok(match, `missing TOML section: ${name}`);
	const rest = source.slice(match.index + match[0].length);
	const next = rest.search(/^\[[^\n]+\]\s*$/m);
	return next === -1 ? rest : rest.slice(0, next);
}

function tomlMultilineString(source, key) {
	const match = new RegExp(`^${escapeRegExp(key)}\\s*=\\s*"""\\n([\\s\\S]*?)\\n"""\\s*$`, 'm').exec(
		source
	);
	assert.ok(match, `missing TOML multiline string: ${key}`);
	return match[1];
}

function section(source, heading, nextHeading) {
	const start = source.indexOf(heading);
	assert.notEqual(start, -1, `missing section: ${heading}`);
	const end = nextHeading ? source.indexOf(nextHeading, start + heading.length) : source.length;
	assert.notEqual(end, -1, `missing section boundary: ${nextHeading}`);
	return source.slice(start, end);
}

test('workflow evaluation fixtures remain complete and approval-aware', async () => {
	const fixture = JSON.parse(await repositoryFile(fixturePath));

	assert.equal(fixture.version, 1);
	assert.equal(fixture.scenarios.length, 6);
	assert.equal(new Set(fixture.scenarios.map(({ id }) => id)).size, 6);

	for (const scenario of fixture.scenarios) {
		assert.ok(scenario.id.length > 0, 'scenario id must not be empty');
		assert.ok(scenario.turns.length > 0, `${scenario.id} must contain turns`);
		assert.ok(
			scenario.turns.every((turn) => typeof turn === 'string' && turn.trim().length > 0),
			`${scenario.id} turns must be non-empty strings`
		);
		assert.ok(
			scenario.requiredObservations.length >= 2,
			`${scenario.id} must contain at least two observations`
		);
	}

	const implementationScenarios = fixture.scenarios.filter(({ turns }) => turns.length > 1);
	for (const scenario of implementationScenarios) {
		assert.match(
			scenario.turns.slice(1).join('\n'),
			/\bI explicitly approve the (?:exact )?current(?: [^.\n]+)? plan\b/i,
			`${scenario.id} must include explicit approval after its planning turn`
		);
	}
});

test('work graph validator accepts three disjoint tasks in one wave', async () => {
	const graph = workGraph(
		workTask('backend-task', {
			ownedPaths: ['backend-school/src/modules/attendance/services/summary.rs']
		}),
		workTask('frontend-task', {
			ownedPaths: ['frontend-school/src/lib/features/attendance/summary.ts']
		}),
		workTask('test-task', {
			ownedPaths: ['frontend-school/tests/static/attendance-summary.test.mjs']
		})
	);

	const { stdout, stderr } = await runWorkGraph(graph);

	assert.equal(stdout, 'Valid work graph: 3 tasks across 1 waves\n');
	assert.equal(stderr, '');
});

test('work graph module exports pure normalization and validation helpers', async () => {
	const validator = await import(pathToFileURL(workGraphValidator).href);

	assert.equal(
		validator.normalizeOwnedPath('frontend-school/src/lib/api.ts'),
		'frontend-school/src/lib/api.ts'
	);
	assert.equal(
		validator.pathsOverlap('frontend-school/src', 'frontend-school/src/lib/api.ts'),
		true
	);
	assert.deepEqual(
		validator
			.classifyOwnedPath('backend-school/src/api_contract.rs')
			.map(({ resource }) => resource),
		['api-contract']
	);
	assert.deepEqual(
		validator.validateWorkGraph(
			workGraph(
				workTask('api-contract-task', {
					ownedPaths: ['backend-school/src/api_contract.rs'],
					protectedResources: ['api-contract'],
					risk: 'high',
					agentProfile: 'schoolorbit_high_risk_implementer'
				})
			)
		),
		{ valid: true, errors: [], taskCount: 1, waveCount: 1 }
	);
});

test('work graph validator rejects unsafe ownership and routing', async (context) => {
	const invalidCases = [
		{
			name: 'duplicate task id',
			graph: workGraph(
				workTask('duplicate-task', { ownedPaths: ['backend-school/src/a.rs'] }),
				workTask('duplicate-task', { ownedPaths: ['frontend-school/src/a.ts'] })
			),
			expected: 'duplicate task id'
		},
		{
			name: 'more than three tasks in one wave',
			graph: workGraph(
				workTask('task-one'),
				workTask('task-two'),
				workTask('task-three'),
				workTask('task-four')
			),
			expected: 'exceeds the concurrency limit of 3'
		},
		{
			name: 'missing dependency id',
			graph: workGraph(workTask('consumer', { wave: 2, dependencies: ['missing-task'] })),
			expected: 'unknown dependency'
		},
		{
			name: 'dependency in the same wave',
			graph: workGraph(workTask('producer'), workTask('consumer', { dependencies: ['producer'] })),
			expected: 'must run in an earlier wave'
		},
		{
			name: 'equal owned paths',
			graph: workGraph(
				workTask('task-one', { ownedPaths: ['backend-school/src/shared.rs'] }),
				workTask('task-two', { ownedPaths: ['backend-school/src/shared.rs'] })
			),
			expected: 'overlapping owned paths'
		},
		{
			name: 'directory and file prefix overlap',
			graph: workGraph(
				workTask('task-one', { ownedPaths: ['backend-school/src/modules/attendance'] }),
				workTask('task-two', {
					ownedPaths: ['backend-school/src/modules/attendance/services/summary.rs']
				})
			),
			expected: 'overlapping owned paths'
		},
		{
			name: 'duplicate protected resource in one wave',
			graph: workGraph(
				workTask('task-one', { protectedResources: ['manual-owner'] }),
				workTask('task-two', { protectedResources: ['manual-owner'] })
			),
			expected: 'shared protected resource'
		},
		{
			name: 'declared high risk on normal implementer',
			graph: workGraph(workTask('high-risk-task', { risk: 'high' })),
			expected: 'requires schoolorbit_high_risk_implementer'
		},
		{
			name: 'auth path falsely declared normal',
			graph: workGraph(
				workTask('auth-task', {
					ownedPaths: ['backend-school/src/modules/auth/config.rs'],
					protectedResources: ['security-identity']
				})
			),
			expected: 'high-risk owned path'
		},
		{
			name: 'permission registry omits its owner',
			graph: workGraph(
				workTask('permission-task', {
					ownedPaths: ['backend-school/src/permissions/registry.rs'],
					risk: 'high',
					agentProfile: 'schoolorbit_high_risk_implementer'
				})
			),
			expected: 'requires protected resource permission-contract'
		},
		{
			name: 'lockfile omits its owner',
			graph: workGraph(
				workTask('lockfile-task', { ownedPaths: ['frontend-school/package-lock.json'] })
			),
			expected: 'requires protected resource dependency-lockfile'
		},
		{
			name: 'deployment workflow omits its owner',
			graph: workGraph(
				workTask('deployment-task', {
					ownedPaths: ['.github/workflows/documentation.yml'],
					risk: 'high',
					agentProfile: 'schoolorbit_high_risk_implementer'
				})
			),
			expected: 'requires protected resource deployment-owner'
		},
		{
			name: 'Rust API contract owner omits its owner',
			graph: workGraph(
				workTask('api-task', {
					ownedPaths: ['backend-school/src/api_contract.rs'],
					risk: 'high',
					agentProfile: 'schoolorbit_high_risk_implementer'
				})
			),
			expected: 'requires protected resource api-contract'
		},
		{
			name: 'legacy migration ownership',
			graph: workGraph(
				workTask('legacy-task', {
					ownedPaths: ['backend-school/migrations_legacy/001-old.sql'],
					protectedResources: ['migration-timeline'],
					risk: 'high',
					agentProfile: 'schoolorbit_high_risk_implementer'
				})
			),
			expected: 'must not own a legacy migration path'
		},
		{
			name: 'empty owned paths',
			graph: workGraph(workTask('empty-path-task', { ownedPaths: [] })),
			expected: 'requires at least one owned path'
		},
		{
			name: 'empty verification list',
			graph: workGraph(workTask('empty-check-task', { verification: [] })),
			expected: 'requires at least one verification command'
		},
		{
			name: 'absolute path',
			graph: workGraph(workTask('absolute-task', { ownedPaths: ['/tmp/owned.ts'] })),
			expected: 'must be repository-relative'
		},
		{
			name: 'parent traversal',
			graph: workGraph(
				workTask('traversal-task', { ownedPaths: ['backend-school/../secrets.txt'] })
			),
			expected: 'must not contain parent traversal'
		},
		{
			name: 'root ownership',
			graph: workGraph(workTask('root-task', { ownedPaths: ['.'] })),
			expected: 'must not own the repository root'
		},
		{
			name: 'wildcard ownership',
			graph: workGraph(workTask('wildcard-task', { ownedPaths: ['frontend-school/src/*.ts'] })),
			expected: 'must not contain glob syntax'
		},
		{
			name: 'backslash path',
			graph: workGraph(
				workTask('backslash-task', { ownedPaths: ['frontend-school\\src\\index.ts'] })
			),
			expected: 'must use POSIX separators'
		}
	];

	for (const invalidCase of invalidCases) {
		await context.test(invalidCase.name, async () => {
			await assertCommandFailure(runWorkGraph(invalidCase.graph), invalidCase.expected);
		});
	}
});

test('work graph CLI reports usage, file, and JSON errors without a stack trace', async () => {
	await assertCommandFailure(
		execFileAsync(process.execPath, [workGraphValidator]),
		'usage: node validate-work-graph.mjs <graph.json>'
	);

	const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-missing-work-graph-'));
	try {
		await assertCommandFailure(
			execFileAsync(process.execPath, [workGraphValidator, path.join(temporary, 'missing.json')]),
			'unable to read work graph file'
		);
	} finally {
		await rm(temporary, { recursive: true, force: true });
	}

	await assertCommandFailure(runWorkGraphSource('{ invalid json'), 'invalid work graph JSON');
});

test('work graph validator collects all repairable errors in one run', async () => {
	const graph = workGraph(
		workTask('auth-task', {
			ownedPaths: ['backend-school/src/modules/auth/config.rs'],
			verification: []
		})
	);

	await assert.rejects(runWorkGraph(graph), (error) => {
		assert.equal(error.code, 1);
		for (const expected of [
			'requires at least one verification command',
			'requires protected resource security-identity',
			'has a high-risk owned path',
			'requires schoolorbit_high_risk_implementer'
		]) {
			assert.ok(error.stderr.includes(expected), `expected stderr to contain: ${expected}`);
		}
		assert.doesNotMatch(error.stderr, /\n\s+at /);
		return true;
	});
});

test('skill metadata activates only for SchoolOrbit mutation work', async () => {
	const [skill, metadata] = await Promise.all([
		repositoryFile(skillPath),
		repositoryFile(metadataPath)
	]);
	const values = frontmatter(skill);

	assert.equal(values.name, 'schoolorbit-workflow');
	assert.match(values.description, /^Use when\s/);
	assert.doesNotMatch(
		values.description,
		/\b(?:plan|planning|delegate|delegating|review|reviewing|verify|verification|workflow)\b/i
	);
	assert.match(metadata, /^policy:\n\s+allow_implicit_invocation: true$/m);
	assert.match(metadata, /^\s+default_prompt: ["'].*\$schoolorbit-workflow.*["']$/m);
});

test('project rules and CI own the activated SchoolOrbit workflow', async () => {
	const [rules, workflow] = await Promise.all([
		repositoryFile('.rules'),
		repositoryFile(documentationWorkflowPath)
	]);

	for (const marker of [
		'schoolorbit-workflow',
		'explicit approval',
		'max_concurrent_threads_per_session = 3',
		'frontend-school/tests/static/schoolorbit-agent-workflow.test.mjs'
	]) {
		assert.ok(rules.includes(marker), `.rules must contain: ${marker}`);
	}

	const requiredPaths = [
		'.agents/skills/schoolorbit-workflow/**',
		'.codex/**',
		'frontend-school/tests/static/fixtures/schoolorbit-agent-workflow-scenarios.json',
		'frontend-school/tests/static/schoolorbit-agent-workflow.test.mjs'
	];
	const pullRequest = section(workflow, '  pull_request:', '  push:');
	const push = section(workflow, '  push:', '\npermissions:');
	for (const event of [pullRequest, push]) {
		for (const requiredPath of requiredPaths) {
			assert.ok(
				event.includes(`- "${requiredPath}"`),
				`workflow event must include path: ${requiredPath}`
			);
		}
	}

	assert.ok(workflow.includes('- name: Check repository documentation and agent workflow'));
	assert.ok(
		workflow.includes(
			'run: >-\n          node --test\n          frontend-school/tests/static/documentation-policy.test.mjs\n          frontend-school/tests/static/schoolorbit-agent-workflow.test.mjs'
		)
	);
});

test('workflow state machine places approval before every execution state', async () => {
	const skill = await repositoryFile(skillPath);
	const headings = skill.match(/^#{1,2} .+$/gm) ?? [];

	assert.deepEqual(headings, REQUIRED_HEADINGS);
	const stateMachine = section(skill, '## Workflow State Machine', '## Classify the Request');
	assertMarkersInOrder(stateMachine, [
		'DISCOVER',
		'DRAFT_PLAN',
		'AWAIT_APPROVAL',
		'RECORD_PLAN',
		'EXECUTE_WAVES',
		'INTEGRATE',
		'REVIEW_FIX',
		'VERIFY',
		'COMPLETE',
		'BLOCKED'
	]);
	assertMarkersInOrder(skill, ['## Approval Gate', '## Execute Approved Work']);
});

test('readiness, routing, isolation, and work-graph contracts are explicit', async () => {
	const skill = await repositoryFile(skillPath);
	const readiness = section(skill, '## Readiness Gate', '## Workflow State Machine');
	const routing = section(skill, '## Model Routing', '## Validate the Work Graph');
	const execution = section(skill, '## Execute Approved Work', '## Integrate and Review');
	const normalizedReadiness = normalizeWhitespace(readiness);

	for (const marker of [
		'agents/openai.yaml',
		'allow_implicit_invocation: true',
		'.codex/config.toml',
		'planner.toml',
		'explorer.toml',
		'implementer.toml',
		'high-risk-implementer.toml',
		'reviewer.toml',
		'verifier.toml',
		'scripts/validate-work-graph.mjs',
		'BLOCKED',
		'before planning, delegation, or mutation'
	]) {
		assert.ok(normalizedReadiness.includes(marker), `readiness gate must contain: ${marker}`);
	}

	for (const row of [
		'Planner | `gpt-5.6-sol` | `max` | read-only',
		'Explorer | `gpt-5.6-terra` | `xhigh` | read-only',
		'Implementer | `gpt-5.6-sol` | `xhigh` | workspace-write',
		'High-risk Implementer | `gpt-5.6-sol` | `max` | workspace-write',
		'Reviewer | `gpt-5.6-sol` | `max` | read-only',
		'Verifier | `gpt-5.6-terra` | `high` | workspace-write'
	]) {
		assert.ok(routing.includes(row), `model routing must contain: ${row}`);
	}
	assert.match(
		routing,
		/named custom role[\s\S]*unavailable[\s\S]*complete `developer_instructions`/i
	);
	assert.match(routing, /never (?:rely on|use) silent inheritance/i);

	assert.match(execution, /live parent sandbox[\s\S]*override/i);
	assert.match(execution, /pre- and post-task `git status --short`/i);
	assert.match(execution, /reject[\s\S]*read-only role[\s\S]*write/i);
	assert.match(execution, /separate (?:Git )?index/i);
	assert.match(execution, /at most three/i);
	assert.match(
		skill,
		/\.superpowers\/schoolorbit-workflow\/work-graph\.json[\s\S]*node \.agents\/skills\/schoolorbit-workflow\/scripts\/validate-work-graph\.mjs/i
	);
});

test('pre-approval discovery is filesystem-read-only', async () => {
	const [skill, planner, explorer] = await Promise.all([
		repositoryFile(skillPath),
		repositoryFile('.codex/agents/schoolorbit-planner.toml'),
		repositoryFile('.codex/agents/schoolorbit-explorer.toml')
	]);
	const discovery = normalizeWhitespace(section(skill, '## Discover', '## Plan Contract'));

	assert.match(discovery, /filesystem-read-only/i);
	assert.match(
		discovery,
		/builds?[^.]*tests?[^.]*linters?[^.]*formatters?[^.]*generators?[^.]*package managers?/i
	);
	assert.match(discovery, /caches?[^.]*artifacts?/i);
	assert.match(discovery, /defer[^.]*implementation approval/i);

	for (const [role, profile] of [
		['Planner', planner],
		['Explorer', explorer]
	]) {
		const instructions = normalizeWhitespace(
			tomlMultilineString(profile, 'developer_instructions')
		);
		assert.match(
			instructions,
			/builds?[^.]*tests?[^.]*linters?[^.]*formatters?[^.]*generators?[^.]*package managers?/i,
			`${role} must prohibit pre-approval artifact-producing commands`
		);
		assert.match(instructions, /caches?[^.]*artifacts?/i);
	}
});

test('planner consumes reconciled evidence without repeating explorer discovery', async () => {
	const [skill, planner] = await Promise.all([
		repositoryFile(skillPath),
		repositoryFile('.codex/agents/schoolorbit-planner.toml')
	]);
	const discovery = normalizeWhitespace(section(skill, '## Discover', '## Plan Contract'));
	const planContract = normalizeWhitespace(
		section(skill, '## Plan Contract', '## Approval Gate')
	);
	const instructions = normalizeWhitespace(
		tomlMultilineString(planner, 'developer_instructions')
	);

	assert.match(discovery, /reconciled evidence packet/i);
	assert.match(discovery, /fork_turns: none/i);
	assert.match(planContract, /one bounded Planner/i);
	assert.match(planContract, /defer[^.]*implementation-only skills?[^.]*approval/i);

	assert.match(instructions, /reconciled evidence packet/i);
	assert.match(instructions, /without repository tool calls/i);
	assert.match(instructions, /NEEDS_CONTEXT/i);
});

test('multi-file approval records a mandatory design and implementation plan pair', async () => {
	const skill = await repositoryFile(skillPath);
	const artifacts = normalizeWhitespace(
		section(skill, '## Plan Artifacts', '## Model Routing')
	);

	assert.match(artifacts, /exactly two dated Superpowers artifacts/i);
	assert.match(artifacts, /one design[^.]*docs\/superpowers\/specs\//i);
	assert.match(artifacts, /one implementation plan[^.]*docs\/superpowers\/plans\//i);
	assert.match(artifacts, /both or neither/i);
	assert.match(artifacts, /missing[^.]*return to `DRAFT_PLAN`/i);
});

test('workflow composes required Superpowers and owns parallel writer waves', async () => {
	const skill = await repositoryFile(skillPath);

	for (const subSkill of REQUIRED_SUB_SKILLS) {
		assert.ok(
			skill.includes(`**REQUIRED SUB-SKILL:** Use ${subSkill}`),
			`missing required composition marker for ${subSkill}`
		);
	}
	assert.match(
		skill,
		/superpowers:subagent-driven-development[\s\S]*forbids concurrent implementers/i
	);
});

test('status and measured anti-rationalization contracts remain inspectable', async () => {
	const skill = await repositoryFile(skillPath);
	const status = section(skill, '## Status Contract', '## Quick Reference');
	const rationalizations = section(skill, '## Common Rationalizations', '## Red Flags');
	const normalizedStatus = normalizeWhitespace(status);
	const normalizedRationalizations = normalizeWhitespace(rationalizations);

	for (const marker of [
		'DONE',
		'DONE_WITH_CONCERNS',
		'NEEDS_CONTEXT',
		'BLOCKED',
		'base commit',
		'head commit',
		'changed files',
		'focused commands',
		'exit statuses',
		'output summary',
		'self-review',
		'concerns'
	]) {
		assert.ok(normalizedStatus.includes(marker), `status contract must contain: ${marker}`);
	}

	for (const observedPressure of [
		'"done"',
		'one line',
		'urgent',
		'inert fixtures',
		'disk or time constraints',
		'generic frontier specialist'
	]) {
		assert.ok(
			normalizedRationalizations.includes(observedPressure),
			`measured rationalization must be addressed: ${observedPressure}`
		);
	}
});

test('agent registry enables six model-pinned roles with bounded concurrency', async () => {
	const config = await repositoryFile(agentConfigPath);
	const features = tomlSection(config, 'features');
	const agents = tomlSection(config, 'agents');

	assert.equal(tomlBoolean(features, 'multi_agent'), true);
	assert.equal(tomlBoolean(agents, 'enabled'), true);
	assert.equal(tomlInteger(agents, 'max_concurrent_threads_per_session'), 3);

	for (const [profile, expected] of Object.entries(registeredProfiles)) {
		const registration = tomlSection(config, `agents.${profile}`);
		assert.equal(tomlString(registration, 'description'), expected.description);
		assert.equal(tomlString(registration, 'config_file'), expected.configFile);
	}
});

test('agent profiles pin routing and enforce bounded write contracts', async () => {
	for (const [profile, [model, effort, sandbox]] of Object.entries(expectedProfiles)) {
		const registration = registeredProfiles[profile];
		const source = await repositoryFile(path.join('.codex', registration.configFile));
		const instructions = normalizeWhitespace(tomlMultilineString(source, 'developer_instructions'));

		assert.equal(tomlString(source, 'name'), profile);
		assert.equal(tomlString(source, 'description'), registration.description);
		assert.equal(tomlString(source, 'model'), model);
		assert.equal(tomlString(source, 'model_reasoning_effort'), effort);
		assert.equal(tomlString(source, 'sandbox_mode'), sandbox);
		assert.ok(instructions.includes('`.rules`'), `${profile} must require .rules`);
		for (const marker of roleInstructionMarkers[profile]) {
			assert.ok(instructions.includes(marker), `${profile} instructions must contain: ${marker}`);
		}
	}
});
