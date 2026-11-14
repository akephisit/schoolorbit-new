import {expect,test} from '@playwright/test';
import {installTimetableMock,makeTimetableBlock,timetableIds} from './timetable-test-harness';

test.use({serviceWorkers:'block'});
const url=()=>`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&view=homeroom&ownerId=${timetableIds.homeroom}`;
const header=(page:import('@playwright/test').Page)=>page.locator('header[aria-label="บริบทตารางสอน"]');

test('loads one current workspace and preserves every exact attached teacher',async({page})=>{
 const block=makeTimetableBlock(timetableIds.blockA,timetableIds.period1,{instructorIds:[timetableIds.teacherA,timetableIds.teacherB]});
 const mock=await installTimetableMock(page,{blocks:[block],eligibleInstructorIds:[timetableIds.teacherA]});
 await page.goto(url());
 await expect(header(page)).toContainText('ตารางสอนปัจจุบัน');
 await expect(page.getByText('ครูคณิตศาสตร์ +1')).toBeVisible();
 expect(mock.timetableRequestCount()).toBe(1);expect(mock.workspaceRequestCount()).toBe(1);
 await page.getByRole('button',{name:/ดูรายละเอียด ค21101/}).click();
 await expect(page.getByRole('dialog').getByText('ครูคณิตศาสตร์ B')).toBeVisible();
 expect(mock.retiredRequests()).toEqual([]);
});

test('read-only staff inspect and export the same current board without management controls',async({page})=>{
 const block=makeTimetableBlock(timetableIds.blockA,timetableIds.period1);
 const mock=await installTimetableMock(page,{permissions:['academic_timetable.read.school'],blocks:[block]});
 await page.goto(url());
 await expect(page.locator(`[data-block-id="${block.id}"]`)).toHaveAttribute('draggable','false');
 await expect(page.getByRole('button',{name:'แก้ไข',exact:true})).toHaveCount(0);
 await expect(page.getByRole('button',{name:'ดาวน์โหลด PDF',exact:true})).toBeEnabled();
 await page.getByRole('button',{name:/ดูรายละเอียด ค21101/}).click();
 await expect(page.getByRole('dialog').getByRole('button',{name:'บันทึก',exact:true})).toHaveCount(0);
 expect(mock.createRequestCount()+mock.updateRequestCount()+mock.deleteRequestCount()).toBe(0);
 expect(mock.retiredRequests()).toEqual([]);
});

test('Edit validates current data once and exposes placement only while editing',async({page})=>{
 const mock=await installTimetableMock(page,{blocks:[makeTimetableBlock(timetableIds.blockA,timetableIds.period1)]});
 await page.goto(url());
 await expect(header(page)).toBeVisible();
 await expect(page.locator('aside article')).toHaveCount(0);
 expect(mock.workspaceRequestCount()).toBe(1);
 await page.getByRole('button',{name:'แก้ไข',exact:true}).click();
 await expect(page.locator('aside article')).not.toHaveCount(0);
 await expect(page.locator('article[data-block-id]')).toHaveAttribute('draggable','true');
 expect(mock.workspaceRequestCount()).toBe(2);
 await page.getByRole('button',{name:'เสร็จสิ้น',exact:true}).click();
 await expect(page.locator('aside article')).toHaveCount(0);
 await expect(page.locator('article[data-block-id]')).toHaveAttribute('draggable','false');
 expect(mock.retiredRequests()).toEqual([]);
});

test('current Delivery demand refreshes under the same timetable without source commands',async({page})=>{
 const mock=await installTimetableMock(page,{requiredPeriods:3});
 await page.goto(url());await page.getByRole('button',{name:'แก้ไข',exact:true}).click();
 await expect(page.getByText('3/3',{exact:true})).toBeVisible();
 mock.changeDelivery({requiredPeriods:5});
 await expect(page.getByText('5/5',{exact:true})).toBeVisible();
 await expect(header(page)).toHaveAttribute('data-timetable-id',timetableIds.timetable);
 expect(mock.workspace().deliveryRowVersion).toBe(2);
 expect(mock.timetableCreateRequestCount()).toBe(0);expect(mock.retiredRequests()).toEqual([]);
});

test('an unused term remains empty on GET until the school manager explicitly creates its current table',async({page})=>{
 const mock=await installTimetableMock(page,{initiallyAbsent:true});
 await page.goto(url());
 await expect(page.getByText('ยังไม่มีตารางสอน',{exact:true})).toBeVisible();
 expect(mock.timetableCreateRequestCount()).toBe(0);expect(mock.workspaceRequestCount()).toBe(0);
 await page.getByRole('button',{name:'สร้างตารางสอน',exact:true}).click();
 await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
 expect(mock.timetableCreateRequestCount()).toBe(1);expect(mock.retiredRequests()).toEqual([]);
});

test('publication permission alone never grants direct management or singleton creation',async({page})=>{
 const mock=await installTimetableMock(page,{permissions:['academic_timetable.read.school','academic_timetable.publish.school']});
 await page.goto(url());await expect(header(page)).toBeVisible();
 await expect(page.getByRole('button',{name:'แก้ไข',exact:true})).toHaveCount(0);
 await expect(page.getByRole('button',{name:'สร้างตารางสอน',exact:true})).toHaveCount(0);
 expect(mock.timetableCreateRequestCount()).toBe(0);expect(mock.retiredRequests()).toEqual([]);
});

test('current context tools fit the desktop header without a retired version selector',async({page})=>{
 await page.setViewportSize({width:1440,height:1000});await installTimetableMock(page);await page.goto(url());
 const owner=header(page).getByRole('combobox',{name:'เลือกรายการสำหรับจัดตาราง',exact:true});
 await expect(owner).toBeVisible();
 expect((await owner.boundingBox())!.width).toBeLessThanOrEqual(240);
 await expect(page.getByRole('combobox',{name:'เลือกรุ่นตารางสอน',exact:true})).toHaveCount(0);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
});
