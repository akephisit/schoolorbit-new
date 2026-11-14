import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import test from 'node:test';
const contract=JSON.parse(await readFile(new URL('../../../contracts/openapi/school-api.json',import.meta.url),'utf8'));

test('current teacher replacement carries exact group revision and complete role choices',()=>{
 const request=contract.components.schemas.ReplaceLearningGroupTeachersRequest;
 assert.ok(request.required.includes('rowVersion'));
 assert.ok(request.required.includes('teachers'));
 assert.equal(request.properties.startsOn,undefined);
 assert.equal(request.properties.endsOn,undefined);
 assert.deepEqual(contract.components.schemas.TeacherAssignmentInput.required.toSorted(),['role','teacherId']);
});
test('saved assignment identity and activity state remain explicit in current metadata',()=>{
 const teacher=contract.components.schemas.DeliveryTeacher;
 for(const field of ['assignmentId','teacherId','role','rowVersion','isActive'])assert.ok(teacher.required.includes(field),field);
 for(const field of ['startsOn','endsOn','status'])assert.equal(teacher.properties[field],undefined);
});
