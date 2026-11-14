import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import test from 'node:test';
import {registerDeliveryReconcile} from '../../src/lib/academic/delivery-reconcile.ts';

test('current removal requires exact preview context, resource revision and owner digest',async()=>{
 const api=JSON.parse(await readFile(new URL('../../../contracts/openapi/school-api.json',import.meta.url),'utf8'));
 assert.deepEqual(api.components.schemas.DeleteDeliveryResourceRequest.required.toSorted(),['academicTermId','academicYearId','rowVersion','sourceDigest']);
 assert.equal(api.paths['/api/academic/offerings/{id}/deletion-preview'].get.operationId,'previewLearningOfferingDeletion');
 assert.equal(api.paths['/api/academic/learning-groups/{id}'].delete.operationId,'deleteLearningGroup');
 for(const name of Object.keys(api.paths))assert.doesNotMatch(name,/delivery-versions|term-change-sets/);
});
test('current reconciliation is event driven, pauses while hidden, and unregisters every listener without polling',()=>{
 const win=new EventTarget(),doc=new EventTarget();doc.hidden=false;let reads=0;
 win.setInterval=()=>{throw new Error('current Delivery must not poll')};
 const dispose=registerDeliveryReconcile(()=>reads++,win,doc);assert.equal(reads,0);
 win.dispatchEvent(new Event('focus'));assert.equal(reads,1);
 doc.hidden=true;doc.dispatchEvent(new Event('visibilitychange'));win.dispatchEvent(new Event('focus'));assert.equal(reads,1);
 doc.hidden=false;doc.dispatchEvent(new Event('visibilitychange'));assert.equal(reads,2);
 dispose();win.dispatchEvent(new Event('focus'));doc.dispatchEvent(new Event('visibilitychange'));assert.equal(reads,2);
});
