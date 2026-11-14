import {expect,test,type Route,type Page} from '@playwright/test';
import {installTimetableMock,makeTimetable,timetableIds} from './timetable-test-harness';
import type {AcademicContextOptionsResponse} from '../../src/lib/api/academic-context';
test.use({serviceWorkers:'block'});
const boardUrl=(term:string=timetableIds.term)=>`/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${term}`;
const templatesUrl=()=>`${boardUrl().replace('/timetable?','/timetable/templates?')}`;
async function fulfill(route:Route,data:unknown){await route.fulfill({json:{success:true,data}});}
const term=(id:string,sequence:number):AcademicContextOptionsResponse['terms'][number]=>({id,isVisible:true,academicYearId:timetableIds.year,name:`ภาคเรียนที่ ${sequence}`,code:String(sequence),sequence,termType:'regular',schoolDays:['MON','TUE','WED','THU','FRI'],status:'active',includedInYearResult:true,blocksYearClosure:true});
async function navigate(page:Page,href:string){await page.evaluate(href=>{const a=document.createElement('a');a.href=href;a.id='context-navigation';a.textContent='เปิดภาคเรียนใหม่';a.style.cssText='position:fixed;top:0;left:0;z-index:99999;background:white';document.body.append(a);},href);await page.locator('#context-navigation').click();}

test('current singleton metadata precedes one bounded board load and never loads operational history',async({page})=>{
 const mock=await installTimetableMock(page);const parents:Route[]=[],workspaces:Route[]=[];
 await page.route('**/api/academic/timetable?*',route=>{parents.push(route);});await page.route('**/api/academic/timetable-blocks/workspace?*',route=>{workspaces.push(route);});
 await page.goto(boardUrl());await expect.poll(()=>parents.length).toBe(1);expect(workspaces).toHaveLength(0);await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
 await parents[0].fallback();await expect.poll(()=>workspaces.length).toBe(1);await workspaces[0].fallback();await expect(page.getByTestId('timetable-board-ready')).toBeVisible();expect(mock.retiredRequests()).toEqual([]);
});

test('a stale retired pin never reads a different table in the explicit current term',async({page})=>{
 const mock=await installTimetableMock(page);const requested:string[]=[];await page.route('**/api/academic/timetable-blocks/workspace?*',async route=>{requested.push(new URL(route.request().url()).searchParams.get('timetableId')!);await route.fallback();});
 await page.goto(`${boardUrl()}&timetableId=a1000000-0000-4000-8000-000000000999`);await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
 expect(requested).toEqual([timetableIds.timetable]);expect(mock.timetableRequestCount()).toBe(1);expect(mock.retiredRequests()).toEqual([]);await expect(page).toHaveURL(new RegExp(`timetableId=${timetableIds.timetable}`));
});

test('template list paints independently from delayed current-table metadata and retries only that region',async({page})=>{
 await installTimetableMock(page);const parents:Route[]=[];let templates=0;
 await page.route('**/api/academic/timetable-templates',async route=>{templates++;await fulfill(route,[]);});await page.route('**/api/academic/timetable?*',route=>{parents.push(route);});
 await page.goto(templatesUrl());await expect.poll(()=>parents.length).toBe(1);await expect(page.getByText('ยังไม่มีแม่แบบ',{exact:true})).toBeVisible();expect(templates).toBe(1);
 await parents[0].fulfill({status:503,json:{success:false,error:'ตารางไม่พร้อม'}});await expect(page.getByText('โหลดตารางสอนไม่สำเร็จ',{exact:true})).toBeVisible();
 await page.getByRole('button',{name:'ลองอีกครั้ง',exact:true}).click();await expect.poll(()=>parents.length).toBe(2);await parents[1].fallback();await expect(page.getByRole('button',{name:'สร้างจากภาคนี้',exact:true}).first()).toBeEnabled();expect(templates).toBe(1);
});

test('template current-table region stays usable while the independent template list is pending',async({page})=>{
 const mock=await installTimetableMock(page);const templates:Route[]=[];await page.route('**/api/academic/timetable-templates',route=>{templates.push(route);});
 await page.goto(templatesUrl());await expect.poll(()=>templates.length).toBe(1);await expect(page.getByRole('button',{name:'สร้างจากภาคนี้',exact:true}).first()).toBeEnabled();expect(mock.timetableRequestCount()).toBe(1);
 await fulfill(templates[0],[]);await expect(page.getByText('ยังไม่มีแม่แบบ',{exact:true})).toBeVisible();expect(mock.workspaceRequestCount()).toBe(0);
});

test('a new term clears the old board before the next current workspace arrives',async({page})=>{
 const nextTerm='21000000-0000-4000-8000-000000000299',nextTable='a1000000-0000-4000-8000-000000000299';
 await installTimetableMock(page,{timetables:[makeTimetable(),makeTimetable({id:nextTable,academicTermId:nextTerm})],terms:[term(timetableIds.term,1),term(nextTerm,2)]});
 const workspaces:Route[]=[];await page.route('**/api/academic/timetable-blocks/workspace?*',route=>{workspaces.push(route);});
 await page.goto(boardUrl());await expect.poll(()=>workspaces.length).toBe(1);await workspaces[0].fallback();await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
 await navigate(page,boardUrl(nextTerm));await expect.poll(()=>workspaces.length).toBe(2);await expect(page.getByTestId('timetable-board-ready')).toHaveCount(0);await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
 await workspaces[1].fallback();await expect(page.locator('header[aria-label="บริบทตารางสอน"]')).toHaveAttribute('data-timetable-id',nextTable);
});

test('an empty current term can browse templates without creating a table or loading block data',async({page})=>{
 const mock=await installTimetableMock(page,{initiallyAbsent:true});await page.route('**/api/academic/timetable-templates',route=>fulfill(route,[]));
 await page.goto(templatesUrl());await expect(page.getByText('ยังไม่มีตารางสอน',{exact:true})).toBeVisible();await expect(page.getByText('ยังไม่มีแม่แบบ',{exact:true})).toBeVisible();
 expect(mock.workspaceRequestCount()).toBe(0);expect(mock.timetableCreateRequestCount()).toBe(0);expect(mock.retiredRequests()).toEqual([]);
});
