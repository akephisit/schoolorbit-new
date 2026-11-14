import {expect,test,type Page,type Route} from '@playwright/test';
import {installTimetableMock,makeTimetableBlock,timetableIds} from './timetable-test-harness';
test.use({serviceWorkers:'block'});
const url=()=>`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&view=homeroom&ownerId=${timetableIds.homeroom}`;
function gate(){let release=()=>{};const promise=new Promise<void>(resolve=>release=resolve);return{promise,release};}
async function editTitle(page:Page,id:string,title:string){
 await page.locator(`[data-block-id="${id}"]`).getByRole('button',{name:/ดูรายละเอียด/}).click();
 const dialog=page.getByRole('dialog');await expect(dialog.getByLabel('ชื่อที่แสดงในตาราง')).toBeEnabled();
 await dialog.getByLabel('ชื่อที่แสดงในตาราง').fill(title);await dialog.getByRole('button',{name:'บันทึก',exact:true}).click();
}
async function failed(route:Route,kind:'network'|'conflict'|'server'){
 if(kind==='network')await route.abort('failed');
 else await route.fulfill({status:kind==='conflict'?409:500,json:{success:false,error:'ผลการบันทึกยังไม่ยืนยัน'}});
}
async function drop(page:Page,period=timetableIds.period2){
 const tray=page.locator('aside article[data-timetable-source-kind="ordinary_demand"]').first();
 const cell=page.locator(`td[data-timetable-day="MON"][data-timetable-period-id="${period}"]`).first();
 const transfer=await page.evaluateHandle(()=>new DataTransfer());
 await tray.dispatchEvent('dragstart',{dataTransfer:transfer});await cell.dispatchEvent('dragover',{dataTransfer:transfer});
 await cell.dispatchEvent('drop',{dataTransfer:transfer});await tray.dispatchEvent('dragend',{dataTransfer:transfer});return cell;
}

test('read-only current Delivery refresh keeps the same table and sends no lifecycle commands',async({page})=>{
 const mock=await installTimetableMock(page,{permissions:['academic_timetable.read.school'],blocks:[makeTimetableBlock(timetableIds.blockA,timetableIds.period1)]});
 await page.goto(url());await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
 mock.changeDelivery({requiredPeriods:4});
 await expect.poll(mock.workspaceRequestCount).toBe(2);
 await expect(page.locator('header[aria-label="บริบทตารางสอน"]')).toHaveAttribute('data-timetable-id',timetableIds.timetable);
 await expect(page.getByRole('button',{name:'แก้ไข',exact:true})).toHaveCount(0);
 expect(mock.retiredRequests()).toEqual([]);expect(mock.timetableCreateRequestCount()).toBe(0);
});

test('authoritative Delivery refresh preserves an unsaved details form and reacquires its lease',async({page})=>{
 const block=makeTimetableBlock(timetableIds.blockA,timetableIds.period1);const mock=await installTimetableMock(page,{blocks:[block]});
 await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();
 await page.getByRole('button',{name:/ดูรายละเอียด ค21101/}).click();const dialog=page.getByRole('dialog');
 await expect(dialog.getByLabel('ชื่อที่แสดงในตาราง')).toBeEnabled();
 await dialog.getByLabel('ชื่อที่แสดงในตาราง').fill('คาบที่ยังไม่บันทึก');await dialog.getByLabel('หมายเหตุ').fill('หมายเหตุเดิม');
 mock.changeDelivery({requiredPeriods:4});await expect.poll(mock.workspaceRequestCount).toBe(3);
 await expect(dialog.getByLabel('ชื่อที่แสดงในตาราง')).toHaveValue('คาบที่ยังไม่บันทึก');
 await expect(dialog.getByLabel('หมายเหตุ')).toHaveValue('หมายเหตุเดิม');
 await dialog.getByRole('button',{name:'ตรวจสิทธิ์แก้ไขคาบอีกครั้ง',exact:true}).click();
 await expect(dialog.getByRole('button',{name:'บันทึก',exact:true})).toBeEnabled();
 await dialog.getByRole('button',{name:'บันทึก',exact:true}).click();await expect.poll(mock.updateRequestCount).toBe(1);
 expect(mock.lastUpdateBody()?.title).toBe('คาบที่ยังไม่บันทึก');expect(mock.lastUpdateBody()?.note).toBe('หมายเหตุเดิม');
 expect(mock.retiredRequests()).toEqual([]);
});

