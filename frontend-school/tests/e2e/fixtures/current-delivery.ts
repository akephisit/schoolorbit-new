import {expect,type Page,type Route} from '@playwright/test';
import type {CreateLearningGroupRequest,DeleteDeliveryResourceRequest,DeliveryDeletionPreview,DeliveryManagementOptions,DeliveryWorkspace,LearningGroup,LearningGroupRosterTracking,LearningOffering,ReplaceLearningGroupTeachersRequest,UpdateLearningGroupRequest} from '../../../src/lib/api/learning-delivery.js';
import {mockDeliveryNavigation,navigationDelivery,navigationIds,navigationOfferingIds,navigationWorkspace} from './delivery-navigation-route-data';
import {sessionState} from './session-state';

export const currentDeliveryIds={...navigationIds,groupA:'a0000000-0000-4000-8000-000000000001',groupB:'a0000000-0000-4000-8000-000000000002',createdGroup:'a0000000-0000-4000-8000-000000000003',teacherB:'a0000000-0000-4000-8000-000000000004'};
export function deferred(){let release=()=>{};const promise=new Promise<void>(resolve=>release=resolve);return{promise,release};}
export async function deliveryReply(route:Route,data:unknown,status=200){await route.fulfill({status,json:status<400?{success:true,data}:{success:false,error:data}});}

