//! Leave owns selected plans and exact-plan consent. Timetable owns dated occupancy.
use super::{coverage_models::*, models::*, policy::*, repository::*};
use chrono::{NaiveDate,NaiveTime,Timelike,Utc};
use school_academic_timetable::services::dated_teaching as duties;
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_workflow::process;
use sqlx::{types::Json,PgConnection,PgPool,Postgres,Transaction};
use std::collections::{BTreeMap,BTreeSet};
use uuid::Uuid;

fn end_of_day()->Result<NaiveTime,AppError>{NaiveTime::from_hms_nano_opt(23,59,59,999_999_999).ok_or_else(||invalid("เวลาไม่ถูกต้อง"))}
fn own_access(actor:&ActorContext)->Result<(),AppError>{actor.require_permission(codes::STAFF_LEAVE_CREATE_OWN)?;actor.require_any_permission(&[codes::STAFF_LEAVE_READ_OWN,codes::STAFF_LEAVE_READ_SCHOOL])}
fn settings_access(actor:&ActorContext)->Result<(),AppError>{actor.require_any_permission(&[codes::STAFF_LEAVE_CREATE_OWN,codes::STAFF_LEAVE_MANAGE_SCHOOL])}
pub(super) async fn lock_request_context(tx:&mut Transaction<'_,Postgres>,id:Uuid)->Result<(),AppError>{
 let context:Option<(Option<Uuid>,Option<Uuid>)>=DbSession::new(&mut **tx).query_as("staff.leave.coverage.lock_context","SELECT (document->>'academicYearId')::uuid,(document->>'academicTermId')::uuid FROM staff_leave_requests WHERE id=$1").bind(id).fetch_optional().await?;
 if let Some((Some(year),Some(term)))=context {duties::lock_context(tx,year,term).await?;} Ok(())
}
pub(super) fn explicit_context(period:&LeavePeriodInput)->Result<(Uuid,Uuid),AppError>{match(period.academic_year_id,period.academic_term_id){(Some(y),Some(t))=>Ok((y,t)),_=>Err(invalid("กรุณาเลือกปีการศึกษาและภาคเรียนอย่างชัดเจน"))}}
pub async fn settings(pool:&PgPool,actor:&ActorContext)->Result<LeaveCoverageSettings,AppError>{settings_access(actor)?;let mut connection=school_tenancy::TenantPool::observed_checkout_pool(pool,"staff.leave.coverage.settings").await?;settings_on_connection(&mut connection).await}
async fn settings_on_connection(connection:&mut PgConnection)->Result<LeaveCoverageSettings,AppError>{
 let(revision,half_day_boundary):(i64,Option<NaiveTime>)=DbSession::new(connection).query_as("staff.leave.coverage.settings.read","SELECT revision,half_day_boundary FROM staff_leave_coverage_settings WHERE singleton").fetch_one().await?;
 let options=school_academic_core::services::context::list_options_on_connection(connection).await?;
 let contexts=options.terms.iter().filter_map(|t|options.years.iter().find(|y|y.id==t.academic_year_id).map(|y|LeaveCoverageContext{academic_year_id:y.id,academic_year_name:y.name.clone(),academic_term_id:t.id,academic_term_name:t.name.clone()})).collect::<Vec<_>>();
 let valid_default=contexts.iter().any(|c|Some(c.academic_year_id)==options.default_academic_year_id && Some(c.academic_term_id)==options.default_academic_term_id);
 Ok(LeaveCoverageSettings{revision,half_day_boundary,default_academic_year_id:if valid_default{options.default_academic_year_id}else{None},default_academic_term_id:if valid_default{options.default_academic_term_id}else{None},contexts})
}
pub async fn save_settings(pool:&PgPool,actor:&ActorContext,input:SaveLeaveCoverageSettingsRequest)->Result<LeaveCoverageSettings,AppError>{
 actor.require_permission(codes::STAFF_LEAVE_MANAGE_SCHOOL)?;
 if input.half_day_boundary.is_some_and(|t|t.second()!=0||t.nanosecond()!=0||t==NaiveTime::MIN){return Err(invalid("เวลาแบ่งครึ่งวันต้องเป็นนาทีและอยู่ระหว่างวัน"))}
 let mut tx=school_tenancy::TenantPool::begin_observed(pool,"staff.leave.coverage.save_settings").await?;
 if claim(&mut tx,actor.user_id,input.request_id,"coverage_settings",Uuid::nil(),&input).await?.is_none(){
 let changed=DbSession::new(&mut *tx).query("staff.leave.coverage.settings.update","UPDATE staff_leave_coverage_settings SET revision=revision+1,half_day_boundary=$1 WHERE singleton AND revision=$2").bind(input.half_day_boundary).bind(input.expected_revision).execute().await?;
 if changed.rows_affected()!=1{return Err(stale())}}
 let result=settings_on_connection(&mut tx).await?;tx.commit().await?;Ok(result)
}
pub(super) async fn interval(connection:&mut PgConnection,period:&LeavePeriodInput,verify_setting:bool)->Result<(NaiveTime,NaiveTime),AppError>{
 if let(Some(start),Some(end))=(period.start_time,period.end_time){if start>=end{return Err(invalid("เวลาเริ่มต้องก่อนเวลาสิ้นสุด"))}return Ok((start,end))}
 if period.day_portion==LeaveDayPortion::FullDay{return Ok((NaiveTime::MIN,end_of_day()?))}
 let boundary=period.half_day_boundary.ok_or_else(||invalid("โรงเรียนต้องตั้งค่าเวลาแบ่งครึ่งวันก่อนจัดสอนแทน"))?;
 if verify_setting {let configured:Option<NaiveTime>=DbSession::new(connection).scalar("staff.leave.coverage.boundary","SELECT half_day_boundary FROM staff_leave_coverage_settings WHERE singleton FOR SHARE").fetch_one().await?;if configured!=Some(boundary){return Err(invalid("เวลาแบ่งครึ่งวันเปลี่ยนแล้ว กรุณาใช้ค่าตั้งโรงเรียนล่าสุด"))}}
 Ok(if period.day_portion==LeaveDayPortion::Morning{(NaiveTime::MIN,boundary)}else{(boundary,end_of_day()?)})
}
fn empty_plan(period:&LeavePeriodInput,state:LeaveCoverageState)->LeaveCoveragePlan{LeaveCoveragePlan{revision:0,plan_hash:String::new(),state,source_digest:None,academic_year_id:period.academic_year_id,academic_term_id:period.academic_term_id,half_day_boundary:period.half_day_boundary,timetable_state:LeaveCoverageTimetableState::Legacy,days:vec![],can_manage:false,blocking_reason:None,emergency_reason:None}}
pub(super) async fn preview_on_connection(connection:&mut PgConnection,owner:Uuid,period:&LeavePeriodInput)->Result<LeaveCoveragePlan,AppError>{
 let(year,term)=explicit_context(period)?;let(start,end)=interval(connection,period,true).await?;
 let snapshot=duties::snapshot_on_connection(connection,year,term,owner,period.start_date,period.end_date,start,end).await?;
 let mut plan=empty_plan(period,LeaveCoverageState::Incomplete);plan.source_digest=Some(snapshot.source_digest);plan.timetable_state=if snapshot.timetable_state=="ready"{LeaveCoverageTimetableState::Ready}else{LeaveCoverageTimetableState::Missing};
 let mut days=BTreeMap::<NaiveDate,Vec<LeaveCoverageLesson>>::new();
 for n in 0..=(period.end_date-period.start_date).num_days(){days.insert(period.start_date+chrono::Duration::days(n),vec![]);}
 for lesson in snapshot.lessons {days.entry(lesson.date).or_default().push(LeaveCoverageLesson{lesson,assignment:None});}
 plan.days=days.into_iter().map(|(date,lessons)|LeaveCoverageDay{date,lessons}).collect();Ok(plan)
}
pub async fn preview(pool:&PgPool,actor:&ActorContext,input:PreviewLeaveCoverageRequest)->Result<LeaveCoveragePlan,AppError>{
 own_access(actor)?;let(year,term)=explicit_context(&input.period)?;let mut tx=school_tenancy::TenantPool::begin_observed(pool,"staff.leave.coverage.preview").await?;duties::lock_context(&mut tx,year,term).await?;staff_name(&mut tx,actor.user_id).await?;let result=preview_on_connection(&mut tx,actor.user_id,&input.period).await?;tx.commit().await?;Ok(result)
}
pub(super) async fn stored_plan(connection:&mut PgConnection,id:Uuid)->Result<Option<LeaveCoveragePlan>,AppError>{Ok(DbSession::new(connection).scalar::<Json<LeaveCoveragePlan>>("staff.leave.coverage.plan","SELECT document FROM staff_leave_coverage_plans WHERE request_id=$1").bind(id).fetch_optional().await?.map(|p|p.0))}
fn complete(plan:&LeaveCoveragePlan)->bool{plan.timetable_state==LeaveCoverageTimetableState::Ready&&plan.days.iter().flat_map(|d|&d.lessons).all(|l|l.assignment.is_some())}
pub(super) async fn detail_on_connection(connection:&mut PgConnection,actor:&ActorContext,r:&Record)->Result<LeaveCoveragePlan,AppError>{
 let Some(mut plan)=stored_plan(connection,r.document.id).await? else{
 if r.document.academic_term_id.is_none(){return Ok(empty_plan(&period_from(r),LeaveCoverageState::LegacyUnverified))}
 if matches!(r.document.status,LeaveStatus::Draft|LeaveStatus::Returned){return match preview_on_connection(connection,r.document.owner_id,&period_from(r)).await{Ok(plan)=>Ok(plan),Err(AppError::ValidationError(message))=>{let mut plan=empty_plan(&period_from(r),LeaveCoverageState::Incomplete);plan.blocking_reason=Some(message);Ok(plan)},Err(error)=>Err(error)}}
 return Ok(empty_plan(&period_from(r),LeaveCoverageState::Unplanned))};
 let confirmations:Vec<(Uuid,Uuid,String,chrono::DateTime<Utc>)>=DbSession::new(connection).query_as("staff.leave.coverage.consents","SELECT assignment_id,teacher_id,decision,accepted_at FROM staff_leave_coverage_confirmations WHERE request_id=$1 AND plan_revision=$2 AND plan_hash=$3").bind(r.document.id).bind(plan.revision).bind(&plan.plan_hash).fetch_all().await?;
 for assignment in plan.days.iter_mut().flat_map(|d|&mut d.lessons).filter_map(|l|l.assignment.as_mut()) {if let Some((_,_,decision,at))=confirmations.iter().find(|(id,t,_,_)|*id==assignment.id&&*t==assignment.replacement_teacher_id){assignment.consent=if decision=="confirmed"{LeaveCoverageConsent::Confirmed}else{LeaveCoverageConsent::Declined};assignment.confirmed_at=Some(*at);}}
 plan.state=if !complete(&plan){if plan.emergency_reason.is_some(){LeaveCoverageState::Emergency}else{LeaveCoverageState::Incomplete}}else if plan.days.iter().flat_map(|d|&d.lessons).filter_map(|l|l.assignment.as_ref()).all(|a|a.consent==LeaveCoverageConsent::Confirmed){LeaveCoverageState::Confirmed}else{LeaveCoverageState::AwaitingConfirmation};
 plan.can_manage=actor.has_permission(codes::STAFF_LEAVE_MANAGE_SCHOOL)&&r.document.status==LeaveStatus::Submitted&&plan.emergency_reason.is_some()&&r.instance_id.is_none();Ok(plan)
}
pub(super) async fn approval_ready(connection:&mut PgConnection,r:&Record)->Result<bool,AppError>{
 if r.document.academic_term_id.is_none(){return Ok(true)} // Explicit grandfathering: never invent or rehash legacy coverage.
 let actor=ActorContext{user_id:r.document.owner_id,permissions:vec![]};let plan=detail_on_connection(connection,&actor,r).await?;Ok(plan.state==LeaveCoverageState::Confirmed)
}
async fn editable_plan(tx:&mut Transaction<'_,Postgres>,actor:&ActorContext,id:Uuid,revision:i64)->Result<Record,AppError>{
 let r=load(tx,id).await?;staff_name(tx,actor.user_id).await?;
 let manager=actor.has_permission(codes::STAFF_LEAVE_MANAGE_SCHOOL)&&r.document.status==LeaveStatus::Submitted&&r.instance_id.is_none()&&stored_plan(&mut **tx,id).await?.is_some_and(|p|p.emergency_reason.is_some());
 if !manager {owner_edit(actor,&r)?;}if r.document.revision!=revision{return Err(stale())}Ok(r)
}
async fn current_plan(tx:&mut Transaction<'_,Postgres>,actor:&ActorContext,id:Uuid,revision:i64,digest:&str)->Result<(Record,LeaveCoveragePlan),AppError>{
 let r=editable_plan(tx,actor,id,revision).await?;let plan=preview_on_connection(&mut **tx,r.document.owner_id,&period_from(&r)).await?;if plan.source_digest.as_deref()!=Some(digest){return Err(stale())}Ok((r,plan))
}
async fn candidate_metadata(connection:&mut PgConnection,owner:Uuid,year:Uuid,term:Uuid,search:Option<&str>)->Result<(Vec<LeaveCoverageCandidate>,Vec<LeaveCoverageLabel>,Vec<LeaveCoverageLabel>),AppError>{
 let staff=school_staff::services::coverage_candidates_on_connection(connection,search).await?;
 let mut ids=staff.iter().map(|s|s.0).collect::<Vec<_>>();ids.push(owner);let groups=school_staff::services::subject_groups_for_users_on_connection(connection,&ids).await?;
 let owner_groups=groups.iter().filter(|g|g.user_id==owner).map(|g|g.subject_group_id).collect::<BTreeSet<_>>();
 let source=school_academic_delivery::services::current::complete_workspace_on_connection(connection,year,term).await?;
 let grades:Vec<(Uuid,String)>=DbSession::new(connection).query_as("staff.leave.coverage.grade_names","SELECT id,concat(level_type,' ',year) FROM grade_levels ORDER BY level_type,year,id").fetch_all().await?;
 let mut candidates=vec![];let mut all_groups=BTreeMap::new();let mut all_grades=BTreeMap::new();
 for(teacher_id,display_name)in staff {if teacher_id==owner{continue}let subject_groups=groups.iter().filter(|g|g.user_id==teacher_id).map(|g|{all_groups.insert(g.subject_group_id,g.name.clone());LeaveCoverageLabel{id:g.subject_group_id,name:g.name.clone()}}).collect::<Vec<_>>();
 let taught=source.offerings.iter().filter(|o|o.groups.iter().any(|g|g.teachers.iter().any(|t|t.teacher_id==teacher_id))).collect::<Vec<_>>();
 let actual_grades=taught.iter().flat_map(|o|o.targets.iter().filter(|target|target.homeroom_id.is_none()||o.groups.iter().any(|g|g.teachers.iter().any(|t|t.teacher_id==teacher_id)&&target.homeroom_id.is_some_and(|id|g.homeroom_ids.contains(&id)))).map(|t|t.grade_level_id)).collect::<BTreeSet<_>>();
 let grade_levels=grades.iter().filter(|(id,_)|actual_grades.contains(id)).map(|(id,name)|{all_grades.insert(*id,name.clone());LeaveCoverageLabel{id:*id,name:name.clone()}}).collect();
 candidates.push(LeaveCoverageCandidate{teacher_id,display_name,is_same_subject_group:subject_groups.iter().any(|g|owner_groups.contains(&g.id)),subject_groups,grade_levels,taught_offerings:taught.iter().map(|o|LeaveCoverageLabel{id:o.id,name:o.name.clone()}).collect()});}
 Ok((candidates,all_groups.into_iter().map(|(id,name)|LeaveCoverageLabel{id,name}).collect(),all_grades.into_iter().map(|(id,name)|LeaveCoverageLabel{id,name}).collect()))
}
pub async fn candidates(pool:&PgPool,actor:&ActorContext,id:Uuid,input:LeaveCoverageCandidatesRequest)->Result<LeaveCoverageCandidates,AppError>{
 settings_access(actor)?;if input.lesson_keys.is_empty()||input.lesson_keys.len()>500||input.search.as_ref().is_some_and(|s|s.chars().count()>100){return Err(invalid("กรุณาเลือกคาบสอนและคำค้นไม่เกิน 100 ตัวอักษร"))}
 let mut tx=school_tenancy::TenantPool::begin_observed(pool,"staff.leave.coverage.candidates").await?;lock_request_context(&mut tx,id).await?;let(r,plan)=current_plan(&mut tx,actor,id,input.expected_revision,&input.source_digest).await?;
 let selected=plan.days.iter().flat_map(|d|&d.lessons).filter(|l|input.lesson_keys.contains(&l.lesson.lesson_key)).map(|l|&l.lesson).collect::<Vec<_>>();if selected.len()!=input.lesson_keys.iter().collect::<BTreeSet<_>>().len(){return Err(stale())}
 let(year,term)=explicit_context(&period_from(&r))?;let(people,subject_groups,grade_levels)=candidate_metadata(&mut tx,r.document.owner_id,year,term,input.search.as_deref()).await?;
 let actual_grades=selected.iter().flat_map(|l|l.grade_level_ids.iter().copied()).collect::<BTreeSet<_>>();
 let source=school_academic_delivery::services::current::complete_workspace_on_connection(&mut tx,year,term).await?;
 let group_ids=source.offerings.iter().flat_map(|o|o.groups.iter().map(|g|g.id)).collect::<Vec<_>>();
 let memberships=if input.same_student_group_only{school_academic_delivery::services::attendance_roster::membership_spans(&mut tx,&group_ids,r.document.start_date,r.document.end_date).await?}else{vec![]};
 let audience=|group:Uuid,date:NaiveDate|memberships.iter().filter(|m|m.learning_group_id==group&&m.joined_at<=date&&m.left_at.is_none_or(|left|left>=date)).map(|m|m.student_id).collect::<BTreeSet<_>>();
 let mut result=vec![];for candidate in people {
 if input.same_grade_only&&(actual_grades.is_empty()||actual_grades.iter().any(|id|!candidate.grade_levels.iter().any(|g|g.id==*id))){continue}
 if input.same_student_group_only {let teacher_groups=source.offerings.iter().flat_map(|o|&o.groups).filter(|g|g.teachers.iter().any(|t|t.teacher_id==candidate.teacher_id)).map(|g|g.id).collect::<Vec<_>>();if selected.iter().any(|lesson|{let original=lesson.learning_group_id.map(|g|audience(g,lesson.date)).unwrap_or_default();original.is_empty()||!teacher_groups.iter().any(|g|audience(*g,lesson.date)==original)}){continue}}
if input.subject_group_id.is_some_and(|id|!candidate.subject_groups.iter().any(|g|g.id==id))||input.grade_level_id.is_some_and(|id|!candidate.grade_levels.iter().any(|g|g.id==id))||(input.same_subject_group_only&&!candidate.is_same_subject_group){continue}let mut free=true;for l in &selected {if !duties::teacher_free_on_connection(&mut tx,candidate.teacher_id,l.date,l.start_time,l.end_time,id,&[]).await?{free=false;break}}if free{result.push(candidate)}}let has_more=result.len()>200;result.truncate(200);tx.commit().await?;Ok(LeaveCoverageCandidates{candidates:result,subject_groups,grade_levels,has_more})
}
async fn return_options(connection:&mut PgConnection,r:&Record,lesson:&DatedTeachingLesson,teacher:Uuid,date:NaiveDate)->Result<Vec<DatedTeachingLesson>,AppError>{
 let period=period_from(r);let(year,term)=explicit_context(&period)?;
 if lesson.learning_group_id.is_none()||date>=period.start_date&&date<=period.end_date||(date-lesson.date).num_days().abs()>366{return Ok(vec![])}
 let snapshot=duties::snapshot_on_connection(connection,year,term,teacher,date,date,NaiveTime::MIN,end_of_day()?).await?;let mut result=vec![];
 for back in snapshot.lessons {if back.lesson_key==lesson.lesson_key{continue}let same=match(lesson.learning_group_id,back.learning_group_id){(Some(a),Some(b))=>school_academic_delivery::services::attendance_roster::same_audience_on_connection(connection,a,lesson.date,b,date).await?,_=>false};if !same{continue}if duties::teacher_free_on_connection(connection,r.document.owner_id,date,back.start_time,back.end_time,r.document.id,&[]).await?&&duties::teacher_free_on_connection(connection,teacher,lesson.date,lesson.start_time,lesson.end_time,r.document.id,&[]).await?{result.push(back)}}Ok(result)
}
pub async fn return_lessons(pool:&PgPool,actor:&ActorContext,id:Uuid,input:LeaveCoverageReturnLessonsRequest)->Result<LeaveCoverageReturnLessons,AppError>{settings_access(actor)?;let mut tx=school_tenancy::TenantPool::begin_observed(pool,"staff.leave.coverage.return_lessons").await?;lock_request_context(&mut tx,id).await?;let(r,plan)=current_plan(&mut tx,actor,id,input.expected_revision,&input.source_digest).await?;let lesson=plan.days.iter().flat_map(|d|&d.lessons).find(|l|l.lesson.lesson_key==input.lesson_key).ok_or_else(stale)?;let lessons=return_options(&mut tx,&r,&lesson.lesson,input.replacement_teacher_id,input.date).await?;tx.commit().await?;Ok(LeaveCoverageReturnLessons{lessons})}
async fn persist_plan(connection:&mut PgConnection,id:Uuid,plan:&mut LeaveCoveragePlan)->Result<(),AppError>{
 plan.can_manage=false;plan.plan_hash.clear();plan.plan_hash=process::payload_hash(plan)?;
 let(year,term)=match(plan.academic_year_id,plan.academic_term_id){(Some(y),Some(t))=>(y,t),_=>return Err(invalid("ไม่พบปีและภาคเรียนของแผน"))};
 DbSession::new(connection).query("staff.leave.coverage.save_revision","INSERT INTO staff_leave_coverage_plan_revisions(request_id,revision,document,plan_hash) VALUES($1,$2,$3,$4)").bind(id).bind(plan.revision).bind(Json(&*plan)).bind(&plan.plan_hash).execute().await?;
 DbSession::new(connection).query("staff.leave.coverage.save_plan","INSERT INTO staff_leave_coverage_plans(request_id,revision,academic_year_id,academic_term_id,document,plan_hash,emergency) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(request_id) DO UPDATE SET revision=excluded.revision,academic_year_id=excluded.academic_year_id,academic_term_id=excluded.academic_term_id,document=excluded.document,plan_hash=excluded.plan_hash,emergency=excluded.emergency").bind(id).bind(plan.revision).bind(year).bind(term).bind(Json(&*plan)).bind(&plan.plan_hash).bind(plan.emergency_reason.is_some()).execute().await?;Ok(())
}
pub async fn save(pool:&PgPool,actor:&ActorContext,id:Uuid,input:SaveLeaveCoverageRequest)->Result<LeaveRequestDetail,AppError>{
 settings_access(actor)?;if input.assignments.len()>3000{return Err(invalid("แผนสอนมีจำนวนคาบเกินขนาด"))}let mut tx=school_tenancy::TenantPool::begin_observed(pool,"staff.leave.coverage.save").await?;lock_request_context(&mut tx,id).await?;
 if claim(&mut tx,actor.user_id,input.request_id,"coverage_save",id,&(id,&input)).await?.is_some(){tx.commit().await?;return super::queries::detail(pool,actor,id).await}
 let(mut r,mut plan)=current_plan(&mut tx,actor,id,input.expected_revision,&input.source_digest).await?;let previous=stored_plan(&mut tx,id).await?;plan.revision=previous.as_ref().map_or(1,|p|p.revision+1);plan.emergency_reason=previous.and_then(|p|p.emergency_reason);
 let(year,term)=explicit_context(&period_from(&r))?;let(people,_,_)=candidate_metadata(&mut tx,r.document.owner_id,year,term,None).await?;let mut keys=BTreeSet::new();
 for selection in input.assignments {if !keys.insert(selection.lesson_key.clone()){return Err(invalid("คาบสอนซ้ำในแผน"))}let lesson=plan.days.iter().flat_map(|d|&d.lessons).find(|l|l.lesson.lesson_key==selection.lesson_key).ok_or_else(stale)?.lesson.clone();let teacher=people.iter().find(|c|c.teacher_id==selection.replacement_teacher_id).ok_or_else(||invalid("ผู้สอนไม่พร้อมใช้งาน"))?;
 if selection.taught_offering_id.is_some()&&selection.taught_activity.is_some()||selection.taught_activity.as_ref().is_some_and(|s|s.trim().is_empty()||s.chars().count()>200){return Err(invalid("กรุณาเลือกเนื้อหาสอนจริงอย่างใดอย่างหนึ่ง"))}
 let mut taught_offering_id=selection.taught_offering_id;let mut taught_activity=selection.taught_activity;
 if taught_offering_id.is_none()&&taught_activity.is_none(){taught_offering_id=lesson.offering_id;if taught_offering_id.is_none(){taught_activity=Some(lesson.offering_name.clone().unwrap_or_else(||"กิจกรรมตามตาราง".into()))}}
 let taught_offering_name=if let Some(offering)=taught_offering_id{if Some(offering)==lesson.offering_id{lesson.offering_name.clone()}else{Some(teacher.taught_offerings.iter().find(|o|o.id==offering).ok_or_else(||invalid("วิชาที่สอนจริงต้องเป็นวิชาของผู้สอนในภาคเรียนนี้"))?.name.clone())}}else{None};
 let back=if selection.mode==LeaveCoverageMode::Reciprocal{let key=selection.return_lesson_key.as_ref().ok_or_else(||invalid("กรุณาเลือกคาบสอนคืน"))?;let date=key.get(..10).and_then(|d|NaiveDate::parse_from_str(d,"%Y-%m-%d").ok()).ok_or_else(||invalid("วันสอนคืนไม่ถูกต้อง"))?;Some(return_options(&mut tx,&r,&lesson,teacher.teacher_id,date).await?.into_iter().find(|l|&l.lesson_key==key).ok_or_else(||invalid("คาบสอนคืนไม่ว่างหรือไม่ใช่กลุ่มผู้เรียนเดียวกัน"))?)}else{if selection.return_lesson_key.is_some(){return Err(invalid("สอนแทนปกติไม่ใช้คาบสอนคืน"))}None};
 if !duties::teacher_free_on_connection(&mut tx,teacher.teacher_id,lesson.date,lesson.start_time,lesson.end_time,id,&[]).await?{return Err(invalid("ผู้สอนที่เลือกไม่ว่างในคาบนี้"))}
 let (taught_offering_id,taught_offering_name,taught_learning_group_id,taught_activity)=if let Some(return_lesson)=&back {
 (return_lesson.offering_id,return_lesson.offering_name.clone(),return_lesson.learning_group_id,if return_lesson.offering_id.is_none(){return_lesson.offering_name.clone()}else{None})
 }else if let Some(offering_id)=taught_offering_id {
 if Some(offering_id)==lesson.offering_id {(Some(offering_id),taught_offering_name,lesson.learning_group_id,None)}else{
 let source=school_academic_delivery::services::current::complete_workspace_on_connection(&mut tx,year,term).await?;let offering=source.offerings.iter().find(|o|o.id==offering_id).ok_or_else(stale)?;let mut compatible=None;
 for group in offering.groups.iter().filter(|g|g.teachers.iter().any(|t|t.teacher_id==teacher.teacher_id)){if let Some(original)=lesson.learning_group_id{if school_academic_delivery::services::attendance_roster::same_audience_on_connection(&mut tx,original,lesson.date,group.id,lesson.date).await?{compatible=Some(group.id);break}}}
 (Some(offering_id),taught_offering_name,Some(compatible.ok_or_else(||invalid("วิชาของผู้สอนต้องมีกลุ่มนักเรียนเดียวกัน เลือกกิจกรรมเสริมแทนเมื่อไม่มีกลุ่มที่ตรงกัน"))?),None)
 }}else{(None,None,lesson.learning_group_id,taught_activity)};
 let assignment=LeaveCoverageAssignment{id:Uuid::now_v7(),replacement_teacher_id:teacher.teacher_id,replacement_teacher_name:teacher.display_name.clone(),mode:selection.mode,taught_offering_id,taught_offering_name,taught_learning_group_id,taught_activity,return_lesson_key:back.as_ref().map(|l|l.lesson_key.clone()),return_date:back.as_ref().map(|l|l.date),consent:LeaveCoverageConsent::Pending,confirmed_at:None,return_lesson:back};
 if let Some(target)=plan.days.iter_mut().flat_map(|d|&mut d.lessons).find(|l|l.lesson.lesson_key==selection.lesson_key){target.assignment=Some(assignment)}
 }
 plan.state=if complete(&plan){LeaveCoverageState::AwaitingConfirmation}else{LeaveCoverageState::Incomplete};persist_plan(&mut tx,id,&mut plan).await?;
 r.document.revision+=1;r.content_hash=process::payload_hash(&(&r.document,&r.evidence_file_ids,&plan.plan_hash))?;store(&mut tx,&r).await?;history(&mut tx,actor,&r,"coverage_planned",None,None,None).await?;
 if r.document.status==LeaveStatus::Submitted {if !complete(&plan){return Err(invalid("ผู้จัดการต้องเลือกผู้สอนให้ครบก่อนส่งเข้ากระบวนงาน"))}reserve(&mut tx,&r,&plan).await?;let p=pinned_policy(&mut tx,r.policy_id).await?.ok_or_else(||invalid("ไม่พบเกณฑ์คำขอ"))?;super::lifecycle::start(&mut tx,actor,&mut r,&p,input.request_id,LeaveProcessPurpose::Request).await?;DbSession::new(&mut *tx).query("staff.leave.coverage.bind_process","UPDATE staff_leave_requests SET instance_id=$2 WHERE id=$1").bind(id).bind(r.instance_id).execute().await?;}
 tx.commit().await?;super::queries::detail(pool,actor,id).await
}
pub(super) async fn reserve(tx:&mut Transaction<'_,Postgres>,r:&Record,plan:&LeaveCoveragePlan)->Result<(),AppError>{
 let period=period_from(r);let(year,term)=explicit_context(&period)?;let(start,end)=interval(&mut **tx,&period,false).await?;let mut inputs=vec![];
 for n in 0..=(period.end_date-period.start_date).num_days(){inputs.push(duties::DatedDutyInput{id:Uuid::now_v7(),teacher_id:r.document.owner_id,original_teacher_id:r.document.owner_id,date:period.start_date+chrono::Duration::days(n),start_time:start,end_time:end,lesson:None,kind:"absence".into(),pair_id:None,pair_leg:None,taught_offering_id:None,taught_learning_group_id:None,taught_activity:None});}
 for l in plan.days.iter().flat_map(|d|&d.lessons){if let Some(a)=&l.assignment {let pair=if a.mode==LeaveCoverageMode::Reciprocal{Some(a.id)}else{None};inputs.push(duties::DatedDutyInput{id:Uuid::now_v7(),teacher_id:a.replacement_teacher_id,original_teacher_id:r.document.owner_id,date:l.lesson.date,start_time:l.lesson.start_time,end_time:l.lesson.end_time,lesson:Some(l.lesson.clone()),kind:if pair.is_some(){"reciprocal"}else{"substitution"}.into(),pair_id:pair,pair_leg:pair.map(|_|1),taught_offering_id:a.taught_offering_id,taught_learning_group_id:a.taught_learning_group_id,taught_activity:a.taught_activity.clone()});if let Some(back)=&a.return_lesson {inputs.push(duties::DatedDutyInput{id:Uuid::now_v7(),teacher_id:r.document.owner_id,original_teacher_id:a.replacement_teacher_id,date:back.date,start_time:back.start_time,end_time:back.end_time,lesson:Some(back.clone()),kind:"reciprocal".into(),pair_id:pair,pair_leg:Some(2),taught_offering_id:l.lesson.offering_id,taught_learning_group_id:l.lesson.learning_group_id,taught_activity:if l.lesson.offering_id.is_none(){l.lesson.offering_name.clone()}else{None}})}}}
 duties::reserve_on_connection(&mut **tx,r.document.id,plan.revision,year,term,&inputs).await
}
pub(super) async fn prepare_submission(tx:&mut Transaction<'_,Postgres>,r:&Record,emergency_reason:Option<&str>)->Result<LeaveCoveragePlan,AppError>{
 let fresh=preview_on_connection(&mut **tx,r.document.owner_id,&period_from(r)).await?;if fresh.timetable_state!=LeaveCoverageTimetableState::Ready{return Err(invalid("ยังไม่มีตารางเรียนในภาคเรียนนี้ กรุณาจัดตารางก่อนยื่นคำขอ"))}
 let mut plan=stored_plan(&mut **tx,r.document.id).await?.unwrap_or(fresh.clone());if plan.source_digest!=fresh.source_digest{return Err(stale())}
 if emergency_reason.is_none()&&!complete(&plan){return Err(invalid("กรุณาเลือกผู้สอนให้ครบทุกคาบก่อนส่งคำขอ"))}
 if let Some(reason)=emergency_reason {if reason.trim().is_empty()||reason.chars().count()>2000{return Err(invalid("กรุณาระบุเหตุผลฉุกเฉินไม่เกิน 2000 ตัวอักษร"))}}
 // Freeze a fresh immutable version for this submission; previous consent never migrates silently.
 plan.revision+=1;plan.emergency_reason=emergency_reason.map(str::to_owned);persist_plan(&mut **tx,r.document.id,&mut plan).await?;reserve(tx,r,&plan).await?;Ok(plan)
}
async fn task(connection:&mut PgConnection,actor:&ActorContext,id:Uuid)->Result<LeaveCoverageTask,AppError>{
 actor.require_permission(codes::STAFF_LEAVE_READ_ASSIGNED)?;let row:Option<(Json<LeaveRequestSummary>,)>=DbSession::new(connection).query_as("staff.leave.coverage.task_request","SELECT document FROM staff_leave_requests WHERE id=$1 AND status='submitted'").bind(id).fetch_optional().await?;let r=row.ok_or_else(denied)?.0.0;let mut plan=stored_plan(connection,id).await?.ok_or_else(denied)?;
 let confirmed:Vec<(Uuid,String,chrono::DateTime<Utc>)>=DbSession::new(connection).query_as("staff.leave.coverage.task_consent","SELECT assignment_id,decision,accepted_at FROM staff_leave_coverage_confirmations WHERE request_id=$1 AND plan_revision=$2 AND teacher_id=$3 AND plan_hash=$4").bind(id).bind(plan.revision).bind(actor.user_id).bind(&plan.plan_hash).fetch_all().await?;
 let mut assignments=vec![];for mut lesson in plan.days.drain(..).flat_map(|d|d.lessons){if let Some(a)=&mut lesson.assignment{if a.replacement_teacher_id==actor.user_id{if let Some((_,decision,at))=confirmed.iter().find(|(id,_,_)|*id==a.id){a.consent=if decision=="confirmed"{LeaveCoverageConsent::Confirmed}else{LeaveCoverageConsent::Declined};a.confirmed_at=Some(*at)}assignments.push(lesson)}}}if assignments.is_empty(){return Err(denied())}Ok(LeaveCoverageTask{request_id:id,request_revision:r.revision,plan_revision:plan.revision,plan_hash:plan.plan_hash,owner_name:r.owner_name,assignments})
}
pub async fn assigned(pool:&PgPool,actor:&ActorContext,q:LeaveCoverageTaskQuery)->Result<LeaveCoverageTaskPage,AppError>{actor.require_permission(codes::STAFF_LEAVE_READ_ASSIGNED)?;let limit=q.limit.unwrap_or(30);let offset=q.offset.unwrap_or(0);if !(1..=100).contains(&limit)||!(0..=10000).contains(&offset){return Err(invalid("หน้ารายการไม่ถูกต้อง"))}let mut tx=school_tenancy::TenantPool::begin_observed(pool,"staff.leave.coverage.assigned").await?;staff_name(&mut tx,actor.user_id).await?;let ids:Vec<Uuid>=DbSession::new(&mut *tx).scalar("staff.leave.coverage.assigned_ids","SELECT p.request_id FROM staff_leave_coverage_plans p JOIN staff_leave_requests r ON r.id=p.request_id AND r.status='submitted' WHERE EXISTS(SELECT 1 FROM jsonb_array_elements(p.document->'days') d CROSS JOIN LATERAL jsonb_array_elements(d->'lessons') l WHERE l->'assignment'->>'replacementTeacherId'=$1) ORDER BY r.created_at DESC,r.id LIMIT $2 OFFSET $3").bind(actor.user_id.to_string()).bind(limit+1).bind(offset).fetch_all().await?;let has_more=ids.len()>limit as usize;let mut items=vec![];for id in ids.into_iter().take(limit as usize){items.push(task(&mut tx,actor,id).await?)}tx.commit().await?;Ok(LeaveCoverageTaskPage{items,has_more})}
pub async fn confirm(pool:&PgPool,actor:&ActorContext,id:Uuid,input:ConfirmLeaveCoverageRequest)->Result<LeaveCoverageTask,AppError>{
 actor.require_permission(codes::STAFF_LEAVE_READ_ASSIGNED)?;actor.require_permission(codes::STAFF_LEAVE_UPDATE_ASSIGNED)?;if input.decision==LeaveCoverageConsent::Pending{return Err(invalid("กรุณายืนยันหรือปฏิเสธ"))}let mut tx=school_tenancy::TenantPool::begin_observed(pool,"staff.leave.coverage.confirm").await?;lock_request_context(&mut tx,id).await?;
 if claim(&mut tx,actor.user_id,input.request_id,"coverage_confirm",id,&(id,&input)).await?.is_some(){let result=task(&mut tx,actor,id).await?;tx.commit().await?;return Ok(result)}
 let r=load(&mut tx,id).await?;staff_name(&mut tx,actor.user_id).await?;let current=task(&mut tx,actor,id).await?;if current.plan_revision!=input.expected_revision{return Err(stale())}
 let expected=current.assignments.iter().filter_map(|l|l.assignment.as_ref()).map(|a|a.id).collect::<BTreeSet<_>>();if input.assignment_ids.is_empty()||input.assignment_ids.iter().any(|id|!expected.contains(id))||input.assignment_ids.iter().collect::<BTreeSet<_>>().len()!=input.assignment_ids.len(){return Err(invalid("เลือกเฉพาะหน้าที่ของตนในแผนฉบับปัจจุบัน"))}
 let plan=stored_plan(&mut tx,id).await?.ok_or_else(stale)?;revalidate(&mut tx,&r,&plan).await?;
 let decision=if input.decision==LeaveCoverageConsent::Confirmed{"confirmed"}else{"declined"};
 for assignment_id in &input.assignment_ids {
 let existing:Option<String>=DbSession::new(&mut *tx).scalar("staff.leave.coverage.existing_confirmation","SELECT decision FROM staff_leave_coverage_confirmations WHERE request_id=$1 AND plan_revision=$2 AND assignment_id=$3 AND teacher_id=$4").bind(id).bind(plan.revision).bind(assignment_id).bind(actor.user_id).fetch_optional().await?;
 if let Some(previous)=existing {if previous!=decision{return Err(stale())}continue}
 DbSession::new(&mut *tx).query("staff.leave.coverage.confirm_receipt","INSERT INTO staff_leave_coverage_confirmations(id,request_id,plan_revision,plan_hash,assignment_id,teacher_id,decision) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(Uuid::now_v7()).bind(id).bind(plan.revision).bind(&plan.plan_hash).bind(assignment_id).bind(actor.user_id).bind(decision).execute().await?;
 }
 history(&mut tx,actor,&r,if decision=="confirmed"{"coverage_confirmed"}else{"coverage_declined"},None,None,None).await?;let result=task(&mut tx,actor,id).await?;tx.commit().await?;Ok(result)
}
pub(super) async fn revalidate(tx:&mut Transaction<'_,Postgres>,r:&Record,plan:&LeaveCoveragePlan)->Result<(),AppError>{
 let period=period_from(r);let(year,term)=explicit_context(&period)?;let(start,end)=interval(&mut **tx,&period,false).await?;let fresh=duties::snapshot_on_connection(&mut **tx,year,term,r.document.owner_id,period.start_date,period.end_date,start,end).await?;if plan.source_digest.as_deref()!=Some(&fresh.source_digest){return Err(stale())}
 let keys=plan.days.iter().flat_map(|d|&d.lessons).filter_map(|l|l.assignment.as_ref().map(|a|(a.replacement_teacher_id,l.lesson.date))).collect::<Vec<_>>();duties::lock_teacher_dates(&mut **tx,&keys).await?;
 for l in plan.days.iter().flat_map(|d|&d.lessons){if let Some(a)=&l.assignment{if !duties::teacher_free_on_connection(&mut **tx,a.replacement_teacher_id,l.lesson.date,l.lesson.start_time,l.lesson.end_time,r.document.id,&[]).await?{return Err(invalid("ผู้สอนไม่พร้อมใช้งานแล้ว กรุณาจัดแผนใหม่"))}if let Some(back)=&a.return_lesson{let fresh=duties::snapshot_on_connection(&mut **tx,year,term,a.replacement_teacher_id,back.date,back.date,NaiveTime::MIN,end_of_day()?).await?;if !fresh.lessons.contains(back)||!duties::teacher_free_on_connection(&mut **tx,r.document.owner_id,back.date,back.start_time,back.end_time,r.document.id,&[]).await?{return Err(stale())}}}}Ok(())
}
pub async fn replan(pool:&PgPool,actor:&ActorContext,id:Uuid,input:LeaveMutationRequest)->Result<LeaveRequestDetail,AppError>{
 own_access(actor)?;let mut tx=school_tenancy::TenantPool::begin_observed(pool,"staff.leave.coverage.replan").await?;lock_request_context(&mut tx,id).await?;
 if claim(&mut tx,actor.user_id,input.request_id,"coverage_replan",id,&(id,&input)).await?.is_some(){tx.commit().await?;return super::queries::detail(pool,actor,id).await}
 let mut r=load(&mut tx,id).await?;if r.document.owner_id!=actor.user_id||r.document.status!=LeaveStatus::Submitted||r.document.revision!=input.expected_revision{return Err(stale())}
 let declined:bool=DbSession::new(&mut *tx).scalar("staff.leave.coverage.declined","SELECT EXISTS(SELECT 1 FROM staff_leave_coverage_confirmations c JOIN staff_leave_coverage_plans p ON p.request_id=c.request_id AND p.revision=c.plan_revision WHERE c.request_id=$1 AND c.decision='declined')").bind(id).fetch_one().await?;if !declined{return Err(invalid("ยังไม่มีผู้สอนปฏิเสธแผนนี้"))}
 super::lifecycle::return_for_coverage_revision(&mut tx,actor,&r,input.request_id).await?;duties::release_on_connection(&mut tx,id,false).await?;r.document.status=LeaveStatus::Returned;r.document.revision+=1;store(&mut tx,&r).await?;history(&mut tx,actor,&r,"coverage_revision_requested",None,None,None).await?;tx.commit().await?;super::queries::detail(pool,actor,id).await
}
#[cfg(test)] mod tests {use super::*;#[test]fn explicit_context_never_infers_from_date(){let p:LeavePeriodInput=serde_json::from_value(serde_json::json!({"typeId":Uuid::nil(),"startDate":"2026-11-17","endDate":"2026-11-17","startTime":null,"endTime":null,"dayPortion":"full_day","reason":"test"})).unwrap();assert!(explicit_context(&p).is_err());assert!(!serde_json::to_string(&p).unwrap().contains("academicYearId"));}}