test('reconciliation drains an accepted in-flight save and preserves its content without replay',async({page})=>{
 const block=makeTimetableBlock(timetableIds.blockA,timetableIds.period1);const mock=await installTimetableMock(page,{blocks:[block]});
 const held=gate();let captured=0;await page.route(`**/api/academic/timetable-blocks/${block.id}`,async route=>{
  if(route.request().method()!=='PUT')return route.fallback();captured++;await held.promise;await route.fallback();
 });
 try{
  await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();await editTitle(page,block.id,'บันทึกที่รับแล้ว');
  await expect.poll(()=>captured).toBe(1);mock.changeDelivery({requiredPeriods:4});
  expect(mock.updateRequestCount()).toBe(0);held.release();
  await expect.poll(mock.updateRequestCount).toBe(1);await expect.poll(()=>mock.blocks()[0].title).toBe('บันทึกที่รับแล้ว');
  await expect(page.locator('[data-pending="true"]')).toHaveCount(0);expect(captured).toBe(1);expect(mock.retiredRequests()).toEqual([]);
 }finally{held.release();}
});

for(const kind of ['network','conflict','server'] as const)test(`dispatched details ${kind} retains unknown outcome until explicit read-and-discard`,async({page})=>{
 const block=makeTimetableBlock(timetableIds.blockA,timetableIds.period1);const mock=await installTimetableMock(page,{blocks:[block]});let writes=0;
 await page.route(`**/api/academic/timetable-blocks/${block.id}`,async route=>{
  if(route.request().method()!=='PUT')return route.fallback();writes++;mock.changeDelivery();await failed(route,kind);
 });
 await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();await editTitle(page,block.id,'รายการที่ยังไม่ยืนยัน');
 await expect(page.getByText(/ยังยืนยันผลการบันทึกไม่ได้ การละทิ้ง/)).toBeVisible();
 await expect(page.getByRole('button',{name:'ทบทวนรายการ',exact:true})).toHaveCount(0);expect(writes).toBe(1);
 const reads=mock.workspaceRequestCount();await page.getByRole('button',{name:'โหลดข้อมูลล่าสุดและละทิ้งรายการ',exact:true}).click();
 await expect(page.getByText(/ยังมีรายการที่ไม่ได้ยืนยันการบันทึก/)).toHaveCount(0);
 expect(mock.workspaceRequestCount()).toBeGreaterThan(reads);expect(writes).toBe(1);expect(mock.blocks()).toEqual([block]);expect(mock.retiredRequests()).toEqual([]);
});

test('two ambiguous saves retain both captured intents without automatic replay',async({page})=>{
 const blocks=[makeTimetableBlock(timetableIds.blockA,timetableIds.period1),makeTimetableBlock(timetableIds.blockB,timetableIds.period2)];
 const mock=await installTimetableMock(page,{blocks});const held=gate();let writes=0;
 await page.route('**/api/academic/timetable-blocks/*',async route=>{if(route.request().method()!=='PUT')return route.fallback();writes++;await held.promise;await route.abort('failed');});
 try{await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();
  await editTitle(page,blocks[0].id,'รายการแรก');await expect.poll(()=>writes).toBe(1);
  await editTitle(page,blocks[1].id,'รายการที่สอง');await expect.poll(()=>writes).toBe(2);held.release();
  await expect(page.getByText('ยังมีรายการที่ไม่ได้ยืนยันการบันทึก (2)',{exact:true})).toBeVisible();
  await expect(page.getByText('รายละเอียดคาบ: รายการแรก',{exact:true})).toBeVisible();await expect(page.getByText('รายละเอียดคาบ: รายการที่สอง',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'ทบทวนรายการ',exact:true})).toHaveCount(0);expect(mock.blocks()).toEqual(blocks);expect(writes).toBe(2);
 }finally{held.release();}
});

test('an ambiguous removal rolls back only its card and never repeats DELETE on reconcile',async({page})=>{
 const block=makeTimetableBlock(timetableIds.blockA,timetableIds.period1);const mock=await installTimetableMock(page,{blocks:[block]});let writes=0;
 await page.route(`**/api/academic/timetable-blocks/${block.id}?*`,async route=>{if(route.request().method()!=='DELETE')return route.fallback();writes++;await route.abort('failed');});
 await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();
 await page.locator(`[data-block-id="${block.id}"]`).getByRole('button',{name:/นำ .* ออกจากตาราง/}).click();await page.getByRole('button',{name:'ยืนยันนำออก',exact:true}).click();
 await expect(page.getByText(/ยังยืนยันผลการบันทึกไม่ได้/)).toBeVisible();await expect(page.locator(`[data-block-id="${block.id}"]`)).toBeVisible();
 mock.changeDelivery();await page.getByRole('button',{name:'โหลดข้อมูลล่าสุดและละทิ้งรายการ',exact:true}).click();
 await expect(page.getByText(/ยังมีรายการที่ไม่ได้ยืนยันการบันทึก/)).toHaveCount(0);expect(writes).toBe(1);expect(mock.blocks()).toEqual([block]);
});

