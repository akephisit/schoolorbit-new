import assert from 'node:assert/strict';
import test from 'node:test';
import {
	buildPublicOrganizationTree,
	groupPublicOrganizationMembers,
	publicExecutiveTeam,
	publicPersonnelGroups
} from '../../src/lib/school-public/organization.ts';
const unit = (id: string, parentId: string | null) => ({
	id,
	parentId,
	name: id,
	unitType: 'unit',
	learningAreaId: null,
	members: []
});
test('public tree preserves hierarchy, input order, orphan units and vacant members', () => {
	const roots = buildPublicOrganizationTree([
		unit('root', null),
		unit('child', 'root'),
		unit('orphan', 'inactive')
	]);
	assert.deepEqual(
		roots.map((n) => n.id),
		['root', 'orphan']
	);
	assert.deepEqual(
		roots[0].children.map((n) => n.id),
		['child']
	);
	assert.deepEqual(roots[0].members, []);
});
test('cyclic legacy structures terminate and preserve each unit exactly once', () => {
	const roots = buildPublicOrganizationTree([unit('a', 'b'), unit('b', 'a'), unit('self', 'self')]);
	const flattened: string[] = [];
	const visit = (nodes: typeof roots) => {
		for (const n of nodes) {
			flattened.push(n.id);
			visit(n.children);
		}
	};
	visit(roots);
	assert.deepEqual(flattened.sort(), ['a', 'b', 'self']);
});

test('public member groups order every role and preserve all names and custom titles', () => {
	const member = (name: string, positionCode: string, positionTitle: string | null = null) => ({
		personKey: name,
		name,
		positionCode,
		positionTitle
	});
	const groups = groupPublicOrganizationMembers([
		member('member-a', 'member', 'ครูฝ่ายวิชาการ'),
		member('deputy', 'deputy_head'),
		member('head', 'head'),
		member('member-b', 'member'),
		member('coordinator', 'coordinator'),
		member('director', 'director'),
		member('deputy-director', 'deputy_director')
	]);
	assert.deepEqual(
		groups.map((group) => group.code),
		['director', 'deputy_director', 'head', 'deputy_head', 'coordinator', 'member']
	);
	assert.equal(groups.at(-1)?.label, 'สมาชิก');
	assert.deepEqual(
		groups.at(-1)?.members.map((member) => member.name),
		['member-a', 'member-b']
	);
	assert.equal(groups.at(-1)?.members[0].positionTitle, 'ครูฝ่ายวิชาการ');
});

const member = (personKey: string, positionCode: string, name = personKey) => ({
	personKey,
	name,
	positionCode,
	positionTitle: null,
	avatarUrl: null
});
const department = (
	id: string,
	unitType: string,
	members: ReturnType<typeof member>[],
	parentId: string | null = 'school'
) => ({
	...unit(id, parentId),
	unitType,
	learningAreaId: unitType === 'subject_group' ? `area-${id}` : null,
	members
});

test('executives use the highest available role, retain equal leaders, and omit ordinary members', () => {
	const team = publicExecutiveTeam([
		department('school', 'school', [member('director', 'director')], null),
		department('academic', 'management_group', [
			member('head', 'head'),
			member('deputy', 'deputy_director'),
			member('ordinary', 'member')
		]),
		department('general', 'management_group', [
			member('head-a', 'head'),
			member('head-b', 'head'),
			member('assistant', 'deputy_head')
		]),
		department('budget', 'management_group', [member('coordinator', 'coordinator')]),
		department('vacant', 'management_group', []),
		department('math', 'subject_group', [member('subject-head', 'head')]),
		department('subject-work', 'work', [member('not-an-executive', 'director')], 'math')
	]);
	assert.deepEqual(
		team.directors.map((p) => p.member.personKey),
		['director']
	);
	assert.deepEqual(
		team.deputies.map((p) => p.member.personKey),
		['deputy']
	);
	assert.deepEqual(
		team.leaders.map((p) => p.member.personKey),
		['head-a', 'head-b', 'coordinator']
	);
});

test('one executive portrait retains all responsibilities and same-name people stay distinct', () => {
	const team = publicExecutiveTeam([
		department('school', 'school', [member('one', 'deputy_director', 'ชื่อซ้ำ')], null),
		department('academic', 'management_group', [member('one', 'head', 'ชื่อซ้ำ')]),
		department('general', 'management_group', [member('two', 'head', 'ชื่อซ้ำ')])
	]);
	assert.equal(team.deputies.length, 1);
	assert.deepEqual(
		team.deputies[0].responsibilities.map((r) => r.unitId),
		['school', 'academic']
	);
	assert.deepEqual(
		team.leaders.map((p) => p.member.personKey),
		['two']
	);
});

