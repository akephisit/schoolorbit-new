import assert from 'node:assert/strict';
import test from 'node:test';
import {decodeDraftEvent} from '../../src/lib/academic/timetable/collaboration-decoder.ts';
import {collaborationWorkspace} from './timetable-collaboration-fixture.ts';
import {timetableIds} from '../e2e/timetable-test-harness.ts';
const frame=(workspace:unknown)=>JSON.stringify({epoch:timetableIds.blockA,seq:0,type:'TimetableStateSync',payload:{connectionId:timetableIds.user,workspace,users:[],reservations:[]}});

test('an authoritative reconnect supplies current Delivery revision, active choices and unchanged timetable identity',()=>{
 const before=collaborationWorkspace();
 const current=structuredClone(before);
 current.deliveryRowVersion=before.deliveryRowVersion+1;
 current.learningGroups[0].registrationOpen=true;
 current.learningGroups[0].eligibleInstructors=[{teacherId:timetableIds.teacherB,displayName:'ครูปัจจุบัน',role:'primary',orderIndex:0}];
 const decoded=decodeDraftEvent(frame(current));
 assert.equal(decoded?.type,'TimetableStateSync');
 if(decoded?.type!=='TimetableStateSync')throw new Error('missing current snapshot');
 assert.equal(decoded.payload.workspace.timetable.id,before.timetable.id);
 assert.equal(decoded.payload.workspace.deliveryRowVersion,2);
 assert.equal(decoded.payload.workspace.learningGroups[0].registrationOpen,true);
 assert.deepEqual(decoded.payload.workspace.learningGroups[0].eligibleInstructors.map(row=>row.teacherId),[timetableIds.teacherB]);
 assert.deepEqual(before.learningGroups[0].eligibleInstructors,[]);
});

test('current snapshots reject missing revisions and retired business source fields',()=>{
 const workspace=collaborationWorkspace();
 for(const timetable of [
  {...workspace.timetable,id:undefined},
  {...workspace.timetable,rowVersion:undefined},
  {...workspace.timetable,rowVersion:Number.NaN},
  {...workspace.timetable,status:'published'},
  {...workspace.timetable,deliveryVersionId:timetableIds.groupA},
  {...workspace.timetable,createdBy:timetableIds.user}
 ])assert.equal(decodeDraftEvent(frame({...workspace,timetable})),null);
 assert.equal(decodeDraftEvent(frame({...workspace,deliveryRowVersion:undefined})),null);
});
