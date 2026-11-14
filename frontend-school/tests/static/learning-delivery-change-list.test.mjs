import assert from 'node:assert/strict';
import test from 'node:test';
import {deliveryChangePage} from '../../src/lib/academic/delivery-pagination.ts';

test('current missing-work pagination preserves stable resources, duplicate labels and input order',()=>{
 const items=Array.from({length:19},(_,id)=>({id:String(id),name:'ชื่อซ้ำ'}));const before=structuredClone(items);
 const first=deliveryChangePage(items,0),second=deliveryChangePage(items,1),last=deliveryChangePage(items,2);
 assert.deepEqual(first.items.map(item=>item.id),items.slice(0,8).map(item=>item.id));assert.deepEqual(second.items.map(item=>item.id),items.slice(8,16).map(item=>item.id));assert.deepEqual(last.items.map(item=>item.id),['16','17','18']);assert.deepEqual(items,before);
});
test('pagination clamps stale page selection after a current resource is removed',()=>{
 const remaining=[{id:'retained'}];const page=deliveryChangePage(remaining,7);assert.equal(page.page,0);assert.equal(page.pageCount,1);assert.equal(page.total,1);assert.equal(page.items[0],remaining[0]);
});
test('empty current coverage has one empty page and never fabricates a source or resource',()=>{assert.deepEqual(deliveryChangePage([],-3),{items:[],page:0,pageCount:1,total:0});});
test('different presentation page sizes never alter the normalized current resource objects',()=>{
 const items=[{id:'a',rowVersion:4},{id:'b',rowVersion:8},{id:'c',rowVersion:1}];
 assert.deepEqual(deliveryChangePage(items,1,2).items,[items[2]]);assert.equal(deliveryChangePage(items,0,2).items[0],items[0]);assert.deepEqual(items.map(item=>item.rowVersion),[4,8,1]);
});
