# Certificate Campaign Permanent Purge Design

## เป้าหมาย

ปรับระบบเกียรติบัตรให้ผู้มีสิทธิ์สามารถลบกิจกรรมได้อย่างถาวร ไม่ว่ากิจกรรมนั้นจะเป็นฉบับร่าง มีคำขอที่กำลังดำเนินการ หรือออกเกียรติบัตรแล้ว การลบต้องครอบคลุมข้อมูลในโมดูลเกียรติบัตร ไฟล์จริงใน storage ข้อมูล File Platform และ audit log ที่เกี่ยวข้อง โดยไม่เปิดช่องให้คำสั่งลบโดยตรงข้ามกติกาความเป็น immutable ของระบบ

เมื่อเริ่มลบ กิจกรรมต้องหยุดใช้งานและหายจากผู้ใช้ทั่วไปทันที แต่การลบแถวฐานข้อมูลขั้นสุดท้ายต้องเกิดหลัง storage ยืนยันแล้วว่า object ของกิจกรรมนั้นถูกลบหรือไม่มีอยู่จริง เพื่อไม่ให้เกิด object กำพร้าหรือข้อมูลครึ่งลบครึ่งเหลือ

เอกสารนี้แทนกติกาเดิมเฉพาะส่วนที่ระบุว่าออกเกียรติบัตรแล้วห้ามลบใน `2026-08-13-certificate-issuance-and-verification-design.md` กติกาอื่นของระบบเกียรติบัตรยังคงเดิม

## ผลลัพธ์ที่ผู้ใช้จะได้รับ

- ผู้มีสิทธิ์ลบเห็นส่วนอันตราย “ลบกิจกรรมถาวร” ในหน้าภาพรวมกิจกรรม
- ก่อนลบ ระบบแสดงจำนวนแบบ ผู้รับ คำขอ เกียรติบัตร และไฟล์ที่จะได้รับผลกระทบ
- ผู้ใช้ต้องพิมพ์ชื่อกิจกรรมให้ตรงทุกตัวอักษรจึงยืนยันได้
- กิจกรรมหายจากหน้าปกติ หน้าเกียรติบัตรส่วนตัว การตรวจสอบ QR/เลข และการดาวน์โหลดทันทีที่รับคำสั่งลบสำเร็จ
- ระบบลบ background รูปภาพ ฟอนต์ และไฟล์ชั่วคราวของแม่แบบจาก provider ด้วยงานที่ retry ได้
- หลัง object ถูกลบครบ ระบบลบข้อมูล certificate, audit และ File Platform metadata จริง ไม่เหลือ tombstone ของไฟล์ชุดนั้น
- หาก provider ขัดข้อง ผู้มีสิทธิ์เห็นสถานะที่ปลอดภัยและกดลองใหม่ได้ โดยกิจกรรมยังถูกล็อกและไม่กลับมาเผยแพร่
- เมื่อลบเสร็จ เลขเกียรติบัตรและ QR เดิมตอบว่าไม่พบ และเลขเดิมไม่ถูกนำกลับมาใช้

## ขอบเขต

### อยู่ในขอบเขต

- ลบกิจกรรมทุกสถานะ รวมทั้งกิจกรรมที่มีเกียรติบัตร `issued` หรือ `revoked`
- ลบคำขอ `pending`, `reviewing`, `returned`, `withdrawn` และ `issued` โดยอัตโนมัติเป็นส่วนหนึ่งของ purge
- ลบ candidate ที่ใช้งานอยู่ ถูก soft-delete หรือใช้สร้างใบทดแทนแล้ว
- ลบ replacement graph ภายในกิจกรรมเดียวกัน
- ลบไฟล์ที่กิจกรรมเป็นเจ้าของทั้ง object และ metadata
- ลบ `audit_logs` ของ certificate entities ในกิจกรรมนั้น
- รองรับ retry และการทำงานต่อหลัง process restart
- ปรับ permission description, generated permission registry, OpenAPI และ generated TypeScript contract

### ไม่อยู่ในขอบเขต

- กู้คืนหรือยกเลิก purge หลังเริ่มแล้ว
- นำเลข activity sequence หรือ certificate number ที่เคยใช้กลับมาใช้ซ้ำ
- ลบบัญชีผู้ใช้ ข้อมูลนักเรียน ข้อมูลบุคลากร หน่วยงาน หรือปีการศึกษา
- ลบ PDF ที่ผู้ใช้ดาวน์โหลดออกไปเก็บในอุปกรณ์หรือระบบภายนอกแล้ว
- ลบข้อมูลจาก backup หรือ infrastructure log ก่อนหมดอายุตาม retention policy ของระบบนั้น
- เก็บประวัติถาวรว่าผู้ใดเป็นผู้ purge เพราะผู้ใช้เลือกให้ audit ของกิจกรรมถูกลบทั้งหมด

