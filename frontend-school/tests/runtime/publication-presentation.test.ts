import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import test from 'node:test';
const contract = JSON.parse(await readFile(new URL('../../../contracts/openapi/school-api.json', import.meta.url),'utf8'));

test('current operational models expose revisions without business publication or source identities', () => {
 for (const name of ['Timetable','DeliveryWorkspace','ExamRound']) {
  const model = contract.components.schemas[name];
  assert.ok(model.required.includes('rowVersion'),name);
  for (const field of ['status','publishedAt','publishedBy','publicationNo','sourceVersionId','deliveryVersionId','effectiveFrom','effectiveUntil']) assert.equal(model.properties[field],undefined,`${name}.${field}`);
 }
});
test('operational version, clone and publication endpoints are physically absent', () => {
 for (const route of Object.keys(contract.paths)) assert.doesNotMatch(route,/delivery-versions|timetable-versions|term-change-sets|exam-schedules.*\/(publish|source-preview|sync-source)$/);
});
test('current table reads are nullable and creation is an explicit term command', () => {
 assert.equal(contract.paths['/api/academic/timetable'].get.operationId,'getTimetable');
 assert.equal(contract.paths['/api/academic/timetable'].post.operationId,'createTimetable');
 assert.deepEqual(contract.components.schemas.CreateTimetableRequest.required,['academicTermId']);
});
test('catalog and curriculum publication evidence remains separate from current operational rows', () => {
 assert.ok(contract.components.schemas.CurriculumPublicationSource.properties.publicationNo);
 assert.ok(contract.components.schemas.CourseOfferingSnapshot.properties.subjectVersionId);
 assert.ok(contract.components.schemas.ActivityOfferingSnapshot.properties.activityVersionId);
});
