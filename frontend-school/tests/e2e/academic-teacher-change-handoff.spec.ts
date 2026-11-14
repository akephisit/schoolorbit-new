import {expect,test,type Page} from '@playwright/test';
import {mockCurrentDelivery,deliveryReply,deferred} from './fixtures/current-delivery';
import type {ReplaceLearningGroupTeachersRequest} from '../../src/lib/api/learning-delivery';

test.use({serviceWorkers:'block'});
async function edit(page:Page){await page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true}).click();await expect(page.getByLabel('ชื่อกลุ่ม',{exact:true})).toBeVisible();}
async function replaceTeacher(page:Page){
 await page.getByRole('button',{name:'นำ ครูต้นทาง ออก',exact:true}).click();
 await page.getByRole('combobox',{name:'เลือกครูผู้สอน',exact:true}).click();await page.getByRole('option',{name:'ครูคนใหม่',exact:true}).click();
 await page.getByRole('button',{name:'เพิ่ม',exact:true}).click();
}
const save=(page:Page)=>page.getByRole('button',{name:'บันทึกครูผู้สอน',exact:true});

test('replaces the current teacher of exactly the selected group using its accepted revision',async({page})=>{
 const fixture=await mockCurrentDelivery(page),retained=structuredClone(fixture.groups()[1]);
 await page.goto(fixture.detail);await edit(page);await replaceTeacher(page);await save(page).click();
 await expect.poll(()=>fixture.writes.filter(row=>row.path.endsWith('/teachers')).length).toBe(1);
 await expect(save(page)).toBeEnabled();const write=fixture.writes.find(row=>row.path.endsWith('/teachers'))!;
 expect(write.path).toBe(`/api/academic/learning-groups/${fixture.ids.groupA}/teachers`);expect(write.method).toBe('PUT');expect(write.body).toEqual({rowVersion:3,teachers:[{teacherId:fixture.ids.teacherB,role:'primary'}]});
 expect(fixture.groups()[1]).toEqual(retained);expect(fixture.groups()[0].teacherAssignments.map(row=>row.teacherId)).toEqual([fixture.ids.teacherB]);
 await page.getByRole('button',{name:'ยกเลิกการแก้ไขกลุ่ม',exact:true}).click();await edit(page);await expect(page.getByRole('button',{name:'นำ ครูคนใหม่ ออก',exact:true})).toBeVisible();
 expect(fixture.reads.filter(url=>/term-change-sets|teacher-handoffs|delivery-versions/.test(url.pathname))).toEqual([]);
});

test('a rejected teacher save retains entered choices until an explicit current revision reload',async({page})=>{
 const fixture=await mockCurrentDelivery(page);let fail=true;const writes:ReplaceLearningGroupTeachersRequest[]=[];
 await page.route(`**/api/academic/learning-groups/${fixture.ids.groupA}/teachers`,async route=>{
  const body:ReplaceLearningGroupTeachersRequest=route.request().postDataJSON();writes.push(body);
  if(fail){fail=false;fixture.groups()[0].rowVersion=4;return deliveryReply(route,'ข้อมูลครูเปลี่ยนแล้ว กรุณาใช้ข้อมูลล่าสุด',409);}return route.fallback();
 });
 await page.goto(fixture.detail);await edit(page);await replaceTeacher(page);await save(page).click();
 await expect(page.getByRole('button',{name:'ใช้ข้อมูลกลุ่มล่าสุด',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'นำ ครูคนใหม่ ออก',exact:true})).toBeVisible();await expect(save(page)).toBeDisabled();expect(writes).toHaveLength(1);expect(fixture.groups()[0].teacherAssignments[0].teacherId).toBe(fixture.ids.actor);
 await page.getByRole('button',{name:'ใช้ข้อมูลกลุ่มล่าสุด',exact:true}).click();await expect(page.getByRole('button',{name:'นำ ครูต้นทาง ออก',exact:true})).toBeVisible();await replaceTeacher(page);await save(page).click();await expect.poll(()=>writes.length).toBe(2);expect(writes.map(row=>row.rowVersion)).toEqual([3,4]);
});

test('a delayed accepted teacher response belongs to its original group after selecting another group',async({page})=>{
 const held=deferred(),fixture=await mockCurrentDelivery(page,{teacherGate:held.promise}),retained=structuredClone(fixture.groups()[1]);
 try{
  await page.goto(fixture.detail);await edit(page);await replaceTeacher(page);await save(page).click();await expect.poll(()=>fixture.writes.filter(row=>row.path.endsWith('/teachers')).length).toBe(1);
  await page.getByRole('button',{name:/^กลุ่ม ข ครู /}).click();await expect(page.getByRole('heading',{name:'กลุ่ม ข',exact:true})).toBeVisible();
 }finally{held.release();}
 await expect.poll(()=>fixture.groups()[0].rowVersion).toBe(4);await expect(page.getByRole('heading',{name:'กลุ่ม ข',exact:true})).toBeVisible();expect(fixture.groups()[1]).toEqual(retained);await edit(page);await expect(page.getByRole('button',{name:'นำ ครูต้นทาง ออก',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'นำ ครูคนใหม่ ออก',exact:true})).toHaveCount(0);
});

test('read-only current teacher information loads without management options or retired handoff workflows',async({page})=>{
 const fixture=await mockCurrentDelivery(page,{permissions:['learning_offering.read.school']});fixture.setCanManage(false);
 await page.goto(fixture.detail);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();await expect(page.getByText('ครูต้นทาง').first()).toBeVisible();await expect(page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true})).toHaveCount(0);
 expect(fixture.count('/api/academic/delivery/management-options')).toBe(0);expect(fixture.writes).toEqual([]);expect(fixture.reads.filter(url=>/teacher-handoffs|term-change-sets|delivery-versions/.test(url.pathname))).toEqual([]);
});
