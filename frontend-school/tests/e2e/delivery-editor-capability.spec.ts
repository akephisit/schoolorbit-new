import {expect,test,type Page} from '@playwright/test';
import {mockCurrentDelivery,deferred,deliveryReply} from './fixtures/current-delivery';
test.use({serviceWorkers:'block'});
const editor=(page:Page)=>page.locator('section').filter({has:page.getByRole('heading',{name:/^จัดการกลุ่ม ·/})}).last();
for(const pending of ['save','reload'] as const)test(`same-identity capability loss settles pending editor ${pending} without accepting its old response`,async({page})=>{
 const fixture=await mockCurrentDelivery(page,{groupConflict:pending==='reload'}),held=deferred();let entered=false;const path=`/api/academic/learning-groups/${fixture.ids.groupA}`;let saves=0;
 await page.route(`**${path}`,async route=>{
  const method=route.request().method();if(pending==='save'&&method==='PATCH'&&saves++===0){const body=route.request().postDataJSON();entered=true;await held.promise;Object.assign(fixture.groups()[0],body,{rowVersion:4});return deliveryReply(route,fixture.groups()[0]);}
  if(pending==='reload'&&method==='GET'){entered=true;const captured=structuredClone(fixture.groups()[0]);await held.promise;return deliveryReply(route,captured);}return route.fallback();
 });
 try{
  await page.goto(fixture.detail);await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();await page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true}).click();await editor(page).getByLabel('ชื่อกลุ่ม',{exact:true}).fill('ชื่อที่ขอแก้ไขก่อนสิทธิ์เปลี่ยน');await editor(page).getByRole('button',{name:'บันทึกข้อมูลกลุ่ม',exact:true}).click();
  if(pending==='reload'){await expect(editor(page).getByRole('alert').filter({hasText:'ข้อมูลกลุ่มเปลี่ยนแล้ว'})).toBeVisible();await editor(page).getByRole('button',{name:'ใช้ข้อมูลกลุ่มล่าสุด',exact:true}).click();}
  await expect.poll(()=>entered).toBe(true);await expect(page.getByRole('button',{name:'ยกเลิกการแก้ไขกลุ่ม',exact:true})).toBeDisabled();const before=fixture.count(`/api/academic/offerings/${fixture.resource().id}`);fixture.setCanManage(false);await page.evaluate(()=>window.dispatchEvent(new Event('focus')));await expect.poll(()=>fixture.count(`/api/academic/offerings/${fixture.resource().id}`)).toBe(before+1);await expect(page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true})).toHaveCount(0);
  const response=page.waitForResponse(response=>new URL(response.url()).pathname===path&&response.request().method()===(pending==='save'?'PATCH':'GET'));held.release();await(await response).finished();await page.evaluate(()=>new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve()))));await expect(page.getByRole('heading',{name:'กลุ่ม ก',exact:true})).toBeVisible();await expect(editor(page).getByLabel('ชื่อกลุ่ม',{exact:true})).toHaveValue('ชื่อที่ขอแก้ไขก่อนสิทธิ์เปลี่ยน');
  fixture.setCanManage(true);await page.evaluate(()=>window.dispatchEvent(new Event('focus')));const cancel=page.getByRole('button',{name:'ยกเลิกการแก้ไขกลุ่ม',exact:true});await expect(cancel).toBeEnabled();await cancel.click();await expect(page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true})).toBeFocused();await page.getByRole('button',{name:'แก้ไขกลุ่มและครู',exact:true}).click();await expect(editor(page)).toBeVisible();await expect(page.getByRole('button',{name:'ยกเลิกการแก้ไขกลุ่ม',exact:true})).toBeEnabled();await editor(page).getByLabel('ชื่อกลุ่ม',{exact:true}).fill('ตรวจบันทึกหลังเปิดสิทธิ์อีกครั้ง');await editor(page).getByRole('button',{name:'บันทึกข้อมูลกลุ่ม',exact:true}).click();await expect.poll(()=>fixture.groups()[0].rowVersion).toBe(5);
 }finally{held.release();}
});
