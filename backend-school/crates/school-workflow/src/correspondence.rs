//! Correspondence records, source ACL and immutable document/action decisions.
use crate::{correspondence_files,process::{self,denied,invalid,stale},process_models::*};
use chrono::{Datelike,Duration,Utc};
use school_authorization::{accessible_exact_units_for_permission,ActorContext};
use school_errors::AppError;
use school_permissions::registry::codes;
use serde::{Deserialize,Serialize};
use sqlx::{types::Json,PgPool,Postgres,Transaction};
use uuid::Uuid;

#[derive(Debug,Deserialize)]
#[cfg_attr(feature="api-docs",derive(utoipa::IntoParams))]
#[serde(rename_all="camelCase")]
pub struct CorrespondenceListQuery {pub cursor:Option<Uuid>,pub limit:Option<i64>,pub register:Option<CorrespondenceRegister>,pub status:Option<CorrespondenceStatus>,pub search:Option<String>}
#[derive(Debug,Deserialize)]
#[cfg_attr(feature="api-docs",derive(utoipa::IntoParams))]
pub struct CorrespondenceOptionsQuery{pub search:Option<String>}

pub fn validate_annotations(values:&[DocumentAnnotation])->Result<(),AppError>{
 if values.len()>300{return Err(invalid("บันทึกกำกับเกิน 300 รายการ"))}
 let mut ids=std::collections::BTreeSet::new();
 for a in values {if !ids.insert(a.id)||!valid_box(a.page,a.x,a.y,a.width,a.height)||!valid_color(&a.color)||a.text.as_ref().is_some_and(|t|t.chars().count()>5000){return Err(invalid("ตำแหน่งหรือข้อความกำกับไม่ถูกต้อง"))}validate_strokes(&a.strokes)?;if a.kind==DocumentAnnotationKind::Text&&a.text.as_deref().is_none_or(|s|s.trim().is_empty()){return Err(invalid("กรุณาระบุข้อความกำกับ"))}}
 Ok(())
}
fn valid_color(value:&str)->bool {value.len()==7&&value.starts_with('#')&&value[1..].bytes().all(|c|c.is_ascii_hexdigit())}
fn valid_box(page:u32,x:f64,y:f64,w:f64,h:f64)->bool{page>0&&page<=1000&&[x,y,w,h].iter().all(|v|v.is_finite()&&(0.0..=1.0).contains(v))&&w>0.0&&h>0.0&&x+w<=1.000001&&y+h<=1.000001}
pub fn validate_strokes(strokes:&[InkStroke])->Result<(),AppError>{
 if strokes.len()>200||strokes.iter().map(|s|s.points.len()).sum::<usize>()>30_000{return Err(invalid("เส้นกำกับมีขนาดเกินกำหนด"))}
 for s in strokes {if !s.width.is_finite()||s.width<=0.0||s.width>0.1||!valid_color(&s.color)||s.points.len()<2||s.points.iter().any(|p|!p.x.is_finite()||!p.y.is_finite()||!(0.0..=1.0).contains(&p.x)||!(0.0..=1.0).contains(&p.y)){return Err(invalid("ข้อมูลเส้นลายเซ็นไม่ถูกต้อง"))}}Ok(())
}
fn validate_content(content:&str,annotations:&[DocumentAnnotation])->Result<(),AppError>{if content.chars().count()>100_000{return Err(invalid("เนื้อหาเกิน 100,000 ตัวอักษร"))}validate_annotations(annotations)}
fn enum_code<T:Serialize>(value:T)->Result<String,AppError>{serde_json::to_value(value).ok().and_then(|v|v.as_str().map(str::to_owned)).ok_or_else(||invalid("รหัสไม่ถูกต้อง"))}
async fn exact_read_units(pool:&PgPool,actor:&ActorContext)->Result<Vec<Uuid>,AppError>{
 let mut units=Vec::new();for permission in [codes::CORRESPONDENCE_READ_ORGANIZATION_UNIT,codes::CORRESPONDENCE_MANAGE_ORGANIZATION_UNIT]{if actor.has_permission(permission){units.extend(accessible_exact_units_for_permission(pool,actor.user_id,permission).await?);}}units.sort();units.dedup();Ok(units)
}
pub async fn require_read(pool:&PgPool,actor:&ActorContext,id:Uuid)->Result<(),AppError>{
 let units=exact_read_units(pool,actor).await?;
 let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM correspondence_records c WHERE c.id=$1 AND ($3 OR (c.created_by=$2 AND $4) OR (c.organization_unit_id=ANY($5)) OR ($6 AND EXISTS(SELECT 1 FROM process_participants p WHERE p.instance_id=c.instance_id AND p.user_id=$2))))").bind(id).bind(actor.user_id).bind(actor.has_any_permission(&[codes::CORRESPONDENCE_READ_SCHOOL,codes::CORRESPONDENCE_MANAGE_SCHOOL])).bind(actor.has_any_permission(&[codes::CORRESPONDENCE_READ_OWN,codes::CORRESPONDENCE_CREATE_OWN])).bind(units).bind(actor.has_any_permission(&[codes::CORRESPONDENCE_READ_ASSIGNED,codes::CORRESPONDENCE_UPDATE_ASSIGNED])).fetch_one(pool).await?;
 if allowed{Ok(())}else{Err(denied())}
}
pub async fn require_upload(pool:&PgPool,actor:&ActorContext,id:Uuid)->Result<(),AppError>{
 require_read(pool,actor,id).await?;
 let (creator,status):(Uuid,String)=sqlx::query_as("SELECT created_by,summary->>'status' FROM correspondence_records WHERE id=$1").bind(id).fetch_one(pool).await?;
 if creator==actor.user_id&&actor.has_permission(codes::CORRESPONDENCE_CREATE_OWN)&&matches!(status.as_str(),"draft"|"revision_requested"){return Ok(())}
 let assigned:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM correspondence_records c JOIN process_instances i ON i.id=c.instance_id WHERE c.id=$1 AND i.definition->>'status'='active' AND EXISTS(SELECT 1 FROM jsonb_array_elements(i.definition->'steps'->((i.definition->>'currentStep')::int)->'recipients') r WHERE (r->>'userId')::uuid=$2 AND r->>'completedAt' IS NULL))").bind(id).bind(actor.user_id).fetch_one(pool).await?;
 if assigned&&actor.has_permission(codes::CORRESPONDENCE_UPDATE_ASSIGNED){Ok(())}else{Err(denied())}
}
fn require_author(actor:&ActorContext,record:&CorrespondenceSummary)->Result<(),AppError>{if actor.user_id==record.created_by&&actor.has_permission(codes::CORRESPONDENCE_CREATE_OWN){Ok(())}else{Err(denied())}}
async fn locked(tx:&mut Transaction<'_,Postgres>,id:Uuid)->Result<(CorrespondenceSummary,i64,Option<Uuid>),AppError>{let (record,document,instance):(Json<CorrespondenceSummary>,i64,Option<Uuid>)=sqlx::query_as("SELECT summary,current_document_revision,instance_id FROM correspondence_records WHERE id=$1 FOR UPDATE").bind(id).fetch_optional(&mut **tx).await?.ok_or_else(||AppError::NotFound("ไม่พบหนังสือ".into()))?;Ok((record.0,document,instance))}
async fn save_record(tx:&mut Transaction<'_,Postgres>,record:&CorrespondenceSummary)->Result<(),AppError>{sqlx::query("UPDATE correspondence_records SET revision=$2,summary=$3 WHERE id=$1").bind(record.id).bind(record.revision).bind(Json(record)).execute(&mut **tx).await?;Ok(())}
async fn document_in(tx:&mut Transaction<'_,Postgres>,id:Uuid,revision:i64)->Result<CorrespondenceDocumentRevision,AppError>{Ok(sqlx::query_scalar::<_,Json<CorrespondenceDocumentRevision>>("SELECT document FROM correspondence_document_revisions WHERE correspondence_id=$1 AND revision=$2").bind(id).bind(revision).fetch_one(&mut **tx).await?.0)}
async fn add_document(tx:&mut Transaction<'_,Postgres>,id:Uuid,actor:Uuid,revision:i64,content:String,original:Option<Uuid>,attachments:Vec<Uuid>,annotations:Vec<DocumentAnnotation>)->Result<(),AppError>{
 validate_content(&content,&annotations)?;let mut files=attachments.clone();files.extend(original);let file_snapshots=correspondence_files::attach_documents(tx,actor,id,revision,&files).await?;
 let content_hash=process::payload_hash(&(revision,&content,&original,&attachments,&annotations,&file_snapshots))?;
 let document=CorrespondenceDocumentRevision{revision,content,original_file_id:original,attachment_file_ids:attachments,annotations,content_hash,file_snapshots,created_by:actor,created_at:Utc::now()};
 sqlx::query("INSERT INTO correspondence_document_revisions(correspondence_id,revision,document,created_by) VALUES($1,$2,$3,$4)").bind(id).bind(revision).bind(Json(&document)).bind(actor).execute(&mut **tx).await?;
 sqlx::query("INSERT INTO correspondence_file_links(correspondence_id,file_id,document_revision) SELECT $1,unnest($2::uuid[]),$3 ON CONFLICT DO NOTHING").bind(id).bind(files).bind(revision).execute(&mut **tx).await?;
 sqlx::query("UPDATE correspondence_records SET current_document_revision=$2 WHERE id=$1").bind(id).bind(revision).execute(&mut **tx).await?;Ok(())
}
async fn append_history(tx:&mut Transaction<'_,Postgres>,id:Uuid,instance:Option<Uuid>,actor:Uuid,revision:i64,action:&str,comment:Option<String>,evidence:Vec<Uuid>,annotations:Vec<DocumentAnnotation>,document_revision:i64)->Result<Uuid,AppError>{
 let actor_name:String=sqlx::query_scalar("SELECT display_name FROM user_display_names WHERE id=$1").bind(actor).fetch_one(&mut **tx).await?;
 let entry=ProcessHistoryEntry{id:Uuid::now_v7(),actor_id:actor,actor_name,action:action.into(),comment,evidence_file_ids:evidence,annotations,document_revision,created_at:Utc::now(),revision};
 sqlx::query("INSERT INTO process_action_history(id,instance_id,correspondence_id,actor_id,entry) VALUES($1,$2,$3,$4,$5)").bind(entry.id).bind(instance).bind(id).bind(actor).bind(Json(&entry)).execute(&mut **tx).await?;Ok(entry.id)
}
pub async fn create(pool:&PgPool,actor:&ActorContext,request:CreateCorrespondenceRequest)->Result<CorrespondenceDetail,AppError>{
 actor.require_permission(codes::CORRESPONDENCE_CREATE_OWN)?;validate_content(&request.content,&request.annotations)?;
 if request.title.trim().is_empty()||request.title.chars().count()>300||request.sender.trim().is_empty()||request.sender.chars().count()>300||request.document_number.as_ref().is_some_and(|s|s.chars().count()>150)||request.source_links.len()>30||request.source_links.iter().any(|s|s.label.chars().count()>200||s.url.len()>2000||!valid_source_url(&s.url)||s.url.chars().any(char::is_control)){return Err(invalid("ชื่อเรื่อง ผู้ส่ง เลขหนังสือ หรือลิงก์ต้นทางไม่ถูกต้อง"))}
 let id=Uuid::now_v7();let mut tx=pool.begin().await?;if let Some(old)=process::creation_receipt(&mut tx,actor.user_id,request.request_id,"correspondence_create",id,&request).await?{tx.commit().await?;return detail(pool,actor,old).await}
 let year=(Utc::now()+Duration::hours(7)).year()+543;let kind=enum_code(request.register)?;
 let number:i64=sqlx::query_scalar("INSERT INTO correspondence_register_counters(register,year,last_number) VALUES($1,$2,1) ON CONFLICT(register,year) DO UPDATE SET last_number=correspondence_register_counters.last_number+1 RETURNING last_number").bind(&kind).bind(year).fetch_one(&mut *tx).await?;
 let record=CorrespondenceSummary{id,register_number:number,register_year:year,register:request.register,title:request.title,sender:request.sender,document_number:request.document_number,document_date:request.document_date,urgency:request.urgency,organization_unit_id:request.organization_unit_id,revision:1,status:CorrespondenceStatus::Draft,created_by:actor.user_id,created_at:Utc::now()};
 sqlx::query("INSERT INTO correspondence_records(id,revision,created_by,organization_unit_id,summary,source_links,current_document_revision,register_number,register_year,register_kind) VALUES($1,1,$2,$3,$4,$5,1,$6,$7,$8)").bind(id).bind(actor.user_id).bind(record.organization_unit_id).bind(Json(&record)).bind(Json(&request.source_links)).bind(number).bind(year).bind(kind).execute(&mut *tx).await?;
 add_document(&mut tx,id,actor.user_id,1,request.content,request.original_file_id,request.attachment_file_ids,request.annotations).await?;append_history(&mut tx,id,None,actor.user_id,1,"create",None,vec![],vec![],1).await?;tx.commit().await?;detail(pool,actor,id).await
}
pub async fn revise(pool:&PgPool,actor:&ActorContext,id:Uuid,request:ReviseCorrespondenceRequest)->Result<CorrespondenceDetail,AppError>{
 require_read(pool,actor,id).await?;let mut tx=pool.begin().await?;if process::receipt(&mut tx,actor.user_id,request.request_id,"correspondence_revise",id,&request).await?.is_some(){tx.commit().await?;return detail(pool,actor,id).await}
 let(mut record,doc,_)=locked(&mut tx,id).await?;require_author(actor,&record)?;if record.revision!=request.expected_revision{return Err(stale())}if !matches!(record.status,CorrespondenceStatus::Draft|CorrespondenceStatus::RevisionRequested){return Err(invalid("แก้ไขได้เฉพาะฉบับร่างหรือรายการส่งกลับแก้ไข"))}
 add_document(&mut tx,id,actor.user_id,doc+1,request.content,request.original_file_id,request.attachment_file_ids,request.annotations).await?;record.revision+=1;save_record(&mut tx,&record).await?;append_history(&mut tx,id,None,actor.user_id,record.revision,"revise",None,vec![],vec![],doc+1).await?;tx.commit().await?;detail(pool,actor,id).await
}
struct CorrespondenceSource;
#[async_trait::async_trait]
impl process::ProcessSourcePort for CorrespondenceSource{
 async fn validate_start(&self,tx:&mut Transaction<'_,Postgres>,actor:&ActorContext,source:&ProcessSource)->Result<(),AppError>{if source.module_code!="correspondence"||source.resource_type!="correspondence"{return Err(denied())}let(record,_,_)=locked(tx,source.resource_id).await?;require_author(actor,&record)?;if !matches!(record.status,CorrespondenceStatus::Draft|CorrespondenceStatus::RevisionRequested){return Err(stale())}Ok(())}
 async fn validate_action(&self,_tx:&mut Transaction<'_,Postgres>,actor:&ActorContext,instance:&ProcessInstance,action:ProcessAction)->Result<(),AppError>{if instance.source.module_code!="correspondence"||instance.source.resource_type!="correspondence"{return Err(denied())}if action==ProcessAction::Close&&instance.created_by==actor.user_id{actor.require_permission(codes::CORRESPONDENCE_CREATE_OWN)}else if action==ProcessAction::Read{actor.require_permission(codes::CORRESPONDENCE_READ_ASSIGNED)}else{actor.require_permission(codes::CORRESPONDENCE_UPDATE_ASSIGNED)}}
 async fn validate_delegated_authority(&self,tx:&mut Transaction<'_,Postgres>,actor:&ActorContext,instance:&ProcessInstance)->Result<(),AppError>{
 let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM correspondence_records c JOIN organization_permission_delegations d ON d.organization_unit_id IS NOT DISTINCT FROM c.organization_unit_id JOIN permissions p ON p.id=d.permission_id WHERE c.id=$1 AND d.to_user_id=$2 AND d.revoked_at IS NULL AND d.started_at<=now() AND (d.expires_at IS NULL OR d.expires_at>now()) AND p.code=$3)").bind(instance.source.resource_id).bind(actor.user_id).bind(codes::CORRESPONDENCE_UPDATE_ASSIGNED).fetch_one(&mut **tx).await?;
 if allowed{Ok(())}else{Err(denied())}
 }
 async fn completed(&self,_tx:&mut Transaction<'_,Postgres>,_instance:&ProcessInstance)->Result<(),AppError>{Ok(())}
}
pub async fn send(pool:&PgPool,actor:&ActorContext,id:Uuid,request:SendCorrespondenceRequest)->Result<CorrespondenceDetail,AppError>{
 require_read(pool,actor,id).await?;let mut tx=pool.begin().await?;if process::receipt(&mut tx,actor.user_id,request.request_id,"correspondence_send",id,&request).await?.is_some(){tx.commit().await?;return detail(pool,actor,id).await}
 let(mut record,doc,_)=locked(&mut tx,id).await?;require_author(actor,&record)?;if record.revision!=request.expected_revision{return Err(stale())}
 let instance=process::start_core_in_transaction(&mut tx,actor,&CorrespondenceSource,process::StartProcessInput{request_id:request.request_id,source:ProcessSource{module_code:"correspondence".into(),resource_type:"correspondence".into(),resource_id:id},template_id:request.template_id,title:record.title.clone(),action_path:format!("/staff/correspondence/{id}"),step_recipients:request.step_recipients,due_at:request.due_at}).await?;
 record.status=CorrespondenceStatus::InProgress;record.revision+=1;sqlx::query("UPDATE correspondence_records SET instance_id=$2 WHERE id=$1").bind(id).bind(instance.id).execute(&mut *tx).await?;save_record(&mut tx,&record).await?;append_history(&mut tx,id,Some(instance.id),actor.user_id,record.revision,"send",None,vec![],vec![],doc).await?;tx.commit().await?;Ok(detail(pool,actor,id).await?)
}
pub async fn act(pool:&PgPool,actor:&ActorContext,id:Uuid,request:ProcessActionRequest)->Result<CorrespondenceDetail,AppError>{act_impl(pool,actor,id,request,false).await}
async fn act_impl(pool:&PgPool,actor:&ActorContext,id:Uuid,request:ProcessActionRequest,instance_revision:bool)->Result<CorrespondenceDetail,AppError>{
 require_read(pool,actor,id).await?;validate_annotations(&request.annotations)?;
 if request.action==ProcessAction::Read && (!request.annotations.is_empty()||!request.evidence_file_ids.is_empty()||request.signature.is_some()||request.comment.is_some()){return Err(invalid("การอ่านไม่รับความเห็น เอกสาร หรือลายเซ็น"))}
 if matches!(request.action,ProcessAction::Reassign|ProcessAction::Close) && (!request.annotations.is_empty()||!request.evidence_file_ids.is_empty()){return Err(invalid("การดำเนินการนี้ไม่รับชั้นกำกับหรือเอกสารแนบ"))}
 if request.signature.is_some() && matches!(request.action,ProcessAction::Read|ProcessAction::Reassign|ProcessAction::Close){return Err(invalid("การดำเนินการนี้ไม่รองรับลายเซ็น"))}if request.comment.as_ref().is_some_and(|s|s.chars().count()>10000){return Err(invalid("ความเห็นเกินกำหนด"))}
 let mut tx=pool.begin().await?;if process::receipt(&mut tx,actor.user_id,request.request_id,if instance_revision{"process_correspondence_action"}else{"correspondence_action"},id,&request).await?.is_some(){tx.commit().await?;return detail(pool,actor,id).await}
 let(mut record,doc,instance_id)=locked(&mut tx,id).await?;if !instance_revision&&record.revision!=request.expected_revision{return Err(stale())}let instance_id=instance_id.ok_or_else(||invalid("ยังไม่ได้ส่งหนังสือ"))?;let mut instance=process::load_locked(&mut tx,instance_id).await?;
 if instance_revision&&instance.revision!=request.expected_revision{return Err(stale())}
 let mut engine_request=request.clone();engine_request.expected_revision=instance.revision;
 let document=document_in(&mut tx,id,doc).await?;
 if let Some(signature)=&request.signature {actor.require_permission(codes::CORRESPONDENCE_SIGN_OWN)?;if signature.document_revision!=doc||signature.content_hash!=document.content_hash{return Err(stale())}if !valid_box(signature.page,signature.x,signature.y,signature.width,signature.height){return Err(invalid("ตำแหน่งลายเซ็นไม่ถูกต้อง"))}match signature.method{SignatureMethod::Drawn=>{if signature.asset_id.is_some()||signature.strokes.is_empty(){return Err(invalid("กรุณาวาดลายเซ็น"))}validate_strokes(&signature.strokes)?},SignatureMethod::Image=>{if signature.asset_id.is_none()||!signature.strokes.is_empty(){return Err(invalid("กรุณาเลือกลายเซ็นของคุณ"))}}}}
 if !request.evidence_file_ids.is_empty(){correspondence_files::attach_documents(&mut tx,actor.user_id,id,doc,&request.evidence_file_ids).await?;sqlx::query("INSERT INTO correspondence_file_links(correspondence_id,file_id,document_revision) SELECT $1,unnest($2::uuid[]),$3 ON CONFLICT DO NOTHING").bind(id).bind(&request.evidence_file_ids).bind(doc).execute(&mut *tx).await?;}
 process::action_in_transaction(&mut tx,actor,&CorrespondenceSource,&mut instance,&engine_request).await?;
 record.revision+=1;record.status=match instance.status{ProcessStatus::Active=>CorrespondenceStatus::InProgress,ProcessStatus::RevisionRequested=>CorrespondenceStatus::RevisionRequested,ProcessStatus::Completed=>CorrespondenceStatus::Completed,ProcessStatus::Rejected=>CorrespondenceStatus::Rejected,ProcessStatus::Closed=>CorrespondenceStatus::Closed};
 let action_code=enum_code(request.action)?;
 let action_id=append_history(&mut tx,id,Some(instance_id),actor.user_id,record.revision,&action_code,request.comment.clone(),request.evidence_file_ids.clone(),request.annotations.clone(),doc).await?;
 if let Some(signature)=request.signature {
 let file_id=if let Some(asset)=signature.asset_id {Some(sqlx::query_scalar::<_,Uuid>("SELECT a.file_id FROM correspondence_signature_assets a JOIN files f ON f.id=a.file_id WHERE a.id=$1 AND a.owner_id=$2 AND f.visibility='private' AND f.lifecycle_status='ready' AND f.deleted_at IS NULL FOR SHARE OF a,f").bind(asset).bind(actor.user_id).fetch_optional(&mut *tx).await?.ok_or_else(denied)?)}else{None};
 let overlays:Vec<Json<ProcessHistoryEntry>>=sqlx::query_scalar("SELECT entry FROM process_action_history WHERE correspondence_id=$1 AND (entry->>'documentRevision')::bigint=$2 ORDER BY created_at,id").bind(id).bind(doc).fetch_all(&mut *tx).await?;
 let signer_name:String=sqlx::query_scalar("SELECT display_name FROM user_display_names WHERE id=$1").bind(actor.user_id).fetch_one(&mut *tx).await?;
 let signer_role:Option<String>=sqlx::query_scalar("SELECT coalesce(position_title,position_code) FROM organization_members WHERE user_id=$1 AND started_at<=CURRENT_DATE AND (ended_at IS NULL OR ended_at>CURRENT_DATE) ORDER BY is_primary DESC,id LIMIT 1").bind(actor.user_id).fetch_optional(&mut *tx).await?;
 let prior_signatures:Vec<Json<CorrespondenceSigningEvidence>>=sqlx::query_scalar("SELECT evidence FROM correspondence_signing_evidence WHERE correspondence_id=$1 AND document_revision=$2 ORDER BY created_at,id").bind(id).bind(doc).fetch_all(&mut *tx).await?;
 let signed_at=Utc::now();let signed_content_hash=process::payload_hash(&(&document.content_hash,overlays.iter().map(|e|&e.0).collect::<Vec<_>>(),prior_signatures.iter().map(|s|&s.0).collect::<Vec<_>>(),actor.user_id,signed_at,&signature,file_id))?;
 let evidence=CorrespondenceSigningEvidence{id:Uuid::now_v7(),signer_id:actor.user_id,signer_name,signer_role,signed_at,signed_content_hash,signature,file_id,action_id};
 sqlx::query("INSERT INTO correspondence_signing_evidence(id,correspondence_id,document_revision,signer_id,asset_file_id,action_id,evidence) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(evidence.id).bind(id).bind(doc).bind(actor.user_id).bind(file_id).bind(action_id).bind(Json(&evidence)).execute(&mut *tx).await?;}
 save_record(&mut tx,&record).await?;tx.commit().await?;detail(pool,actor,id).await
}
pub async fn list(pool:&PgPool,actor:&ActorContext,query:CorrespondenceListQuery)->Result<CorrespondenceList,AppError>{
 let units=exact_read_units(pool,actor).await?;let limit=query.limit.unwrap_or(30).clamp(1,100);let search=query.search.unwrap_or_default();if search.len()>300{return Err(invalid("คำค้นยาวเกินกำหนด"))}
 let rows:Vec<Json<CorrespondenceSummary>>=sqlx::query_scalar("SELECT c.summary FROM correspondence_records c WHERE ($1 OR (c.created_by=$2 AND $3) OR c.organization_unit_id=ANY($4) OR ($5 AND EXISTS(SELECT 1 FROM process_participants p WHERE p.instance_id=c.instance_id AND p.user_id=$2))) AND ($6::uuid IS NULL OR c.id<$6) AND ($7::text IS NULL OR c.summary->>'register'=$7) AND ($8::text IS NULL OR c.summary->>'status'=$8) AND ($9='' OR c.summary->>'title' ILIKE '%'||$9||'%' OR c.summary->>'documentNumber' ILIKE '%'||$9||'%' OR c.summary->>'sender' ILIKE '%'||$9||'%' OR EXISTS(SELECT 1 FROM correspondence_document_revisions d WHERE d.correspondence_id=c.id AND d.revision=c.current_document_revision AND (d.document->>'content' ILIKE '%'||$9||'%' OR EXISTS(SELECT 1 FROM jsonb_array_elements(d.document->'annotations') a WHERE a->>'text' ILIKE '%'||$9||'%'))) OR EXISTS(SELECT 1 FROM process_action_history h WHERE h.correspondence_id=c.id AND (h.entry->>'comment' ILIKE '%'||$9||'%' OR EXISTS(SELECT 1 FROM jsonb_array_elements(h.entry->'annotations') a WHERE a->>'text' ILIKE '%'||$9||'%')))) ORDER BY c.id DESC LIMIT $10").bind(actor.has_any_permission(&[codes::CORRESPONDENCE_READ_SCHOOL,codes::CORRESPONDENCE_MANAGE_SCHOOL])).bind(actor.user_id).bind(actor.has_any_permission(&[codes::CORRESPONDENCE_READ_OWN,codes::CORRESPONDENCE_CREATE_OWN])).bind(units).bind(actor.has_any_permission(&[codes::CORRESPONDENCE_READ_ASSIGNED,codes::CORRESPONDENCE_UPDATE_ASSIGNED])).bind(query.cursor).bind(query.register.map(enum_code).transpose()?).bind(query.status.map(enum_code).transpose()?).bind(search).bind(limit+1).fetch_all(pool).await?;
 let mut items:Vec<_>=rows.into_iter().map(|r|r.0).collect();let next_cursor=if items.len()>limit as usize{items.truncate(limit as usize);items.last().map(|r|r.id)}else{None};Ok(CorrespondenceList{items,next_cursor})
}
pub fn available_actions(actor:&ActorContext,record:&CorrespondenceSummary,instance:Option<&ProcessInstance>)->Vec<ProcessAction>{
 let Some(instance)=instance else{return vec![]};
 if actor.user_id==record.created_by&&actor.has_permission(codes::CORRESPONDENCE_CREATE_OWN)&&matches!(instance.status,ProcessStatus::Completed|ProcessStatus::Rejected){return vec![ProcessAction::Close]}
 if instance.status!=ProcessStatus::Active{return vec![]}
 let Some(step)=instance.steps.get(instance.current_step)else{return vec![]};
 if !step.recipients.iter().any(|r|r.user_id==actor.user_id&&r.completed_at.is_none()){return vec![]}
 let mut result=vec![];if actor.has_permission(codes::CORRESPONDENCE_READ_ASSIGNED){result.push(ProcessAction::Read)}
 if actor.has_permission(codes::CORRESPONDENCE_UPDATE_ASSIGNED){result.extend([ProcessAction::Report,ProcessAction::Reject,ProcessAction::ReturnForRevision,ProcessAction::Reassign]);if (!step.definition.requires_signature&&step.definition.kind!=ProcessStepKind::Sign)||actor.has_permission(codes::CORRESPONDENCE_SIGN_OWN){result.push(match step.definition.kind{ProcessStepKind::Acknowledge=>ProcessAction::Acknowledge,ProcessStepKind::Review=>ProcessAction::Review,ProcessStepKind::Approve=>ProcessAction::Approve,ProcessStepKind::Execute=>ProcessAction::Execute,ProcessStepKind::Sign=>ProcessAction::Sign})}}result
}
pub async fn detail(pool:&PgPool,actor:&ActorContext,id:Uuid)->Result<CorrespondenceDetail,AppError>{
 require_read(pool,actor,id).await?;let(record,source_links,current,instance_id):(Json<CorrespondenceSummary>,Json<Vec<CorrespondenceSourceLink>>,i64,Option<Uuid>)=sqlx::query_as("SELECT summary,source_links,current_document_revision,instance_id FROM correspondence_records WHERE id=$1").bind(id).fetch_one(pool).await?;
 let revisions:Vec<_>=sqlx::query_scalar::<_,Json<CorrespondenceDocumentRevision>>("SELECT document FROM correspondence_document_revisions WHERE correspondence_id=$1 ORDER BY revision").bind(id).fetch_all(pool).await?.into_iter().map(|r|r.0).collect();let document=revisions.iter().find(|r|r.revision==current).cloned().ok_or_else(||AppError::InternalServerError("ไม่พบฉบับเอกสาร".into()))?;
 let instance=if let Some(instance_id)=instance_id{sqlx::query_scalar::<_,Json<ProcessInstance>>("SELECT definition FROM process_instances WHERE id=$1").bind(instance_id).fetch_optional(pool).await?.map(|r|r.0)}else{None};
 let history=sqlx::query_scalar::<_,Json<ProcessHistoryEntry>>("SELECT entry FROM process_action_history WHERE correspondence_id=$1 ORDER BY created_at,id").bind(id).fetch_all(pool).await?.into_iter().map(|r|r.0).collect();
 let signatures=sqlx::query_scalar::<_,Json<CorrespondenceSigningEvidence>>("SELECT evidence FROM correspondence_signing_evidence WHERE correspondence_id=$1 ORDER BY created_at,id").bind(id).fetch_all(pool).await?.into_iter().map(|r|r.0).collect();
 let actions=available_actions(actor,&record,instance.as_ref());let can_manage=actor.user_id==record.created_by&&actor.has_permission(codes::CORRESPONDENCE_CREATE_OWN)&&matches!(record.status,CorrespondenceStatus::Draft|CorrespondenceStatus::RevisionRequested);let can_act=!actions.is_empty();Ok(CorrespondenceDetail{record:record.0,source_links:source_links.0,document,revisions,instance,history,signatures,can_manage,can_act,available_actions:actions})
}
pub async fn options(pool:&PgPool,actor:&ActorContext,query:CorrespondenceOptionsQuery)->Result<CorrespondenceOptions,AppError>{
 if !actor.has_any_permission(&[codes::CORRESPONDENCE_CREATE_OWN,codes::CORRESPONDENCE_UPDATE_ASSIGNED,codes::PROCESS_MANAGE_SCHOOL]){return Err(denied())}let search=query.search.unwrap_or_default();if search.len()>200{return Err(invalid("คำค้นยาวเกินกำหนด"))}
 let users=sqlx::query_as::<_,(Uuid,String)>("SELECT id,display_name FROM user_display_names WHERE status='active' AND user_type='staff' AND ($1='' OR display_name ILIKE '%'||$1||'%') ORDER BY first_name,last_name,id LIMIT 200").bind(search).fetch_all(pool).await?.into_iter().map(|(id,display_name)|CorrespondenceUserOption{id,display_name}).collect();
 let organizations=sqlx::query_as::<_,(Uuid,String)>("SELECT id,name FROM organization_units WHERE is_active ORDER BY name,id LIMIT 500").fetch_all(pool).await?.into_iter().map(|(id,name)|CorrespondenceOrganizationOption{id,name}).collect();
 let positions=sqlx::query_as::<_,(Uuid,String,String)>("SELECT DISTINCT m.organization_unit_id,m.position_code,coalesce(m.position_title,m.position_code) FROM organization_members m JOIN organization_units ou ON ou.id=m.organization_unit_id AND ou.is_active WHERE m.started_at<=CURRENT_DATE AND (m.ended_at IS NULL OR m.ended_at>CURRENT_DATE) ORDER BY m.organization_unit_id,m.position_code LIMIT 1000").fetch_all(pool).await?.into_iter().map(|(organization_unit_id,code,name)|CorrespondencePositionOption{organization_unit_id,code,name}).collect();
 Ok(CorrespondenceOptions{users,organizations,positions,templates:process::list_templates(pool).await?})
}
pub async fn signatures(pool:&PgPool,actor:&ActorContext)->Result<Vec<SignatureAsset>,AppError>{actor.require_permission(codes::CORRESPONDENCE_SIGN_OWN)?;Ok(sqlx::query_as::<_,(Uuid,String,Uuid,chrono::DateTime<Utc>)>("SELECT id,label,file_id,created_at FROM correspondence_signature_assets WHERE owner_id=$1 ORDER BY created_at,id LIMIT 100").bind(actor.user_id).fetch_all(pool).await?.into_iter().map(|(id,label,file_id,created_at)|SignatureAsset{id,label,file_id,created_at}).collect())}
pub async fn create_signature(pool:&PgPool,actor:&ActorContext,request:CreateSignatureAssetRequest)->Result<SignatureAsset,AppError>{
 actor.require_permission(codes::CORRESPONDENCE_SIGN_OWN)?;if request.label.trim().is_empty()||request.label.chars().count()>100{return Err(invalid("กรุณาระบุชื่อลายเซ็นไม่เกิน 100 ตัวอักษร"))}let mut tx=pool.begin().await?;let id=Uuid::now_v7();let replay=process::creation_receipt(&mut tx,actor.user_id,request.request_id,"signature_create",id,&request).await?;
 if let Some(old)=replay{let(id,label,file_id,created_at):(Uuid,String,Uuid,chrono::DateTime<Utc>)=sqlx::query_as("SELECT id,label,file_id,created_at FROM correspondence_signature_assets WHERE id=$1 AND owner_id=$2").bind(old).bind(actor.user_id).fetch_one(&mut *tx).await?;tx.commit().await?;return Ok(SignatureAsset{id,label,file_id,created_at})}
 correspondence_files::attach_signature(&mut tx,actor.user_id,request.file_id).await?;let result=SignatureAsset{id,label:request.label,file_id:request.file_id,created_at:Utc::now()};sqlx::query("INSERT INTO correspondence_signature_assets(id,owner_id,label,file_id,created_at) VALUES($1,$2,$3,$4,$5)").bind(id).bind(actor.user_id).bind(&result.label).bind(result.file_id).bind(result.created_at).execute(&mut *tx).await?;tx.commit().await?;Ok(result)
}
pub async fn instance_detail(pool:&PgPool,actor:&ActorContext,id:Uuid)->Result<ProcessInstance,AppError>{let instance:Json<ProcessInstance>=sqlx::query_scalar("SELECT definition FROM process_instances WHERE id=$1 AND source_module='correspondence' AND source_type='correspondence'").bind(id).fetch_optional(pool).await?.ok_or_else(denied)?;require_read(pool,actor,instance.source.resource_id).await?;Ok(instance.0)}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn normalized_boxes_reject_nan_and_escape(){assert!(valid_box(1,0.1,0.2,0.2,0.2));assert!(!valid_box(0,0.1,0.2,0.2,0.2));assert!(!valid_box(1,f64::NAN,0.2,0.2,0.2));assert!(!valid_box(1,0.9,0.2,0.2,0.2));}
 #[test]fn read_does_not_imply_acknowledgement(){assert!(!process::action_matches(ProcessStepKind::Acknowledge,ProcessAction::Read))}
 #[test]fn ink_data_is_bounded(){assert!(validate_strokes(&[InkStroke{points:vec![InkPoint{x:0.0,y:0.0},InkPoint{x:1.1,y:0.0}],width:0.01,color:"#111111".into()}]).is_err())}
}

pub async fn readable_ids(pool:&PgPool,actor:&ActorContext,ids:&[Uuid])->Result<Vec<Uuid>,AppError>{
 if ids.is_empty(){return Ok(vec![])}let units=exact_read_units(pool,actor).await?;
 Ok(sqlx::query_scalar("SELECT c.id FROM correspondence_records c WHERE c.id=ANY($1) AND ($3 OR (c.created_by=$2 AND $4) OR c.organization_unit_id=ANY($5) OR ($6 AND EXISTS(SELECT 1 FROM process_participants p WHERE p.instance_id=c.instance_id AND p.user_id=$2)))").bind(ids).bind(actor.user_id).bind(actor.has_any_permission(&[codes::CORRESPONDENCE_READ_SCHOOL,codes::CORRESPONDENCE_MANAGE_SCHOOL])).bind(actor.has_any_permission(&[codes::CORRESPONDENCE_READ_OWN,codes::CORRESPONDENCE_CREATE_OWN])).bind(units).bind(actor.has_any_permission(&[codes::CORRESPONDENCE_READ_ASSIGNED,codes::CORRESPONDENCE_UPDATE_ASSIGNED])).fetch_all(pool).await?)
}

fn valid_source_url(value:&str)->bool{url::Url::parse(value).is_ok_and(|u|matches!(u.scheme(),"http"|"https")&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none())}

pub async fn act_instance(pool:&PgPool,actor:&ActorContext,id:Uuid,request:ProcessActionRequest)->Result<ProcessInstance,AppError>{let instance=instance_detail(pool,actor,id).await?;act_impl(pool,actor,instance.source.resource_id,request,true).await?.instance.ok_or_else(denied)}
