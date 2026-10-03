import assert from 'node:assert/strict';
import test from 'node:test';
import { buildPublicOrganizationTree } from '../../src/lib/school-public/organization.ts';
const unit = (id: string, parentId: string | null) => ({
	id,
	parentId,
	name: id,
	unitType: 'unit',
	leaders: []
});
test('public tree preserves hierarchy, input order, orphan units and vacant leaders', () => {
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
	assert.deepEqual(roots[0].leaders, []);
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