## การตัดสินใจที่ยืนยันแล้ว

1. ใช้ hard delete สำหรับข้อมูลกิจกรรมและเกียรติบัตร ไม่ใช้ archived หรือ soft-delete เป็นผลลัพธ์สุดท้าย
2. ลบ audit log ที่เกี่ยวข้อง ไม่เก็บ application-level purge tombstone
3. ใช้ permission เดิม `certificate.delete.school` และ `certificate.delete.organization_unit`
4. ขอบเขต `organization_unit` ใช้ exact owner unit เท่านั้น ไม่สืบทอดไป parent หรือ child
5. หน้าต่างยืนยันต้องแสดง impact และบังคับพิมพ์ชื่อกิจกรรมแบบ exact match
6. ไม่มีระยะพัก 7 วันและไม่มี undo
7. คำขอที่กำลังรอหรือกำลังตรวจไม่ขวางการลบ
8. ใช้ controlled two-phase purge และ durable retry ไม่ใช้ raw parent cascade เป็น public operation
9. ลบ `files`, `file_versions`, `file_derivatives` และ `file_operations` หลังลบ object สำเร็จ
10. เลิกใช้ endpoint ลบ draft เดิม ไม่รักษา backward-compatible delete path

## แนวทางที่พิจารณา

### ลบ parent แล้วพึ่ง `ON DELETE CASCADE`

แนวทางนี้สั้นแต่ไม่สามารถรวม storage provider ไว้ใน database transaction ได้ อีกทั้ง schema ปัจจุบันมี immutable triggers, `ON DELETE RESTRICT`, candidate/certificate cycle, replacement links และ audit metadata ที่ไม่ได้ผูกด้วย foreign key ทั้งหมด การผ่อน guard เพื่อให้ cascade ทำงานทั่วไปจะลดการป้องกันชั้นสุดท้ายของระบบ จึงไม่เลือก

### ซ่อนกิจกรรมแล้วให้ background worker ลบทุกอย่างภายหลัง

แนวทางนี้ทำ request แรกได้เร็วและ retry ง่าย แต่ถ้าไม่มี transaction เริ่มต้นที่ล็อก inventory และ revoke delivery พร้อมกัน อาจมีการเพิ่ม asset หรือออกใบแทรกระหว่างเก็บรายการไฟล์ ทำให้ลบไม่ครบ จึงไม่เลือกเป็นสถาปัตยกรรมเดี่ยว

### Controlled two-phase purge — เลือกใช้

ระยะแรกล็อก campaign, ตรวจสิทธิ์และ impact, บันทึก inventory ไฟล์, เปลี่ยนสถานะเป็น `purging` และขอ File Platform ลบไฟล์ภายใน transaction เดียวกัน จากนั้น worker ใช้กลไก provider/reconciler เดิมจนยืนยันว่า object หายครบ แล้วจึงเรียก finalizer ที่ลบ domain rows, audit rows, file metadata และ purge job ใน transaction สุดท้าย

วิธีนี้ทำให้การเข้าถึงถูกเพิกถอนทันที มี durable recovery และยังคง immutable guard สำหรับคำสั่งลบอื่นทั้งหมด

## สถานะและ state machine

เพิ่ม `purging` ใน constraint ของ `certificate_campaigns.status` ผ่าน migration ใหม่ลำดับ `039_certificate_campaign_purge.sql` โดยห้าม client ใช้ endpoint แก้สถานะทั่วไปเพื่อกำหนดค่านี้

```text
draft / active / closed / archived
                |
                | POST purge ผ่านการตรวจครบ
                v
             purging
                |
                | storage object หายครบและ finalizer สำเร็จ
                v
          ไม่มีแถวในฐานข้อมูล
```

ไม่มี transition ออกจาก `purging` กลับไปสถานะเดิม หากลบไฟล์ล้มเหลว campaign ยังคง `purging`; การ retry เดินหน้าต่อเท่านั้น

## Schema ใหม่

### `certificate_campaign_purge_jobs`

