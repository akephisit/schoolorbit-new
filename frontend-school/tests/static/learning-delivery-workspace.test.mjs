import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import test from 'node:test';
const read=relative=>readFile(new URL(`../../${relative}`,import.meta.url),'utf8');

test('current Delivery uses generated query contracts without operational version or change-set fallback',async()=>{
 const api=await read('src/lib/api/learning-delivery.ts');for(const operation of ['getDeliveryWorkspace','getHomeroomDeliveryWorkspace','getLearningDeliveryManagementOptions'])assert.match(api,new RegExp(`operations\\['${operation}'\\]`));
 for(const type of ['DeliveryWorkspace','HomeroomDeliveryWorkspace','DeliveryWorkspaceQuery'])assert.match(api,new RegExp(`Schemas\\['${type}'\\]`));assert.doesNotMatch(api,/DeliveryVersion|AcademicTermChangeSet|academic_term_id|ApiResponse<unknown>|delivery\/page-view/);
});
test('homeroom current queries are camelCase and curriculum preparation requires reviewed choices',async()=>{
 const openapi=JSON.parse(await readFile(new URL('../../../contracts/openapi/school-api.json',import.meta.url),'utf8'));const operation=openapi.paths['/api/academic/delivery/homerooms'].get;assert.deepEqual(operation.parameters.map(p=>p.name).sort(),['academicTermId','academicYearId']);
 assert.equal(openapi.paths['/api/academic/delivery/page-view'],undefined);for(const schema of ['CreateLearningGroupRequest','UpdateLearningGroupRequest']){assert.equal(openapi.components.schemas[schema].properties.code,undefined);assert.ok(!openapi.components.schemas[schema].required.includes('code'));}
 assert.ok(openapi.components.schemas.ApplyCurriculumOfferingsRequest.required.includes('choices'));assert.ok(openapi.components.schemas.CurriculumPreparationProposal.required.includes('defaultGroups'));assert.ok(openapi.components.schemas.DeliveryWorkspace.required.includes('rowVersion'));
});
test('curriculum grouping refuses empty apply choices while keeping current teacher and group choices explicit',async()=>{
 const source=await read('src/lib/components/learning-delivery/OfferingCurriculumPreview.svelte');assert.match(source,/choice\.action === 'apply'\s*&&\s*choice\.groups\.length === 0/);assert.match(source,/proposal\.defaultGroups/);assert.match(source,/combineGroups/);assert.match(source,/addSplitGroup/);
});
test('the overview streams independent current regions and keeps offering and management reads lazy',async()=>{
 const page=await read('src/routes/(app)/staff/academic/delivery/+page.svelte'),loader=await read('src/routes/(app)/staff/academic/delivery/+page.ts');
 for(const value of ['getHomeroomDeliveryWorkspace','getDeliveryWorkspace','captureRouteLoad','requestFetch: fetch'])assert.ok(loader.includes(value),value);assert.match(loader,/view/);assert.match(loader,/offerings/);assert.match(page,/data\.homerooms/);assert.match(page,/data\.overview/);assert.match(page,/workspace=\{overview\}/);assert.match(page,/shallow: true/);assert.doesNotMatch(loader+page,/listAcademicTermChangeSets|getAcademicTermChangeSet|DeliveryVersion|invalidateAll\(/);
});
test('detail keeps selected group in the URL and reads actual dated roster facts without publication guards',async()=>{
 const loader=await read('src/routes/(app)/staff/academic/delivery/[offeringId]/+page.ts'),page=await read('src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'),roster=await read('src/lib/components/learning-delivery/RosterPreviewPanel.svelte');
 for(const call of ['getLearningOffering','listLearningGroups','getLearningGroup','listDatedRosterMemberships','getLearningGroupRosterTracking'])assert.match(loader,new RegExp(call));assert.match(page,/url\.searchParams\.set\('groupId', id\)/);assert.match(page,/selectGroupId\(group\.id\)/);assert.match(page,/<DatedRosterMemberships/);assert.doesNotMatch(page,/rosterStatus|teachersLocked/);for(const label of ['studentCode','displayName','gradeLevelName','homeroomName'])assert.match(roster,new RegExp(label));
});
test('weekly operational targets remain separate from pinned catalog standards',async()=>{
 const page=await read('src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'),table=await read('src/lib/components/learning-delivery/DeliveryOverviewTable.svelte');assert.match(page,/updateWeeklyPeriodTarget/);assert.match(page,/rowVersion: offering\.rowVersion/);assert.match(page,/จำนวนคาบนี้ใช้จัดตารางสอน โดยไม่เปลี่ยนค่ามาตรฐานในหลักสูตร/);assert.match(table,/weeklyPeriodTarget/);const graph=JSON.parse(await readFile(new URL('../../../contracts/openapi/school-api.json',import.meta.url),'utf8'));assert.ok(graph.components.schemas.CourseOfferingSnapshot.properties.standardPeriodsPerWeek);
});
test('current teachers remain visible and direct edits require explicit management permission',async()=>{
 const page=await read('src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'),editor=await read('src/lib/components/learning-delivery/LearningGroupEditor.svelte');assert.match(page,/canManage/);assert.match(page,/LEARNING_OFFERING_MANAGE_ASSIGNED/);assert.match(editor,/teacherAssignments/);assert.match(editor,/onReplaceTeachers\(\{ teachers, rowVersion: acceptedRowVersion \}\)/);assert.doesNotMatch(editor,/teachersLocked|publishTeacher|startsOn|endsOn/);
});
test('route seeds are bound to their accepted group, offering and canonical identity and cleared on selection',async()=>{
 const page=await read('src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte');assert.match(page,/initialGroupSeed\.offeringId === offering\?\.id/);assert.match(page,/initialGroupSeed\.groupId === selectedGroup\?\.id/);assert.match(page,/initialGroupSeed\.epoch === authStore\.sessionEpoch/);assert.match(page,/async function selectGroup[\s\S]*initialGroupSeed = null/);assert.match(page,/get\(refreshTrigger\)/);
});
