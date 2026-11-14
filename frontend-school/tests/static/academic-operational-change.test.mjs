import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
const root=path.resolve(import.meta.dirname,'../..');const read=name=>readFile(path.join(root,name),'utf8');

test('current Delivery uses generated exact-context models and direct typed operations',async()=>{
 const api=await read('src/lib/api/learning-delivery.ts');
 for(const model of ['DeliveryWorkspace','DeliveryWorkspaceQuery','DeliveryDeletionPreview','DeleteDeliveryResourceRequest','DeletedDeliveryResource'])assert.match(api,new RegExp(`Schemas\\['${model}'\\]`));
 for(const operation of ['getDeliveryWorkspace','updateWeeklyPeriodTarget','replaceLearningGroupTeachers','deleteLearningOffering','deleteLearningGroup','listDatedRosterMemberships'])assert.match(api,new RegExp(`export const ${operation}`));
 assert.doesNotMatch(api,/AcademicTermChangeSet|DeliveryVersion|TeacherHandoff|academic_term_id|ApiResponse<unknown>|\bas any\b/);
});
test('overview stays homeroom first and reads current offerings lazily without a version workflow',async()=>{
 const loader=await read('src/routes/(app)/staff/academic/delivery/+page.ts'),page=await read('src/routes/(app)/staff/academic/delivery/+page.svelte');
 assert.match(loader,/getHomeroomDeliveryWorkspace/);assert.match(loader,/getDeliveryWorkspace/);assert.match(loader,/offerings/);
 assert.match(page,/data\.homerooms/);assert.match(page,/data\.overview/);assert.doesNotMatch(loader+page,/listDeliveryVersions|AcademicChangeSet|selectedDeliveryVersion/);
});
test('weekly target edits use the captured current offering revision without altering the catalog standard',async()=>{
 const page=await read('src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte');
 assert.match(page,/updateWeeklyPeriodTarget/);assert.match(page,/rowVersion: offering\.rowVersion/);assert.match(page,/weeklyPeriodTarget/);
 const command=page.slice(page.indexOf('await updateWeeklyPeriodTarget('),page.indexOf('if (current())',page.indexOf('await updateWeeklyPeriodTarget(')));
 assert.doesNotMatch(command,/standardPeriodsPerWeek|catalog|snapshot/);
 assert.doesNotMatch(page,/weeklyPeriodAdjustmentRequest|publishAcademicTermChangeSet|startsOn/);
});
test('destructive deletion binds an explicit preview revision and digest to the current identity',async()=>{
 const source=await read('src/lib/components/learning-delivery/DeliveryResourceDeletion.svelte');
 for(const field of ['academicYearId','academicTermId','rowVersion','sourceDigest'])assert.match(source,new RegExp(`${field}: accepted\\.${field}`));
 assert.match(source,/authStore\.sessionEpoch === epoch/);assert.match(source,/previewEpoch !== authStore\.sessionEpoch/);assert.match(source,/await callback\(result\)/);assert.match(source,/ตรวจผลกระทบอีกครั้ง/);
 assert.doesNotMatch(source,/changeSetRowVersion|targetDeliveryVersionRowVersion/);
});
test('dated membership history preserves inclusive actual intervals without a roster publication stage',async()=>{
 const source=await read('src/lib/components/learning-delivery/DatedRosterMemberships.svelte'),api=await read('src/lib/api/learning-delivery.ts');
 assert.match(source,/leftAt|left_at/);assert.match(source,/นับรวมวันสิ้นสุด/);assert.match(api,/endDatedRosterMembership/);assert.match(api,/addDatedRosterMembership/);
 assert.doesNotMatch(source+api,/publishLearningGroupRoster|rosterStatus|rosterPublishedAt/);
});
test('current resource management remains guarded by permission and exact context',async()=>{
 const source=await read('src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte');
 assert.match(source,/LEARNING_OFFERING_MANAGE_SCHOOL/);assert.match(source,/canDelete/);assert.match(source,/academicTermId/);assert.match(source,/authStore\.sessionEpoch/);
 assert.doesNotMatch(source,/\.status === 'published'|teachersLocked|TeacherHandoff/);
});