export async function mockCurrentDelivery(page:Page,options:{permissions?:string[];kind?:'course'|'activity';registrationType?:'self'|'assigned';deleteConflict?:boolean;deleteGate?:Promise<void>;createGate?:Promise<void>;teacherGate?:Promise<void>;groupConflict?:boolean;groupReadGate?:Promise<void>}={}){
 const ids=currentDeliveryIds;
 await mockDeliveryNavigation(page,{manager:true});
 let currentSession=sessionState();let currentActor:string=ids.actor;let resourceManage=true;let permissions=options.permissions??['*'];
 const graph=navigationDelivery();
 const selectedId=options.kind==='activity'?navigationOfferingIds.club:navigationOfferingIds.basic21;
 const selected=graph.offerings.find(row=>row.id===selectedId)!;
 graph.offerings=[selected];selected.homeroomIds=[ids.roomA,ids.roomB];
 if(selected.catalog.kind==='activity')selected.catalog.registrationType=options.registrationType??'assigned';
 const stamp='2026-10-01T00:00:00Z';
 let groups:LearningGroup[]=[ids.groupA,ids.groupB].map((id,index)=>({id,learningOfferingId:selectedId,academicYearId:ids.year,academicTermId:ids.term,code:index?'B':'A',name:index?'กลุ่ม ข':'กลุ่ม ก',description:'ข้อมูลกลุ่มปัจจุบัน',capacity:40,registrationOpen:false,rowVersion:index?5:3,migrated:false,createdAt:stamp,updatedAt:stamp,homeroomIds:index?[ids.roomB]:[ids.roomA,ids.roomB],preferredRoomIds:[],teacherAssignments:[{id:`a0000000-0000-4000-8000-00000000010${index}`,teacherId:ids.actor,displayName:'ครูต้นทาง',role:'primary',rowVersion:1,isActive:true}]}));
 const tracking=new Map<string,LearningGroupRosterTracking>(groups.map((group,index)=>[group.id,{mode:index?'homeroom':'manual',effectiveFrom:index?'2026-10-01':null,groupRowVersion:group.rowVersion}]));
 const reads:URL[]=[];const writes:{path:string;method:string;body:Record<string,unknown>}[]=[];
 const previewReads:{groupId:string|null;epochSessionId:string}[]=[];
 let conflict=options.deleteConflict??false,groupConflict=options.groupConflict??false;
 function synchronize(){selected.groups=groups.map(group=>({id:group.id,code:group.code,name:group.name,description:group.description,capacity:group.capacity,rowVersion:group.rowVersion,registrationOpen:group.registrationOpen,homeroomIds:[...group.homeroomIds],preferredRoomIds:[...group.preferredRoomIds],teachers:group.teacherAssignments.map(row=>({assignmentId:row.id,teacherId:row.teacherId,displayName:row.displayName,role:row.role,rowVersion:row.rowVersion,isActive:row.isActive}))}));}
 synchronize();
 function resource():LearningOffering{return{canManage:resourceManage,id:selected.id,academicYearId:ids.year,academicTermId:ids.term,kind:selected.kind,codeSnapshot:selected.code,nameSnapshot:selected.name,owningOrganizationUnitId:selected.owningOrganizationUnitId,rowVersion:selected.rowVersion,migrated:false,createdAt:stamp,updatedAt:stamp,snapshot:selected.catalog,targets:selected.targets,weeklyPeriodTarget:selected.weeklyPeriodTarget};}
 function preview(groupId:string|null):DeliveryDeletionPreview{
  const scoped=groupId?groups.filter(group=>group.id===groupId):groups;
  return{academicYearId:ids.year,academicTermId:ids.term,learningOfferingId:selectedId,learningGroupId:groupId,rowVersion:groupId?scoped[0].rowVersion:selected.rowVersion,sourceDigest:graph.rowVersion.toString(16).padStart(64,'0'),impact:{learningGroups:scoped.length,teacherAssignments:scoped.reduce((sum,group)=>sum+group.teacherAssignments.length,0),memberships:2,timetableBlocks:2,examScheduleItems:1,assessmentScores:3,attendanceSessions:4,attendanceSummaries:0,officialResults:1,resultCorrections:1,termAggregates:1,annualAggregates:1,supervisionLinks:1,derivedBehaviorEvents:1,evidenceFiles:1}};
 }
 const management:DeliveryManagementOptions={academicYearId:ids.year,academicTermId:ids.term,studyPrograms:[navigationWorkspace().homerooms[0].studyProgram],gradeLevels:[navigationWorkspace().homerooms[0].gradeLevel],homerooms:navigationWorkspace().homerooms.map(row=>row.homeroom),learningGroups:groups,teachers:[{id:ids.actor,name:'ครูต้นทาง'},{id:ids.teacherB,name:'ครูคนใหม่'}],rooms:[],catalogVersions:[]};
 await page.route(url=>url.pathname.startsWith('/api/'),async route=>{
  const request=route.request(),url=new URL(request.url()),path=url.pathname,method=request.method();
  if(method==='GET')reads.push(url);else writes.push({path,method,body:request.postDataJSON()??{}});
  if(path==='/api/auth/me')return deliveryReply(route,{id:currentActor,username:'current-delivery-test',firstName:'ผู้จัดการ',lastName:'ทดสอบ',displayName:'ผู้จัดการ ทดสอบ',userType:'staff',status:'ACTIVE',session:currentSession,permissions});
  if(path==='/api/academic/delivery/workspace'){synchronize();return deliveryReply(route,graph satisfies DeliveryWorkspace);}
  if(path==='/api/academic/delivery/homerooms'){
   const workspace=navigationWorkspace();workspace.rowVersion=graph.rowVersion;
   for(const room of workspace.homerooms)for(const item of room.items){if(item.offeringId!==selectedId)continue;item.offeringState=graph.offerings.length?'present':'missing';item.offeringId=graph.offerings.length?selectedId:null;item.groups=groups.filter(group=>group.homeroomIds.includes(room.homeroom.id)).map(group=>({id:group.id,code:group.code,name:group.name,homeroomIds:group.homeroomIds,homeroomNames:group.homeroomIds.map(id=>id===ids.roomA?'ม.1/1':'ม.1/2'),primaryTeacherCount:group.teacherAssignments.filter(row=>row.role==='primary').length}));}
   return deliveryReply(route,workspace);
  }
  if(path==='/api/academic/delivery/management-options')return deliveryReply(route,management);
  const groupId=groups.find(group=>path.startsWith(`/api/academic/learning-groups/${group.id}`))?.id??null;
  if(path.endsWith('/deletion-preview')&&(path===`/api/academic/offerings/${selectedId}/deletion-preview`||groupId)){
   expect(url.searchParams.get('academicYearId')).toBe(ids.year);expect(url.searchParams.get('academicTermId')).toBe(ids.term);
   previewReads.push({groupId,epochSessionId:currentSession.sessionId});return deliveryReply(route,preview(groupId));
  }
  if(method==='DELETE'&&(path===`/api/academic/offerings/${selectedId}`||groupId&&path===`/api/academic/learning-groups/${groupId}`)){
   const body:DeleteDeliveryResourceRequest=request.postDataJSON();const accepted=preview(groupId);
   expect(body).toEqual({academicYearId:ids.year,academicTermId:ids.term,rowVersion:accepted.rowVersion,sourceDigest:accepted.sourceDigest});
   if(conflict){conflict=false;graph.rowVersion++;if(groupId)groups.find(row=>row.id===groupId)!.rowVersion++;else selected.rowVersion++;return deliveryReply(route,'ข้อมูลเปลี่ยนแล้ว กรุณาตรวจผลกระทบล่าสุด',409);}
   await options.deleteGate;
   if(groupId)groups=groups.filter(group=>group.id!==groupId);else{graph.offerings=[];groups=[];}
   graph.rowVersion++;synchronize();return deliveryReply(route,{academicYearId:ids.year,academicTermId:ids.term,learningOfferingId:selectedId,learningGroupId:groupId,deleted:true,deliveryRowVersion:graph.rowVersion});
  }
  if(path===`/api/academic/offerings/${selectedId}/weekly-period-target`&&method==='PATCH'){
   const body=request.postDataJSON();expect(body.rowVersion).toBe(selected.rowVersion);selected.rowVersion++;selected.weeklyPeriodTarget=body.weeklyPeriodTarget;graph.rowVersion++;return deliveryReply(route,resource());
  }
  if(path===`/api/academic/offerings/${selectedId}/groups`){
   if(method==='POST'){
    const body:CreateLearningGroupRequest=request.postDataJSON();await options.createGate;
    const created:LearningGroup={...groups[0],id:ids.createdGroup,code:'C',name:body.name,description:body.description??null,capacity:body.capacity??null,registrationOpen:body.registrationOpen??false,rowVersion:1,teacherAssignments:[],homeroomIds:[],preferredRoomIds:body.preferredRoomIds??[]};
    groups.push(created);graph.rowVersion++;synchronize();tracking.set(created.id,{mode:'manual',effectiveFrom:null,groupRowVersion:1});return deliveryReply(route,created);
   }return deliveryReply(route,groups);
  }
  if(path===`/api/academic/offerings/${selectedId}`)return graph.offerings.length?deliveryReply(route,resource()):deliveryReply(route,'ไม่พบรายการเปิดสอน',404);
  if(groupId){
   const group=groups.find(group=>group.id===groupId)!;
   if(path.endsWith('/memberships'))return deliveryReply(route,[]);
   if(path.endsWith('/roster-tracking'))return deliveryReply(route,tracking.get(groupId)??{mode:'manual',effectiveFrom:null,groupRowVersion:group.rowVersion});
   if(method==='PATCH'){
    const body:UpdateLearningGroupRequest=request.postDataJSON();
    if(groupConflict){groupConflict=false;group.rowVersion++;return deliveryReply(route,'ข้อมูลกลุ่มเปลี่ยนแล้ว กรุณาโหลดล่าสุด',409);}
    expect(body.rowVersion).toBe(group.rowVersion);Object.assign(group,body,{rowVersion:group.rowVersion+1});graph.rowVersion++;synchronize();return deliveryReply(route,group);
   }
   if(method==='PUT'&&path.endsWith('/teachers')){
    const body:ReplaceLearningGroupTeachersRequest=request.postDataJSON();await options.teacherGate;expect(body.rowVersion).toBe(group.rowVersion);
    group.teacherAssignments=body.teachers.map((teacher,index)=>({...teacher,id:group.teacherAssignments.find(old=>old.teacherId===teacher.teacherId)?.id??`a0000000-0000-4000-8000-00000000020${index}`,displayName:teacher.teacherId===ids.actor?'ครูต้นทาง':'ครูคนใหม่',rowVersion:1,isActive:true}));group.rowVersion++;graph.rowVersion++;synchronize();return deliveryReply(route,group);
   }
   if(method==='PUT'&&path.endsWith('/homerooms')){const body=request.postDataJSON();expect(body.rowVersion).toBe(group.rowVersion);group.homeroomIds=[...body.homeroomIds];group.rowVersion++;graph.rowVersion++;synchronize();return deliveryReply(route,group);}
   if(method==='GET'){await options.groupReadGate;return deliveryReply(route,group);}
  }
  return route.fallback();
 });
 const overview=`/staff/academic/delivery?academicYearId=${ids.year}&academicTermId=${ids.term}&view=offerings`;
 const detail=`/staff/academic/delivery/${selectedId}?academicYearId=${ids.year}&academicTermId=${ids.term}`;
 return{ids,reads,writes,previewReads,graph,resource,groups:()=>groups,tracking,management,overview,detail,session:()=>structuredClone(currentSession),setActor:(id:string)=>{currentActor=id;},setCanManage:(value:boolean)=>{resourceManage=value;},setSession:(next:ReturnType<typeof sessionState>)=>{currentSession=structuredClone(next);},setPermissions:(next:string[])=>{permissions=[...next];},count:(path:string)=>reads.filter(url=>url.pathname===path).length};
}

export async function replaceDeliverySession(page:Page,fixture:Pick<Awaited<ReturnType<typeof mockCurrentDelivery>>,'session'|'setSession'>){
 const next={...fixture.session(),sessionId:crypto.randomUUID(),serverNow:new Date(Date.now()+1000).toISOString()};fixture.setSession(next);
 const refreshed=page.waitForResponse(response=>new URL(response.url()).pathname==='/api/auth/me');
 await page.evaluate(session=>{const channel=new BroadcastChannel(`schoolorbit:session-activity:${location.origin}`);channel.postMessage({type:'observed',sessionId:session.sessionId,serverNow:session.serverNow});channel.close();},next);
 await(await refreshed).finished();return next;
}