เป็น durable state ชั่วคราวระหว่างลบ และถูกลบพร้อม campaign เมื่อ finalization สำเร็จ ฟิลด์หลักประกอบด้วย:

- `campaign_id` เป็น primary key และอ้าง `certificate_campaigns(id) ON DELETE CASCADE` เพื่อให้ job เป็นแถวสุดท้ายที่หายเมื่อ finalizer ลบ campaign
- `status`: `deleting_files`, `failed` หรือ `finalizing`
- `requested_by` อ้างผู้ใช้ด้วย `ON DELETE SET NULL`
- `requested_at`, `updated_at`
- impact snapshot แบบจำนวนเท่านั้น ได้แก่จำนวน template, candidate, request, open request, issued certificate, revoked certificate, logical file และ byte รวม
- `last_error_code` เป็น safe bounded code เท่านั้น ห้ามเก็บข้อความ provider, object key, signed URL, ชื่อผู้รับ หรือข้อมูลจาก Spreadsheet

ไม่มีสถานะ `completed` แบบถาวร เพราะเมื่อสำเร็จต้องไม่เหลือ purge job

### `certificate_campaign_purge_files`

ตรึง inventory ไฟล์ก่อนเปิดงานลบ เพื่อให้ restart แล้วทำต่อได้และป้องกัน file set เปลี่ยนกลางทาง ฟิลด์หลักประกอบด้วย:

- `campaign_id` อ้าง purge job ด้วย `ON DELETE CASCADE`
- `file_id` อ้าง `files(id) ON DELETE CASCADE` และต้องไม่ซ้ำข้าม purge job
- primary key `(campaign_id, file_id)`
- byte/object count snapshot ที่ใช้รายงาน progress โดยไม่เก็บชื่อไฟล์หรือ object key

ความสัมพันธ์กับ `files` ใช้ cascade เฉพาะจากการลบ logical file ที่ผ่าน finalizer แล้ว เพื่อให้ inventory row หายระหว่าง final transaction ได้ ส่วนการลบ `file_versions` และ `file_derivatives` โดยตรงยังถูก guard

## ความเป็นเจ้าของไฟล์

ไฟล์ของ campaign ถูกรวบรวมจาก:

- `certificate_templates.background_file_id`
- `certificate_template_assets.file_id` สำหรับรูปภาพและ font variants
- `certificate_template_file_uploads.file_id` รวม temporary upload ที่ยังไม่ promote

ไฟล์ CSV/XLSX ต้นฉบับไม่อยู่ในรายการ เพราะระบบอ่านใน browser และไม่ได้เก็บต้นฉบับใน File Platform ส่วน PDF เกียรติบัตรสำเร็จรูปก็ไม่มีไฟล์ต่อใบ เพราะ renderer สร้างแบบ ephemeral

ก่อนเพิ่มไฟล์เข้า purge inventory ระบบต้องพิสูจน์ว่าไฟล์มี purpose ของ certificate template และไม่มี consumer นอก campaign อ้างอยู่ หากพบ legacy/inconsistent reference, legal hold หรือการใช้งานข้าม campaign ให้หยุดก่อนเปลี่ยนเป็น `purging` และคืน conflict ที่ปลอดภัย ห้ามลบไฟล์ร่วมจนทำให้กิจกรรมอื่นเสียหาย

ฟอนต์มาตรฐานที่มากับ application ไม่ใช่ไฟล์ของ campaign และไม่ถูกลบ

## API contract

### `GET /api/certificates/campaigns/{campaignId}/purge-impact`

ต้องผ่าน delete permission ตาม scope ของ campaign และ campaign ต้องยังไม่เป็น `purging` ผลลัพธ์ประกอบด้วย:

- campaign ID, exact campaign name และ `updatedAt`
- `templateCount`
- `candidateCount` ซึ่งนับทั้ง active และ soft-deleted rows
- `requestCount` และ `openRequestCount`
- `issuedCertificateCount`
- `revokedCertificateCount`
- `fileCount`
- `totalFileBytes` ซึ่งรวม stored original versions และ derivatives ของ logical files ใน inventory

จำนวน issued และ revoked แยกตามสถานะและไม่ซ้อนกัน `fileCount` นับ logical files ไม่ใช่จำนวน object

### `POST /api/certificates/campaigns/{campaignId}/purge`

รับ body ที่มี:

- `confirmationName` ซึ่งต้องเท่ากับชื่อในฐานข้อมูลทุกตัวอักษรโดยไม่ trim, case-fold หรือ normalize เพิ่ม
- `expectedUpdatedAt`
- expected impact fields ชุดเดียวกับ response ข้างต้น

