import assert from 'node:assert/strict';
import { test } from 'node:test';
import { clampScale, fitChart, zoomChart } from '../../src/lib/school-public/chart-transform.ts';
import { administrationTree } from '../../src/lib/school-public/organization.ts';
test('zoom retains the point under the pointer and clamps both directions', () => {
	const view = { x: 10, y: 20, scale: 0.5 },
		anchor = { x: 120, y: 140 };
	const next = zoomChart(view, 1, anchor);
	assert.equal((anchor.x - next.x) / next.scale, (anchor.x - view.x) / view.scale);
	assert.equal((anchor.y - next.y) / next.scale, (anchor.y - view.y) / view.scale);
	assert.equal(clampScale(-1), 0.08);
	assert.equal(clampScale(9), 2.5);
});
test('fit centers wide and tall charts inside both mobile and desktop bounds', () => {
	for (const width of [375, 1280]) {
		const result = fitChart(width, 560, 1800, 900);
		assert.ok(result.x >= 15);
		assert.ok(result.y >= 15);
		assert.ok(1800 * result.scale <= width - 31);
		assert.ok(900 * result.scale <= 529);
	}
});
test('public administration filtering removes complete subject subtrees without mutating source data', () => {
	const nodes = [
		{
			id: 'root',
			unitType: 'school',
			children: [
				{
					id: 'management',
					unitType: 'management_group',
					children: [
						{
							id: 'subject',
							unitType: 'subject_group',
							children: [{ id: 'nested', unitType: 'work', children: [] }]
						}
					]
				},
				{ id: 'other', unitType: 'work', children: [] }
			]
		}
	];
	const result = administrationTree(nodes);
	assert.deepEqual(
		result[0].children.map((n) => n.id),
		['management', 'other']
	);
	assert.deepEqual(result[0].children[0].children, []);
	assert.equal(nodes[0].children[0].children.length, 1);
});
