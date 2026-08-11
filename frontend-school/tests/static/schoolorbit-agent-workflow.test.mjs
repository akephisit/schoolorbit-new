import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const testDirectory = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(testDirectory, '../../..');
const skillPath = '.agents/skills/schoolorbit-workflow/SKILL.md';
const metadataPath = '.agents/skills/schoolorbit-workflow/agents/openai.yaml';
const fixturePath =
	'frontend-school/tests/static/fixtures/schoolorbit-agent-workflow-scenarios.json';

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

test('skill metadata triggers only for SchoolOrbit mutation work during bootstrap', async () => {
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
	assert.match(metadata, /^policy:\n\s+allow_implicit_invocation: false$/m);
	assert.match(metadata, /^\s+default_prompt: ["'].*\$schoolorbit-workflow.*["']$/m);
});

test('workflow state machine places approval before every execution state', async () => {
	const skill = await repositoryFile(skillPath);
	const headings = skill.match(/^#{1,2} .+$/gm) ?? [];

	assert.deepEqual(headings, REQUIRED_HEADINGS);
	const stateMachine = section(
		skill,
		'## Workflow State Machine',
		'## Classify the Request'
	);
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
	assert.match(routing, /named custom role[\s\S]*unavailable[\s\S]*complete `developer_instructions`/i);
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

test('workflow composes required Superpowers and owns parallel writer waves', async () => {
	const skill = await repositoryFile(skillPath);

	for (const subSkill of REQUIRED_SUB_SKILLS) {
		assert.ok(
			skill.includes(`**REQUIRED SUB-SKILL:** Use ${subSkill}`),
			`missing required composition marker for ${subSkill}`
		);
	}
	assert.match(skill, /superpowers:subagent-driven-development[\s\S]*forbids concurrent implementers/i);
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