Backend ไม่เชื่อข้อมูล client ต้อง lock campaign แล้วคำนวณ impact และ ownership ใหม่ใน transaction เดียวกัน หากชื่อไม่ตรงคืน validation error หาก timestamp หรือ impact เปลี่ยนคืน `409 certificate_purge_impact_changed` เพื่อให้ dialog โหลดข้อมูลใหม่

เมื่อเริ่มสำเร็จคืน `202 Accepted` พร้อม status/progress ปัจจุบัน หาก request ซ้ำขณะ campaign กำลัง `purging` และผู้เรียกยังมี delete scope ให้คืนสถานะงานเดิมแบบ idempotent ไม่สร้างงานใหม่

### `GET /api/certificates/campaigns/{campaignId}/purge-status`

เปิดเฉพาะผู้มี delete permission ตรง scope แสดง job status, logical files ทั้งหมด, files ที่ provider ยืนยันว่าหายแล้ว และ safe error code หลัง finalization จะไม่มีทั้ง campaign และ job; frontend ที่เริ่มติดตามงานแล้วตีความ `404` เป็นสำเร็จและกลับหน้ารายการ

ผู้ที่ไม่เคยเข้าถึง campaign หรือไม่มี delete permission ได้ `404` ไม่เปิดเผยว่ามีงาน purge อยู่

### `POST /api/certificates/campaigns/{campaignId}/purge/retry`

เปิดเฉพาะ job สถานะ `failed` และ permission เดิม หากยังมี object เหลือให้สร้าง delete operations รอบใหม่เฉพาะ object ที่ยังไม่ถูกยืนยันว่าหาย โดยต้องไม่สร้างซ้ำเมื่อมี operation `pending`, `leased` หรือ `retryable_failure` อยู่แล้ว แล้วเปลี่ยน job กลับ `deleting_files` หาก object หายครบอยู่แล้วแต่ finalization รอบก่อนล้มเหลว ให้เปลี่ยนกลับ `finalizing` และเรียก finalizer โดยไม่สร้าง delete operation เปล่า

### Endpoint เดิม

ยกเลิก `DELETE /api/certificates/campaigns/{campaignId}` และ service ที่ลบ draft แบบเดิม ทุกการลบใช้ impact + typed confirmation + purge flow เท่านั้น Frontend และ backend ต้อง deploy เป็น contract ชุดเดียวกัน

## Permission model

ไม่เพิ่ม permission code ใหม่ แต่แก้ description และ generated artifacts ให้ตรงความหมายใหม่:

- `certificate.delete.school` ลบถาวรได้ทุก campaign
- `certificate.delete.organization_unit` ลบถาวรได้เฉพาะ campaign ที่ `owner_organization_unit_id` ตรงกับหน่วยงานที่ผู้ใช้ได้รับสิทธิ์โดยตรง

campaign ระดับโรงเรียนที่ owner เป็น null ต้องใช้ scope `school` การมี read/update/issue permission อย่างเดียวไม่เพียงพอ

ทุก endpoint ตรวจทั้ง permission code และ resource scope ใน backend ห้ามใช้ frontend capability เป็น security boundary

## การเริ่ม purge

ภายใน transaction แรก service ทำตามลำดับ lock เดียวกับ issuance:

1. lock campaign `FOR UPDATE`
2. ตรวจ permission และ exact owner scope หลังได้ lock
3. หาก campaign เป็น `purging` และมี job ที่ถูกต้อง ให้คืนสถานะงานเดิมแบบ idempotent; หากมีสถานะโดยไม่มี job ให้หยุดด้วย integrity error
4. ตรวจ `confirmationName`, `expectedUpdatedAt` และ impact ที่คำนวณใหม่
5. lock template/asset/file relations ที่เกี่ยวข้องและสร้าง frozen file inventory
6. ตรวจว่าไม่มีไฟล์ร่วม, legal hold หรือ reference ที่ไม่รู้จัก
7. สร้าง purge job สถานะ `deleting_files`
8. เปลี่ยน campaign เป็น `purging`
9. เรียก File Platform `request_delete_in_transaction` สำหรับทุก file ID ใน inventory
10. commit แล้วกระตุ้น immediate provider deletion โดยไม่ผูกอายุ HTTP request ไว้กับการลบทั้งหมด

