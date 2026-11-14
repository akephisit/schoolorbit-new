import {expect,test,type Page} from '@playwright/test';
import {mockCurrentDelivery,deferred,deliveryReply} from './fixtures/current-delivery';
test.use({serviceWorkers:'block'});
const editor=(page:Page)=>page.locator('section').filter({has:page.getByRole('heading',{name:/^จัดการกลุ่ม ·/})}).last();
async function open(page:Page,href:string){await page.goto(href);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();await page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true}).click();await expect(editor(page)).toBeVisible();}
const homerooms=(page:Page)=>editor(page).locator(':scope > div > div').filter({has:page.getByRole('heading',{name:'ห้องต้นทางของนักเรียน',exact:true})});

for(const combined of [false,true])for(const width of [390,1280])test(`${combined?'combined':'separate'} current group homeroom edit retains exact identities and unrelated groups at ${width}px`,async({page},info)=>{
 await page.setViewportSize({width,height:900});const fixture=await mockCurrentDelivery(page);if(!combined)fixture.groups()[0].homeroomIds=[fixture.ids.roomA];
 const original=structuredClone(fixture.groups()[0]),other=structuredClone(fixture.groups()[1]),offering=structuredClone(fixture.resource());
 await open(page,fixture.detail);const section=homerooms(page);await section.getByRole('button',{name:'นำ ม.1/1 ออก',exact:true}).click();
 const response=page.waitForResponse(response=>response.request().method()==='PUT'&&response.url().endsWith('/homerooms'));
 await section.getByRole('button',{name:'บันทึกห้องต้นทาง',exact:true}).click();expect((await response).status()).toBe(200);
 expect(fixture.groups()[0]).toEqual({...original,homeroomIds:combined?[fixture.ids.roomB]:[],rowVersion:original.rowVersion+1});expect(fixture.groups()[1]).toEqual(other);expect(fixture.resource()).toEqual(offering);
 const write=fixture.writes.find(write=>write.path.endsWith('/homerooms'))!;expect(write.body).toEqual({homeroomIds:combined?[fixture.ids.roomB]:[],rowVersion:3});
 await page.reload();await page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true}).click();await expect(homerooms(page).getByRole('button',{name:'นำ ม.1/1 ออก',exact:true})).toHaveCount(0);
 expect(fixture.groups()[0].teacherAssignments).toEqual(original.teacherAssignments);await page.screenshot({animations:'disabled',path:info.outputPath(`current-homerooms-${combined}-${width}.png`)});
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);expect(fixture.reads.some(url=>/delivery-versions|term-change-sets/.test(url.pathname))).toBe(false);
});

test('removing a homeroom is a complete direct group replacement and can explicitly re-add that room',async({page})=>{
 const fixture=await mockCurrentDelivery(page);await open(page,fixture.detail);const section=homerooms(page);await section.getByRole('button',{name:'นำ ม.1/1 ออก',exact:true}).click();await section.getByRole('button',{name:'บันทึกห้องต้นทาง',exact:true}).click();await expect.poll(()=>fixture.groups()[0].rowVersion).toBe(4);
 await page.getByRole('combobox',{name:'เลือกห้องต้นทาง',exact:true}).click();await page.getByRole('option',{name:'ม.1/1',exact:true}).click();await section.getByRole('button',{name:'เพิ่มห้องต้นทาง',exact:true}).click();await section.getByRole('button',{name:'บันทึกห้องต้นทาง',exact:true}).click();await expect.poll(()=>fixture.groups()[0].rowVersion).toBe(5);
 expect(fixture.writes.filter(write=>write.path.endsWith('/homerooms')).map(write=>write.body)).toEqual([{homeroomIds:[fixture.ids.roomB],rowVersion:3},{homeroomIds:[fixture.ids.roomB,fixture.ids.roomA],rowVersion:4}]);expect(fixture.groups()[0].id).toBe(fixture.ids.groupA);
});

