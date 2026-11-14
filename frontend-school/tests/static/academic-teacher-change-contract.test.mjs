import assert from 'node:assert/strict';
import {access,readFile} from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
const root=path.resolve(import.meta.dirname,'../..'),read=file=>readFile(path.join(root,file),'utf8');

test('teacher replacement is a direct typed complete group command with optimistic revision',async()=>{
 const api=await read('src/lib/api/learning-delivery.ts');
 assert.match(api,/Schemas\['ReplaceLearningGroupTeachersRequest'\]/);assert.match(api,/export const replaceLearningGroupTeachers/);assert.match(api,/apiClient\.put<LearningGroup>/);
 assert.doesNotMatch(api,/TeacherHandoff|AcademicTeacherChange|\/handoff/);
});
test('teacher editor uses bounded management options and projects only teacher IDs and roles',async()=>{
 const source=await read('src/lib/components/learning-delivery/LearningGroupEditor.svelte');
 assert.match(source,/managementOptions\.teachers/);assert.match(source,/group\.teacherAssignments\.map\(\(\{ teacherId, role \}\)/);assert.match(source,/onReplaceTeachers/);assert.match(source,/rowVersion: acceptedRowVersion/);
 assert.doesNotMatch(source,/startsOn|endsOn|referenceDate|lookupStaff|listLearningGroups|<select(?:\s|>)/i);
});
test('accepted teacher save revision comes from the actual response and failures require explicit baseline review',async()=>{
 const source=await read('src/lib/components/learning-delivery/LearningGroupEditor.svelte');
 assert.match(source,/const accepted = await action\(\)/);assert.match(source,/acceptedRowVersion = accepted\.rowVersion/);assert.match(source,/needsReload = true/);assert.match(source,/ใช้ข้อมูลกลุ่มล่าสุด/);
 assert.doesNotMatch(source,/acceptedRowVersion = group\.rowVersion/);
});
test('retired teacher change and handoff implementations cannot remain as hidden workflow fallback',async()=>{
 for(const file of ['src/lib/components/learning-delivery/AcademicTeacherChangeForm.svelte','src/lib/components/learning-delivery/TeacherHandoffPanel.svelte','src/lib/academic/teacher-change-selection.ts'])await assert.rejects(access(path.join(root,file)),error=>error.code==='ENOENT');
});
test('saved placed instructors remain distinct from current teacher targets and direct placement validation',async()=>{
 const source=await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
 assert.match(source,/blockInstructorIds/);assert.match(source,/blockTargetTeacherIds/);assert.match(source,/updateTimetableBlock/);assert.match(source,/previewTimetableBlockPlacement/);
});
test('teacher metadata has stable assignment identity, activity state and no operational effective dates',async()=>{
 const contract=JSON.parse(await read('../contracts/openapi/school-api.json'));const teacher=contract.components.schemas.DeliveryTeacher;
 for(const field of ['assignmentId','teacherId','role','rowVersion','isActive'])assert.ok(teacher.required.includes(field));
 for(const field of ['startsOn','endsOn','status'])assert.equal(teacher.properties[field],undefined);
});