การ request delete จะเปลี่ยน logical file เป็น `delete_requested` ก่อน ทำให้ endpoint ออก grant ใหม่และ delivery ปกติปฏิเสธไฟล์ทันที จากนั้น File Platform ลบ original versions และ derivatives ด้วย durable `delete_object` operations

## การกัน race และ mutation หลังเริ่มลบ

ทุก certificate mutation ที่อาจเปลี่ยน campaign, template, asset, candidate, request, issuance, revocation หรือ replacement ต้อง lock campaign ก่อนและปฏิเสธสถานะ `purging` ใน transaction เดียวกัน ห้ามตรวจ owner/status ก่อน transaction แล้วค่อยเขียนภายหลัง

ผลของ race ระหว่าง issuance กับ purge คือ:

- หาก issuance ได้ campaign lock ก่อน จะ commit จำนวนใบใหม่ก่อน purge คำนวณ impact ทำให้ impact ไม่ตรงและ purge คืน 409
- หาก purge ได้ lock ก่อน campaign จะเป็น `purging` และ issuance ถูกปฏิเสธก่อนจองเลข

template upload/attach และ generic file delete policy ต้องประสานผ่าน file/domain locks ในลำดับคงที่ เพื่อไม่ให้ไฟล์ถูก attach หลังถูกใส่ inventory หรือถูก purge ระหว่างมี consumer ใหม่

## การซ่อนและเพิกถอนการเข้าถึง

ทันทีที่ transaction แรก commit:

- campaign ไม่ปรากฏใน list/detail ปกติ ยกเว้นมุมมอง progress ของผู้มี delete permission
- หน้าเกียรติบัตรส่วนตัวของนักเรียนและบุคลากรไม่คืนใบใน campaign นี้
- public verification ด้วย QR หรือเลขพร้อมชื่อ/นามสกุลคืนผลแบบไม่พบ
- render manifest, preview จากข้อมูลจริง, admin download และ public download ถูกปฏิเสธ
- endpoint grant ไฟล์ใหม่ถูกปฏิเสธเพราะ file lifecycle ไม่ใช่ `ready`

signed grant ที่ออกไปก่อนหน้าอาจใช้ได้จนหมด TTL หาก provider ยังลบ object ไม่เสร็จ ระบบจึงเริ่ม provider deletion ทันทีหลัง commit แต่ไม่อ้างว่าสามารถเรียกคืนไฟล์ที่ download ไปแล้วได้

public และผู้ไม่มี delete permission ได้ response แบบไม่เปิดเผยรายละเอียด campaign ส่วนผู้มี delete permission ใช้ purge status endpoint แทน detail/editor ปกติ

## File Platform reconciliation

ใช้ provider adapter, lease, bounded backoff และ terminal retry ของ File Platform เดิม ไม่สร้างระบบลบ object แยกอีกชุด File cleaner ต้องเรียก certificate purge finalizer หลัง immediate deletion attempt และหลังแต่ละ tenant reconciliation pass

object ที่ provider ตอบว่าไม่มีอยู่แล้วถือว่าลบสำเร็จและ metadata storage status ต้องถูกทำให้เป็นสถานะยืนยันการลบก่อน finalization งานจะเข้าสู่ `failed` เมื่อ delete operations ที่เกี่ยวข้องถึง terminal failure โดยเก็บเฉพาะ safe error code

เมื่อ object ครบทุกตัวถูกยืนยันว่าหาย worker เปลี่ยน job จาก `deleting_files` เป็น `finalizing` ใน transaction สั้นแยกต่างหาก แล้วเรียก finalizer การ commit สถานะนี้ก่อนทำให้ process restart แล้วรู้ว่าต้องเรียก finalizer ซ้ำได้อย่าง idempotent

process restart โหลด purge jobs ในสถานะ `deleting_files`, `failed` หรือ `finalizing` ได้ งาน `deleting_files` อาศัย due file operations ทำต่อ ส่วน `finalizing` เรียก finalizer ซ้ำ

หาก finalizer ล้มเหลวหลัง rollback worker พยายามเปลี่ยน job เป็น `failed` ด้วย safe database error code ใน transaction ใหม่ หากฐานข้อมูลยังไม่พร้อมจนบันทึกสถานะไม่ได้ job คงเป็น `finalizing` และถูกลองซ้ำหลัง restart โดยไม่สูญเสียข้อมูล

## Guard สำหรับ hard delete

migration ใหม่ไม่ถอด immutable protection ทั่วระบบ แต่เพิ่มหรือปรับ trigger ต่อไปนี้ให้ยอมเฉพาะ finalizer ที่มี guard ตรง campaign/file inventory:

- trigger ใหม่ที่ป้องกันการ hard-delete `certificate_campaigns` นอก finalizer
- `prevent_certificate_delete`
- `enforce_certificate_snapshot_immutability` เฉพาะการคลาย replacement links ก่อนลบ
- delete trigger ของ `certificate_issue_request_items`
- delete trigger ของ `certificate_issue_run_problems`
- trigger ใหม่ที่ป้องกันการ hard-delete logical row ใน `files` นอก finalizer
- `file_versions_prevent_deletion`
- `file_derivatives_prevent_deletion`

finalizer เป็น database function เดียวที่ migration สร้างขึ้น Service เรียก function นี้แทนการกระจาย `DELETE FROM` ไว้ใน Rust และ function ตั้ง transaction-local guard ภายในหลังตรวจครบว่า:

- campaign เป็น `purging`
- purge job ของ campaign อยู่สถานะ `finalizing`
- file ID อยู่ใน frozen inventory ของ job นั้น
- original และ derivative objects ทุกตัวถูก provider ยืนยันว่าลบหรือไม่มีอยู่จริง

คำสั่ง `DELETE` ปกติที่ไม่มี guard หรืออ้างคนละ campaign/file ยังถูก trigger ปฏิเสธเหมือนเดิม จึงไม่เปลี่ยน immutable contract สำหรับโมดูลอื่น และยังรักษากติกา static architecture ที่ห้าม application service hard-delete File Platform rows โดยตรง

## Finalization transaction

เมื่อทุก object หายแล้ว finalizer lock campaign, purge job และ file rows ซ้ำ ตรวจ condition ทั้งหมดอีกครั้ง แล้วทำงานใน transaction เดียว:

1. ยืนยันว่า job เป็น `finalizing`
2. ตรึง ID ของ template, asset, candidate, request, run, certificate และ file ที่ต้องลบ
3. เปิด purge guard เฉพาะ campaign นี้ภายใน transaction
4. ลบ audit rows ที่เป็น certificate entity และผูก campaign ผ่าน entity ID หรือ `metadata.campaignId`
5. คลาย cyclic references ภายใน candidate/certificate replacement graph ภายใต้ guard
6. ลบ issue run problems, candidate locks และ request items
7. ลบ certificates, issue runs และ issue requests
8. ลบ candidates และ import batches
9. ลบ template assets, template file upload relations และ templates
10. ลบ file operations
11. ตั้ง `files.current_version_id` เป็น null แล้วลบ derivatives และ versions ภายใต้ file purge guard
12. ลบ logical files; inventory rows cascade หายตามไฟล์
13. ลบ campaign; purge job ที่อ้าง campaign หายเป็นลำดับสุดท้าย

หากขั้นใดล้มเหลว transaction ทั้งก้อน rollback Campaign และ job จึงยังอยู่ใน `purging`/`finalizing` และ worker เรียกซ้ำได้ ไม่มีสภาวะที่ certificate rows หายแต่ file metadata เหลือจาก finalization บางส่วน

การ implement ต้องเลือก deletion statements แบบ explicit และตรวจ affected row counts ไม่อาศัย raw `DELETE campaign` cascade เพียงคำสั่งเดียว แม้ foreign key ภายในบางกลุ่มจะยังใช้ cascade เป็นรายละเอียดได้

## Audit และข้อมูลที่เหลือ

ลบ audit entity types ที่เกี่ยวข้อง ได้แก่ `certificate_campaign`, `certificate_template`, `certificate_candidate`, `certificate_issue_request` และ `certificate` โดยใช้ frozen entity IDs และ `metadata.campaignId` เพื่อครอบคลุม audit ที่ entity ID เป็น child ID

ไม่สร้าง durable “campaign purged” audit row เพราะจะขัดกับการตัดสินใจลบ audit ทั้งหมด `requested_by` อยู่เพียงใน temporary purge job และหายเมื่อสำเร็จ

หลัง purge สำเร็จยังคงมี:

- `certificate_academic_year_counters` และค่า `next_activity_sequence` เดิม
- users, student/staff records, academic years และ organization units
- application built-in fonts/assets
- aggregate infrastructure metrics/logs และ backups จนหมด retention
- สำเนา PDF ที่เคยถูก download ออกนอกระบบ