test('conflicting homeroom save retains the choice and requires a fresh explicit group baseline',async({page})=>{
 const fixture=await mockCurrentDelivery(page);let conflict=true;await page.route(`**/api/academic/learning-groups/${fixture.ids.groupA}/homerooms`,async route=>{if(conflict){conflict=false;fixture.groups()[0].rowVersion++;return deliveryReply(route,'กลุ่มนี้เปลี่ยนแล้ว กรุณาโหลดใหม่',409);}return route.fallback();});
 await open(page,fixture.detail);const section=homerooms(page);await section.getByRole('button',{name:'นำ ม.1/1 ออก',exact:true}).click();await section.getByRole('button',{name:'บันทึกห้องต้นทาง',exact:true}).click();await expect(editor(page).getByRole('alert').filter({hasText:'กลุ่มนี้เปลี่ยนแล้ว'})).toBeVisible();
 await expect(section.getByRole('button',{name:'นำ ม.1/1 ออก',exact:true})).toHaveCount(0);expect(fixture.groups()[0].homeroomIds).toEqual([fixture.ids.roomA,fixture.ids.roomB]);
 await page.getByRole('button',{name:'ใช้ข้อมูลกลุ่มล่าสุด',exact:true}).click();await expect(section.getByRole('button',{name:'นำ ม.1/1 ออก',exact:true})).toBeVisible();await section.getByRole('button',{name:'นำ ม.1/1 ออก',exact:true}).click();await section.getByRole('button',{name:'บันทึกห้องต้นทาง',exact:true}).click();await expect.poll(()=>fixture.groups()[0].rowVersion).toBe(5);expect(fixture.writes.filter(write=>write.path.endsWith('/homerooms'))[0].body.rowVersion).toBe(4);
});

test('a delayed accepted homeroom response for A cannot replace selected B or its teachers',async({page})=>{
 const fixture=await mockCurrentDelivery(page),held=deferred();let accepted=false;await page.route(`**/api/academic/learning-groups/${fixture.ids.groupA}/homerooms`,async route=>{const body=route.request().postDataJSON();const saved={...fixture.groups()[0],homeroomIds:body.homeroomIds,rowVersion:4};Object.assign(fixture.groups()[0],saved);accepted=true;await held.promise;return deliveryReply(route,saved);});
 try{await open(page,fixture.detail);await homerooms(page).getByRole('button',{name:'นำ ม.1/1 ออก',exact:true}).click();await homerooms(page).getByRole('button',{name:'บันทึกห้องต้นทาง',exact:true}).click();await expect.poll(()=>accepted).toBe(true);await page.getByRole('button',{name:/^กลุ่ม ข ครู /}).click();await expect(page.getByRole('heading',{name:'กลุ่ม ข',exact:true})).toBeVisible();held.release();await expect(page.getByRole('heading',{name:'กลุ่ม ข',exact:true})).toBeVisible();await page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true}).click();await expect(editor(page)).toContainText('ครูต้นทาง');await expect(homerooms(page).getByRole('button',{name:'นำ ม.1/2 ออก',exact:true})).toBeVisible();expect(fixture.groups()[1].homeroomIds).toEqual([fixture.ids.roomB]);}finally{held.release();}
});

for(const kind of ['course','activity'] as const)test(`${kind} group removal deletes only the explicit group while whole offering removal is separate`,async({page})=>{
 const fixture=await mockCurrentDelivery(page,{kind});const remaining=structuredClone(fixture.groups()[1]),resource=structuredClone(fixture.resource());await page.goto(fixture.detail);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();await page.getByRole('button',{name:'ลบกลุ่ม',exact:true}).click();const dialog=page.getByRole('alertdialog');await expect(dialog.getByRole('button',{name:'ยืนยันลบถาวร',exact:true})).toBeEnabled();await dialog.getByRole('button',{name:'ยืนยันลบถาวร',exact:true}).click();await expect(dialog).toHaveCount(0);
 expect(fixture.groups()).toEqual([remaining]);expect(fixture.resource()).toEqual(resource);expect(fixture.graph.offerings).toHaveLength(1);await page.getByRole('button',{name:'ลบรายการเปิดสอน',exact:true}).click();await expect(dialog.getByRole('button',{name:'ยืนยันลบถาวร',exact:true})).toBeEnabled();await dialog.getByRole('button',{name:'ยืนยันลบถาวร',exact:true}).click();await page.waitForURL(url=>url.pathname==='/staff/academic/delivery');expect(fixture.graph.offerings).toEqual([]);expect(fixture.groups()).toEqual([]);
});

test('read-only staff retain current room coverage without management options or writes',async({page})=>{
 const fixture=await mockCurrentDelivery(page,{permissions:['learning_offering.read.school']});await page.goto(fixture.detail);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true})).toHaveCount(0);await expect(page.getByRole('button',{name:/^ลบ/})).toHaveCount(0);expect(fixture.count('/api/academic/delivery/management-options')).toBe(0);expect(fixture.writes).toEqual([]);expect(fixture.groups()[0].homeroomIds).toEqual([fixture.ids.roomA,fixture.ids.roomB]);
});
