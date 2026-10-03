import assert from 'node:assert/strict';
import test from 'node:test';
import {
	buildPublicOrganizationTree,
	groupPublicOrganizationMembers
} from '../../src/lib/school-public/organization.ts';
const unit = (id: string, parentId: string | null) => ({
	id,
	parentId,
	name: id,
	unitType: 'unit',
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