ไม่มี certificate number tombstone แต่เลขเดิมไม่ถูก reuse เพราะ academic-year counter เดินหน้าเท่านั้น Public verification จึงตอบ “ไม่พบ” ไม่ใช่ “ถูกลบ”

## หน้าจอ

### Danger zone

หน้าภาพรวม campaign แสดง “ลบกิจกรรมถาวร” เฉพาะผู้มี delete capability ที่ backend คำนวณตาม exact scope เมื่อกดให้โหลด `purge-impact` ใหม่ทุกครั้ง

Dialog แสดง:

- ชื่อกิจกรรม
- จำนวนแบบ
- จำนวนรายการผู้รับทั้งหมด
- จำนวนคำขอทั้งหมดและคำขอที่ยังเปิดอยู่
- จำนวนเกียรติบัตรที่ยังใช้ได้และที่เพิกถอนแล้ว
- จำนวนไฟล์และขนาดรวมแบบอ่านง่าย
- คำเตือนว่าเลข/QR จะตรวจไม่พบและไม่สามารถเรียกคืน PDF ที่ดาวน์โหลดไปแล้ว

ช่องยืนยันต้องตรง exact campaign name จึงเปิดปุ่ม destructive action แต่ backend ตรวจซ้ำเสมอ

### Progress และ failure

หลังรับ `202` หน้าแสดงขั้น:

1. ปิดการเข้าถึงกิจกรรม
2. ลบไฟล์ `x / y`
3. ลบข้อมูล
4. สำเร็จและกลับหน้ารายการ

ผู้ใช้ปิดหน้าได้ งานยังเดินต่อ เมื่อกลับมา ผู้มี delete permission เห็น campaign ที่กำลังลบในมุมมอง progress และเปิดดูสถานะได้ ผู้ใช้ปกติไม่เห็นรายการนี้

หากล้มเหลว แสดงข้อความทั่วไปและ safe error code โดยไม่แสดง object key, signed URL, provider response หรือข้อมูลผู้รับ มีปุ่ม “ลองลบต่อ” แต่ไม่มีปุ่มยกเลิก/กู้คืน

## Error contract

- `404` เมื่อ campaign ไม่มีอยู่ ผู้เรียกไม่มี scope หรือ campaign ถูก finalization แล้ว
- `409 certificate_purge_impact_changed` เมื่อ timestamp/counts ไม่ตรง
- `409 certificate_purge_file_shared` เมื่อพบ reference นอก campaign
- `409 certificate_purge_file_held` เมื่อพบ legal hold
- `409 certificate_campaign_purging` สำหรับ mutation ปกติหลังเริ่ม purge
- `422 certificate_purge_confirmation_mismatch` เมื่อชื่อยืนยันไม่ตรง

provider ที่ยังใช้งานไม่ได้ไม่ทำให้ request เริ่ม purge สูญหาย เพราะ transaction แรกบันทึก durable operations ก่อนติดต่อ provider หลังมี job แล้วให้รายงาน storage failure ผ่าน purge status เท่านั้น ส่วน database outage ก่อน commit ใช้ infrastructure error contract ปกติและต้องไม่ทิ้ง campaign ไว้ในสถานะครึ่งเริ่ม

ข้อความ API และ log ต้องไม่รวมชื่อผู้รับ ค่า custom column, QR proof plaintext, object key หรือ signed URL และยังคงข้อห้ามเก็บ/log plaintext national ID ของโครงการ

## Contract และ compatibility

- migration ใหม่คือ `039_certificate_campaign_purge.sql`; ห้ามแก้ migrations `035`–`038` ที่ใช้แล้ว
- เพิ่ม API models และ route ผ่าน generated OpenAPI/TypeScript workflow
- แก้ permission descriptions ใน source/migration แล้ว regenerate registries ตาม `.rules`
- ลบ frontend call และ backend route ของ draft delete เดิม
- frontend และ backend รุ่นนี้เป็น contract เดียวกัน ไม่มี compatibility shim สำหรับ endpoint เก่า
- campaign status `purging` เป็นสถานะระบบและ UI editor/status selector ต้องไม่เสนอให้ผู้ใช้เลือก

## การทดสอบ

รันชุดทดสอบทีละชุดและใช้ concurrency หนึ่งตามคำขอของผู้ใช้

### Migration และ database guards

