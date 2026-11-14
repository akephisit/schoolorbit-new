import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { mountApiCalls } from '../helpers/route-startup-policy.mjs';

const routeDirectory = path.resolve(
	import.meta.dirname,
	'../../src/routes/(app)/staff/academic/delivery/[offeringId]'
);
const rosterComponent = path.resolve(
	import.meta.dirname,
	'../../src/lib/components/learning-delivery/DatedRosterMemberships.svelte'
);

async function routeSource(filename) {
	return readFile(path.join(routeDirectory, filename), 'utf8');
}

test('offering identity, group list and selected group start as independent current route reads',async()=>{
 const [loader,page,roster]=await Promise.all([routeSource('+page.ts'),routeSource('+page.svelte'),readFile(rosterComponent,'utf8')]);
 for(const call of ['getLearningOffering','listLearningGroups','getLearningGroup','listDatedRosterMemberships','getLearningGroupRosterTracking'])assert.match(loader,new RegExp(call));
 for(const field of ['offering','groups','selectedGroup','memberships','rosterTracking'])assert.ok(loader.includes(field));
 assert.match(loader,/requestFetch:\s*fetch/);assert.doesNotMatch(loader,/DeliveryVersion|getHomeroomDeliveryWorkspace|getLearningDeliveryManagementOptions/);
 assert.match(page,/data\.offering/);assert.match(page,/data\.groups/);assert.match(page,/data\.selectedGroup/);assert.match(page,/initialGroupSeedIsCurrent/);assert.deepEqual(mountApiCalls(page),['subscribe:getLearningOffering','subscribe:listLearningGroups']);assert.match(roster,/initialMemberships/);assert.doesNotMatch(roster,/\bonMount\s*\(/);
});
test('heavy current offering detail links preload on tap instead of hover',async()=>{
 for(const filename of ['HomeroomDeliveryWorkspace.svelte','DeliveryOverviewTable.svelte']){
  const source=await readFile(path.resolve(import.meta.dirname,'../../src/lib/components/learning-delivery',filename),'utf8');
  const links=[...source.matchAll(/href=\{(?:offeringHref\(item\.offeringId\)|detailUrl\(item\.id\))\}/g)];assert.ok(links.length>0,filename);
  for(const link of links)assert.match(source.slice(link.index,link.index+300),/data-sveltekit-preload-data="tap"/,filename);
 }
});
