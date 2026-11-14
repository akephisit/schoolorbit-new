import {expect,test,type Page} from '@playwright/test';
import {mockCurrentDelivery,deferred,deliveryReply} from './fixtures/current-delivery';

test.use({serviceWorkers:'block'});
const dialog=(page:Page)=>page.getByRole('alertdialog');
const confirm=(page:Page)=>dialog(page).getByRole('button',{name:'ยืนยันลบถาวร',exact:true});
async function select(page:Page,name:string){await page.getByRole('button',{name:new RegExp(`^${name} ครู `)}).click();await expect(page.getByRole('heading',{name,exact:true})).toBeVisible();}
async function create(page:Page,name:string){await page.getByRole('button',{name:'เพิ่มกลุ่ม',exact:true}).click();const modal=page.getByRole('dialog',{name:'เพิ่มกลุ่มเรียน',exact:true});await modal.getByLabel('ชื่อกลุ่ม',{exact:true}).fill(name);await modal.getByRole('button',{name:'สร้างกลุ่มเรียน',exact:true}).click();return modal;}

for(const mode of ['independent','synchronized'] as const)for(const width of [390,1280])for(const dark of [false,true]){
 test(`${mode} current activity retains other groups and supports later groups at ${width}px ${dark?'dark':'light'}`,async({page},info)=>{
  await page.setViewportSize({width,height:900});const fixture=await mockCurrentDelivery(page,{kind:'activity'});const catalog=fixture.graph.offerings[0].catalog;if(catalog.kind!=='activity')throw new Error('activity required');catalog.schedulingMode=mode;
  await page.goto(fixture.detail);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();if(dark)await page.getByRole('button',{name:'Toggle Dark Mode',exact:true}).click();
  const offering=structuredClone(fixture.resource()),retained=structuredClone(fixture.groups()[1]);
  await page.getByRole('button',{name:'ลบกลุ่ม',exact:true}).focus();await page.keyboard.press('Enter');await expect(confirm(page)).toBeEnabled();
  await expect(dialog(page)).toContainText('คะแนนและเช็คชื่อรายคาบ');await expect(dialog(page).locator('dt')).toHaveCount(15);
  await page.screenshot({animations:'disabled',path:info.outputPath(`current-group-delete-${mode}-${width}-${dark}.png`)});
  await confirm(page).click();await expect(dialog(page)).toHaveCount(0);expect(fixture.groups()).toEqual([retained]);expect(fixture.resource()).toEqual(offering);
  await select(page,'กลุ่ม ข');const modal=await create(page,'กลุ่มที่เพิ่มภายหลัง');await expect(modal).toHaveCount(0);
  await expect(page.getByRole('heading',{name:'กลุ่มที่เพิ่มภายหลัง',exact:true})).toBeVisible();expect(fixture.groups().map(group=>group.id)).toEqual([fixture.ids.groupB,fixture.ids.createdGroup]);
  expect(fixture.writes.filter(write=>write.method==='DELETE'||write.method==='POST').map(write=>write.method)).toEqual(['DELETE','POST']);
  await page.getByRole('button',{name:'ลบรายการเปิดสอน',exact:true}).click();await expect(confirm(page)).toBeEnabled();await confirm(page).click();await page.waitForURL(url=>url.pathname==='/staff/academic/delivery');
  expect(fixture.graph.offerings).toEqual([]);expect(fixture.groups()).toEqual([]);expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  expect(fixture.reads.some(url=>/delivery-versions|term-change-sets/.test(url.pathname))).toBe(false);
 });
}

test('removal conflict invalidates its accepted preview and requires a fresh exact revision',async({page})=>{
 const fixture=await mockCurrentDelivery(page,{deleteConflict:true});await page.goto(fixture.detail);const trigger=page.getByRole('button',{name:'ลบกลุ่ม',exact:true});await trigger.click();await expect(confirm(page)).toBeEnabled();
 await page.keyboard.press('Escape');await expect(dialog(page)).toHaveCount(0);await expect(trigger).toBeFocused();expect(fixture.writes.filter(write=>write.method==='DELETE')).toEqual([]);
 await trigger.click();await expect(confirm(page)).toBeEnabled();await confirm(page).click();await expect(dialog(page).getByRole('alert')).toContainText('ข้อมูลเปลี่ยนแล้ว');await expect(confirm(page)).toBeDisabled();expect(fixture.previewReads).toHaveLength(2);
 await dialog(page).getByRole('button',{name:'ตรวจผลกระทบอีกครั้ง',exact:true}).click();await expect(confirm(page)).toBeEnabled();expect(fixture.previewReads).toHaveLength(3);await confirm(page).click();await expect(dialog(page)).toHaveCount(0);
 const writes=fixture.writes.filter(write=>write.method==='DELETE');expect(writes).toHaveLength(2);expect(writes[0].body.rowVersion).toBe(3);expect(writes[1].body.rowVersion).toBe(4);expect(writes[0].body.sourceDigest).not.toBe(writes[1].body.sourceDigest);expect(fixture.groups().map(group=>group.id)).toEqual([fixture.ids.groupB]);
});

