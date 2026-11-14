import {expect,test,type Page} from '@playwright/test';
import {mockCurrentDelivery,replaceDeliverySession,deferred} from './fixtures/current-delivery';
test.use({serviceWorkers:'block'});
const dialog=(page:Page)=>page.getByRole('alertdialog');
const confirm=(page:Page)=>dialog(page).getByRole('button',{name:'ยืนยันลบถาวร',exact:true});
async function browserTurn(page:Page){await page.evaluate(()=>new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve()))));}

test('a delayed A preview cannot become B confirmation on the same route with management still granted',async({page})=>{
 const fixture=await mockCurrentDelivery(page);const held=deferred();let previews=0;
 await page.route(`**/api/academic/offerings/${fixture.resource().id}/deletion-preview?*`,async route=>{previews++;if(previews===1)await held.promise;await route.fallback();});
 try{
  await page.goto(fixture.detail);await page.getByRole('button',{name:'ลบรายการเปิดสอน',exact:true}).click();
  await expect.poll(()=>previews).toBe(1);await expect(dialog(page).getByRole('status')).toBeVisible();
  await replaceDeliverySession(page,fixture);await expect(dialog(page)).toHaveCount(0);held.release();await browserTurn(page);
  expect(fixture.writes.filter(write=>write.method==='DELETE')).toEqual([]);
  await expect(dialog(page)).toHaveCount(0);await page.getByRole('button',{name:'ลบรายการเปิดสอน',exact:true}).click();
  await expect(confirm(page)).toBeEnabled();expect(previews).toBe(2);
  await confirm(page).click();await page.waitForURL(url=>url.pathname==='/staff/academic/delivery');
  expect(fixture.writes.filter(write=>write.method==='DELETE')).toHaveLength(1);
 }finally{held.release();}
});

test('B cannot submit the already accepted A preview and must review a fresh scoped preview',async({page})=>{
 const fixture=await mockCurrentDelivery(page);await page.goto(fixture.detail);await page.getByRole('button',{name:'ลบรายการเปิดสอน',exact:true}).click();
 await expect(confirm(page)).toBeEnabled();expect(fixture.previewReads).toHaveLength(1);const original=await confirm(page).elementHandle();
 await replaceDeliverySession(page,fixture);await expect(dialog(page)).toHaveCount(0);
 await original!.evaluate((button:HTMLButtonElement)=>button.click());await browserTurn(page);
 expect(fixture.writes.filter(write=>write.method==='DELETE')).toEqual([]);
 await page.getByRole('button',{name:'ลบรายการเปิดสอน',exact:true}).click();await expect(confirm(page)).toBeEnabled();
 expect(fixture.previewReads).toHaveLength(2);expect(fixture.previewReads[0].epochSessionId).not.toBe(fixture.previewReads[1].epochSessionId);
 await confirm(page).click();await page.waitForURL(url=>url.pathname==='/staff/academic/delivery');
 expect(fixture.writes.filter(write=>write.method==='DELETE')).toHaveLength(1);
});

test('a late accepted A offering deletion cannot run its navigation callback in identity B',async({page})=>{
 const held=deferred();const fixture=await mockCurrentDelivery(page,{deleteGate:held.promise});
 try{
  await page.goto(fixture.detail);await page.getByRole('button',{name:'ลบรายการเปิดสอน',exact:true}).click();await expect(confirm(page)).toBeEnabled();await confirm(page).click();
  await expect.poll(()=>fixture.writes.filter(write=>write.method==='DELETE').length).toBe(1);
  await expect(dialog(page).getByRole('button',{name:'ยกเลิก',exact:true})).toBeDisabled();
  await replaceDeliverySession(page,fixture);await expect(dialog(page)).toHaveCount(0);
  const receipt=page.waitForResponse(response=>response.request().method()==='DELETE');held.release();await(await receipt).finished();await browserTurn(page);
  expect(new URL(page.url()).pathname).toBe(`/staff/academic/delivery/${fixture.resource().id}`);
  expect(fixture.graph.offerings).toEqual([]);expect(fixture.writes.filter(write=>write.method==='DELETE')).toHaveLength(1);
 }finally{held.release();}
});

test('late A group deletion does not clear B selection and B can review and delete another current group',async({page})=>{
 const held=deferred();const fixture=await mockCurrentDelivery(page,{deleteGate:held.promise});
 try{
  await page.goto(`${fixture.detail}&groupId=${fixture.ids.groupA}`);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'ลบกลุ่ม',exact:true}).click();await expect(confirm(page)).toBeEnabled();await confirm(page).click();
  await expect.poll(()=>fixture.writes.filter(write=>write.method==='DELETE').length).toBe(1);
  await replaceDeliverySession(page,fixture);await expect(dialog(page)).toHaveCount(0);
  await page.getByRole('button',{name:/^กลุ่ม ข ครู /}).click();await expect(page.getByRole('heading',{name:'กลุ่ม ข',exact:true})).toBeVisible();
  const groupsPath=`/api/academic/offerings/${fixture.resource().id}/groups`;const before=fixture.count(groupsPath);
  const receipt=page.waitForResponse(response=>response.request().method()==='DELETE');held.release();await(await receipt).finished();await browserTurn(page);
  await expect(page.getByRole('heading',{name:'กลุ่ม ข',exact:true})).toBeVisible();expect(fixture.count(groupsPath)).toBe(before);
  expect(fixture.groups().map(group=>group.id)).toEqual([fixture.ids.groupB]);
  await page.getByRole('button',{name:'ลบกลุ่ม',exact:true}).click();await expect(confirm(page)).toBeEnabled();await confirm(page).click();
  await expect.poll(()=>fixture.groups().length).toBe(0);expect(fixture.writes.filter(write=>write.method==='DELETE')).toHaveLength(2);
 }finally{held.release();}
});