test('an empty roster has no invented executive tiers', () => {
	assert.deepEqual(publicExecutiveTeam([]), { directors: [], deputies: [], leaders: [] });
});

test('directories keep every leaf member, vacant work, nested groups and subject branches separate', () => {
	const units = [
		department('school', 'school', [], null),
		department('academic', 'management_group', [
			member('deputy', 'deputy_director'),
			member('head', 'head'),
			member('staff', 'member')
		]),
		department(
			'work',
			'division',
			[member('work-head', 'head'), member('work-staff', 'member')],
			'academic'
		),
		department('leaf', 'work', [member('leaf-staff', 'member')], 'work'),
		department('vacant', 'work', [], 'academic'),
		department(
			'math',
			'subject_group',
			[member('math-head', 'head'), member('math-teacher', 'member')],
			'academic'
		),
		department('math-work', 'work', [member('math-staff', 'member')], 'math'),
		department('nested', 'management_group', [member('nested-head', 'head')], 'work')
	];
	const groups = publicPersonnelGroups(units, 'management_group');
	assert.deepEqual(
		groups.map((g) => g.unit.id),
		['academic', 'nested']
	);
	assert.deepEqual(
		groups[0].members.map((p) => p.member.personKey),
		['head', 'work-head', 'nested-head', 'staff', 'work-staff', 'leaf-staff']
	);
	assert.deepEqual(
		groups[0].children.map((g) => g.unit.id),
		['work', 'vacant']
	);
	assert.deepEqual(
		groups[0].children[0].members.map((p) => p.member.personKey),
		['nested-head', 'work-staff', 'leaf-staff']
	);
	assert.equal(groups[0].children[0].children[0].leaders[0].member.personKey, 'leaf-staff');
	const subjects = publicPersonnelGroups(units, 'subject_group');
	assert.deepEqual(
		subjects.map((g) => g.unit.id),
		['math']
	);
	assert.equal(subjects[0].leaders[0].member.personKey, 'math-head');
	assert.equal(subjects[0].members[0].member.personKey, 'math-teacher');
	assert.equal(subjects[0].children[0].unit.id, 'math-work');
});

test('multiple roles in a group render once while preserving titles and same-name colleagues', () => {
	const groups = publicPersonnelGroups(
		[
			department('academic', 'management_group', [
				member('one', 'head', 'ชื่อซ้ำ'),
				member('one', 'member', 'ชื่อซ้ำ'),
				member('two', 'member', 'ชื่อซ้ำ')
			])
		],
		'management_group'
	);
	assert.equal(groups[0].leaders.length, 1);
	assert.deepEqual(
		groups[0].members.map((p) => p.member.personKey),
		['two']
	);
});

test('parent roster deduplicates sources across every depth without promoting inherited heads', () => {
	const groups = publicPersonnelGroups([
		department('parent', 'management_group', [member('direct', 'head'), member('shared', 'member')]),
		department('child', 'division', [member('shared', 'head'), member('deep', 'head')], 'parent'),
		department('leaf', 'division', [member('shared', 'member')], 'child')
	], 'management_group');
	assert.deepEqual(groups[0].leaders.map((person) => person.member.personKey), ['direct']);
	assert.deepEqual(groups[0].members.map((person) => person.member.personKey), ['shared', 'deep']);
	assert.deepEqual(groups[0].members[0].responsibilities.map((source) => source.unitId).sort(), ['child', 'leaf', 'parent']);
});

test('public learning areas follow every actual registry linkage including linked activity divisions and vacant custom areas', () => {
	const linked = Array.from({ length: 12 }, (_, index) => ({
		...department(`custom-${index}`, index === 11 ? 'division' : 'subject_group', []),
		learningAreaId: `registry-${index}`
	}));
	const unlinked = { ...department('legacy-subject', 'subject_group', []), learningAreaId: null };
	const groups = publicPersonnelGroups([...linked, unlinked], 'subject_group');
	assert.deepEqual(groups.map((group) => group.unit.id), linked.map((item) => item.id));
	assert.equal(groups[11].unit.unitType, 'division');
	assert.equal(publicPersonnelGroups(linked, 'management_group').length, 0);
});