- migration ใหม่ใช้กับ tenant schema ใหม่และ schema ที่มีข้อมูลจาก `035`–`038`
- direct delete certificate, request item, run problem, file version และ derivative ยังถูกปฏิเสธ
- guarded finalizer ลบ graph ที่มี issued, revoked และ replacement certificates ได้
- purge สำเร็จแล้วไม่มี domain rows, certificate audit rows, purge rows หรือ file metadata rows ของ campaign
- activity counter ไม่ลดและเลข activity เดิมไม่ถูก reuse
- finalizer rollback ทั้ง transaction เมื่อ deliberate fault เกิดกลางลำดับ

### Service และ permission

- school scope ลบได้ทุก owner; organization-unit scope ลบได้เฉพาะ exact owner
- read/update/issue permission อย่างเดียวลบไม่ได้
- typed name mismatch และ impact mismatch ถูกปฏิเสธ
- candidate soft-deleted และ request ทุกสถานะอยู่ใน impact และถูกลบ
- campaign/file ที่มี shared reference หรือ legal hold ถูกปฏิเสธก่อน `purging`
- duplicate purge/retry requests เป็น idempotent

### Concurrency

- issuance ชนะ lock แล้ว purge ได้ 409 พร้อม impact ใหม่
- purge ชนะ lock แล้ว issuance/revocation/template/candidate/request mutation ถูกปฏิเสธ
- owner transfer, template upload/attach และ generic file delete ไม่ข้าม scope หรือทำ inventory stale
- lock order ไม่สร้าง deadlock ภายใต้ concurrent requests

### File lifecycle และ recovery

- provider success ลบ original และ derivatives ก่อน metadata
- provider temporary failure เกิด durable retry และ process restart ทำต่อได้
- terminal failure แสดง `failed`; authorized retry สร้างงานใหม่เฉพาะ object ที่ยังเหลือ
- provider not-found ถูกยืนยันเป็น deleted และ finalization เดินต่อ
- metadata ไม่ถูก hard-delete หาก object ยังไม่ยืนยันว่าหาย
- successful finalization ลบ `file_operations`, derivatives, versions, logical files และ inventory ทั้งหมด

### API และ frontend

- generated OpenAPI/TypeScript/permission contracts ตรงกับ source
- list, detail, own certificates, manual verification, QR verification, preview และ download ซ่อน campaign ตั้งแต่ `purging`
- dialog แสดง impact, exact-name gate, stale-impact reload, progress และ retry ถูกต้อง
- Svelte static checks, `svelte-check` และ autofixer ผ่าน
- focused Playwright ใช้ `--workers=1` และครอบคลุม typed confirmation/progress/failure

### Live certificate lifecycle

ปรับ lifecycle E2E ให้ purge campaign ที่ suite สร้างสำเร็จในตอนท้าย ตรวจว่า QR/เลขค้นไม่พบและไม่มีไฟล์/metadata ค้าง ชุดนี้เป็น destructive test และต้องใช้ isolated tenant กับ dedicated accounts เท่านั้น

## เอกสารปฏิบัติการ

ปรับ `docs/OPERATIONS.md` ให้ระบุว่า certificate campaign purge เป็นข้อยกเว้นแบบ controlled ต่อกฎ “ไม่ลบ File Platform metadata ด้วยมือ” ผู้ปฏิบัติการยังห้ามรัน SQL ลบ metadata เอง ต้องใช้ purge service/finalizer และตรวจ safe counters/error codes เท่านั้น

ปรับ `docs/TESTING.md` ให้เลิกระบุว่า successful certificate lifecycle ต้องเก็บ issued records ไว้เป็น audit history และอธิบาย environment variables/isolated tenant สำหรับ destructive cleanup ให้ชัดเจน

## เกณฑ์ยอมรับ

งานถือว่าเสร็จเมื่อ:

1. ผู้มีสิทธิ์ตาม exact scope ลบ campaign ทุกสถานะผ่าน typed confirmation ได้
2. campaign ถูกซ่อนจากทุก consumer ปกติทันทีหลังเริ่ม purge
3. storage failure ไม่ทำให้ข้อมูลครึ่งลบและสามารถ retry/restart ได้
4. object ต้องหายก่อน hard-delete File Platform metadata
5. หลังสำเร็จไม่เหลือ certificate domain rows, related audit rows, file tombstones หรือ purge job
6. direct/manual delete ที่ไม่ผ่าน guarded finalizer ยังถูก database ปฏิเสธ
7. เลขเกียรติบัตรเดิมตรวจไม่พบและไม่ถูก reuse
8. permission, API, frontend และ operational documentation ตรงกัน
9. verification matrix ตาม `.rules` ผ่านโดยรัน test แบบลำดับเดียว