test('pending deletion prevents dismissal and duplicate writes',async({page})=>{
 const held=deferred(),fixture=await mockCurrentDelivery(page,{deleteGate:held.promise});try{await page.goto(fixture.detail);await page.getByRole('button',{name:'ลบรายการเปิดสอน',exact:true}).click();await expect(confirm(page)).toBeEnabled();await confirm(page).click();
 await expect(dialog(page).getByRole('button',{name:'กำลังลบ',exact:true})).toBeDisabled();await page.keyboard.press('Escape');await expect(dialog(page)).toBeVisible();await expect(dialog(page).getByRole('button',{name:'ยกเลิก',exact:true})).toBeDisabled();
 await dialog(page).getByRole('button',{name:'กำลังลบ',exact:true}).evaluate((button:HTMLButtonElement)=>button.click());expect(fixture.writes.filter(write=>write.method==='DELETE')).toHaveLength(1);
 }finally{held.release();}await page.waitForURL(url=>url.pathname==='/staff/academic/delivery');
});

for(const permissions of [['learning_offering.read.school'],['learning_offering.read.assigned','learning_offering.manage.assigned']])test(`school deletion is absent for ${permissions.join(',')}`,async({page})=>{
 const fixture=await mockCurrentDelivery(page,{permissions});await page.goto(fixture.detail);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:/^ลบ(กลุ่ม|รายการเปิดสอน)$/})).toHaveCount(0);expect(fixture.previewReads).toEqual([]);expect(fixture.writes).toEqual([]);
});

test('pending group creation is submitted once and its actual accepted response becomes selected',async({page})=>{
 const held=deferred(),fixture=await mockCurrentDelivery(page,{createGate:held.promise});try{await page.goto(fixture.detail);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();const modal=await create(page,'กลุ่มใหม่จริง');await expect(modal.getByRole('button',{name:'กำลังสร้าง',exact:true})).toBeDisabled();await page.keyboard.press('Escape');await expect(modal).toBeVisible();expect(fixture.writes.filter(write=>write.method==='POST')).toHaveLength(1);}finally{held.release();}
 await expect(page.getByRole('heading',{name:'กลุ่มใหม่จริง',exact:true})).toBeVisible();expect(fixture.groups().filter(group=>group.id===fixture.ids.createdGroup)).toHaveLength(1);await expect(page.getByRole('button',{name:/^กลุ่มใหม่จริง /})).toHaveCount(1);
});

test('an in-flight group list may observe the created group before its acknowledgment without duplicating it',async({page})=>{
 const fixture=await mockCurrentDelivery(page),held=deferred();let accepted=false;
 await page.route(`**/api/academic/offerings/${fixture.resource().id}/groups`,async route=>{if(route.request().method()!=='POST')return route.fallback();const body=route.request().postDataJSON();const group={...fixture.groups()[0],id:fixture.ids.createdGroup,name:body.name,rowVersion:1};fixture.groups().push(group);accepted=true;await held.promise;return deliveryReply(route,group);});
 try{await page.goto(fixture.detail);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();await create(page,'กลุ่มที่เห็นก่อนตอบ');await expect.poll(()=>accepted).toBe(true);await page.evaluate(()=>window.dispatchEvent(new Event('focus')));await expect(page.getByRole('button',{name:/^กลุ่มที่เห็นก่อนตอบ /})).toHaveCount(1);}finally{held.release();}
 await expect(page.getByRole('heading',{name:'กลุ่มที่เห็นก่อนตอบ',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:/^กลุ่มที่เห็นก่อนตอบ /})).toHaveCount(1);
});
