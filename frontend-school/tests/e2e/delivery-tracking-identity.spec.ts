import {expect,test,type Page} from '@playwright/test';
import {mockCurrentDelivery,replaceDeliverySession,deferred,deliveryReply} from './fixtures/current-delivery';
import type {LearningGroupRosterTracking} from '../../src/lib/api/learning-delivery';
test.use({serviceWorkers:'block'});
const panel=(page:Page)=>page.getByTestId('roster-tracking');
async function chooseAutomatic(page:Page){await panel(page).getByRole('combobox',{name:'วิธีจัดรายชื่อ',exact:true}).click();await page.getByRole('option',{name:'ติดตามห้องอัตโนมัติ',exact:true}).click();}
async function browserTurn(page:Page){await page.evaluate(()=>new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve()))));}

test('a held tracking read from actor A cannot install its policy in B with the same resource IDs',async({page})=>{
 const fixture=await mockCurrentDelivery(page),held=deferred();const path=`/api/academic/learning-groups/${fixture.ids.groupA}/roster-tracking`;let reads=0;
 await page.route(`**${path}`,async route=>{const snapshot=structuredClone(fixture.tracking.get(fixture.ids.groupA));if(++reads===1)await held.promise;return deliveryReply(route,snapshot);});
 try{await page.goto(fixture.detail);await expect.poll(()=>reads).toBe(1);fixture.groups()[0].rowVersion=4;fixture.tracking.set(fixture.ids.groupA,{mode:'homeroom',effectiveFrom:'2026-11-12',groupRowVersion:4});fixture.setActor(fixture.ids.teacherB);await replaceDeliverySession(page,fixture);await expect.poll(()=>reads).toBe(2);await expect(panel(page).getByRole('combobox',{name:'วิธีจัดรายชื่อ',exact:true})).toHaveText('ติดตามห้องอัตโนมัติ');held.release();await browserTurn(page);await expect(panel(page).getByRole('combobox',{name:'วิธีจัดรายชื่อ',exact:true})).toHaveText('ติดตามห้องอัตโนมัติ');await expect(panel(page).getByRole('button',{name:'วันที่เริ่มติดตาม',exact:true})).toContainText('12 พ.ย. 2569');expect(fixture.writes.filter(write=>write.method==='PUT')).toEqual([]);}finally{held.release();}
});

for(const outcome of ['accepted','rejected'] as const)test(`late A ${outcome} tracking acknowledgment cannot overwrite B policy or clear B pending save`,async({page})=>{
 const fixture=await mockCurrentDelivery(page),first=deferred(),second=deferred();const path=`/api/academic/learning-groups/${fixture.ids.groupA}/roster-tracking`;const writes:Record<string,unknown>[]=[];
 await page.route(`**${path}`,async route=>{
  if(route.request().method()!=='PUT')return route.fallback();const body=route.request().postDataJSON();writes.push(body);
  const accepted:LearningGroupRosterTracking={mode:body.mode,effectiveFrom:body.effectiveFrom??null,groupRowVersion:body.rowVersion+1};
  if(writes.length===1){if(outcome==='accepted'){fixture.groups()[0].rowVersion=accepted.groupRowVersion;fixture.tracking.set(fixture.ids.groupA,accepted);}await first.promise;return outcome==='accepted'?deliveryReply(route,accepted):deliveryReply(route,'คำขอเดิมขัดแย้งกับข้อมูลปัจจุบัน',409);}
  await second.promise;expect(body.rowVersion).toBe(5);fixture.groups()[0].rowVersion=6;fixture.tracking.set(fixture.ids.groupA,accepted);return deliveryReply(route,accepted);
 });
 try{
  await page.goto(fixture.detail);await chooseAutomatic(page);await panel(page).getByRole('button',{name:'บันทึกวิธีจัดรายชื่อ',exact:true}).click();await expect.poll(()=>writes.length).toBe(1);
  // Another accepted edit precedes B's fresh read; A's response still describes its original revision.
  fixture.groups()[0].rowVersion=5;fixture.tracking.set(fixture.ids.groupA,{mode:'manual',effectiveFrom:null,groupRowVersion:5});fixture.setActor(fixture.ids.teacherB);await replaceDeliverySession(page,fixture);await expect(panel(page).getByRole('combobox',{name:'วิธีจัดรายชื่อ',exact:true})).toHaveText('จัดรายชื่อเอง');await chooseAutomatic(page);await panel(page).getByRole('button',{name:'บันทึกวิธีจัดรายชื่อ',exact:true}).click();await expect.poll(()=>writes.length).toBe(2);
  first.release();await browserTurn(page);await expect(panel(page).getByRole('button',{name:'กำลังบันทึก',exact:true})).toBeDisabled();await expect(panel(page).getByRole('alert')).toHaveCount(0);expect(writes[1].rowVersion).toBe(5);
  second.release();await expect(panel(page)).toContainText('บันทึกวิธีจัดรายชื่อแล้ว');await expect(panel(page).getByRole('button',{name:'บันทึกวิธีจัดรายชื่อ',exact:true})).toBeDisabled();expect(fixture.tracking.get(fixture.ids.groupA)?.groupRowVersion).toBe(6);expect(writes).toHaveLength(2);
 }finally{first.release();second.release();}
});