test('an ambiguous placement retains its exact target but never retries creation after reconnect',async({page})=>{
 const mock=await installTimetableMock(page);const bodies:Record<string,unknown>[]=[];
 await page.route('**/api/academic/timetable-blocks/ordinary',async route=>{bodies.push(route.request().postDataJSON());await route.abort('failed');});
 await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();await drop(page);
 await expect(page.getByText(/ยังยืนยันผลการบันทึกไม่ได้/)).toBeVisible();
 expect(bodies).toHaveLength(1);expect(bodies[0]).toMatchObject({timetableId:timetableIds.timetable,bellSchedulePeriodId:timetableIds.period2,dayOfWeek:'MON'});
 expect(mock.blocks()).toHaveLength(0);await expect(page.locator('[data-pending="true"]')).toHaveCount(0);
 mock.changeDelivery();await page.getByRole('button',{name:'โหลดข้อมูลล่าสุดและละทิ้งรายการ',exact:true}).click();await expect(page.getByText(/ยังมีรายการที่ไม่ได้ยืนยันการบันทึก/)).toHaveCount(0);expect(bodies).toHaveLength(1);
});

test('an ambiguous structural save keeps its exact text, targets and slots without another POST',async({page})=>{
 const mock=await installTimetableMock(page);const bodies:Record<string,unknown>[]=[];
 await page.route('**/api/academic/timetable-blocks/structural',async route=>{bodies.push(route.request().postDataJSON());await route.abort('failed');});
 await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();await page.getByRole('button',{name:'เพิ่มคาบพิเศษ',exact:true}).click();
 const dialog=page.getByRole('dialog',{name:'เพิ่มคาบพิเศษ'});await dialog.getByLabel('ชื่อที่แสดง').fill('คาบพิเศษที่ยังไม่ยืนยัน');await dialog.getByLabel('หมายเหตุ').fill('เก็บข้อความเดิม');
 await dialog.getByRole('group',{name:'เลือกครู',exact:true}).getByRole('button',{name:'ครูคณิตศาสตร์ A',exact:true}).click();
 await dialog.getByRole('button',{name:'เลือก',exact:true}).first().click();await dialog.getByRole('button',{name:'เพิ่ม 1 ช่อง',exact:true}).click();
 await expect(page.getByText(/ยังยืนยันผลการบันทึกไม่ได้/)).toBeVisible();expect(bodies).toHaveLength(1);
 expect(bodies[0]).toMatchObject({title:'คาบพิเศษที่ยังไม่ยืนยัน',note:'เก็บข้อความเดิม',teacherIds:[timetableIds.teacherA],slots:[{dayOfWeek:'MON',bellSchedulePeriodId:timetableIds.period1}]});
 expect(mock.blocks()).toHaveLength(0);expect(mock.retiredRequests()).toEqual([]);
});

test('a held claim reset before HTTP remains not-submitted and requires a deliberate fresh drop',async({page})=>{
 const mock=await installTimetableMock(page);await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();mock.relay.holdClaim();
 const target=await drop(page);await expect(page.getByLabel('กำลังบันทึกคาบ คณิตศาสตร์พื้นฐาน')).toBeVisible();
 await expect.poll(()=>mock.relay.received.filter(row=>row.message.type==='ClientClaim').length).toBe(1);mock.changeDelivery();
 await expect(page.getByText(/ยังไม่ได้ส่งรายการนี้ไปบันทึก/)).toBeVisible();expect(mock.createRequestCount()).toBe(0);expect(mock.previewRequestCount()).toBe(0);
 await page.getByRole('button',{name:'ทบทวนรายการ',exact:true}).click();await expect(target).toHaveAttribute('data-state','move');expect(mock.createRequestCount()).toBe(0);
 await target.click();await expect.poll(mock.createRequestCount).toBe(1);expect(mock.lastCreateBody()?.bellSchedulePeriodId).toBe(timetableIds.period2);expect(mock.retiredRequests()).toEqual([]);
});
