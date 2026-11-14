# แบบงานวิชาการจากข้อมูลปัจจุบันชุดเดียว

สถานะ: ทิศทางที่อนุมัติให้พัฒนาในแชตนี้ หลังผู้ใช้ยืนยันย้ายข้อมูลปัจจุบันและลบฉบับเก่า
วันที่: 14 พฤศจิกายน 2569 (2026-11-14) เขตเวลา Asia/Bangkok
รายละเอียด mapping และ migration เป็นงาน audit ที่ต้องผ่านก่อน cutover ไม่ใช่การอนุญาตให้ลบ production ทันที

## ผลลัพธ์ที่ต้องได้

ผู้ใช้เลือกภาคเรียนแล้วแก้ชุดเปิดสอนและตารางเรียนปัจจุบันได้โดยตรง
ตารางสอบมีหนึ่งชุดต่อรอบสอบ กลางภาคและปลายภาคเป็นคนละรอบ
ไม่มี business version ประวัติการแก้ไข draft clone publish หรือการอัปเกรดแหล่งเปิดสอน
ภายในไม่สร้าง snapshot หรือประวัติแฝงเพื่อเลียนแบบขั้นตอนเก่า
revision สำหรับกันการเขียนทับกันยังคงอยู่ โดยไม่เป็นประวัติทางธุรกิจ

[แผนดำเนินงานและรายการหน้าที่กระทบ](../plans/2026-11-14-academic-operations-simplification.md) เป็นขอบเขตงานร่วมของแต่ละ owner
[กติกาโครงการ](../../../.rules) ต้องแก้ประสานกับ implementation ก่อนเปลี่ยน authority จริง

## บริบทและข้อมูลต้นน้ำ

- ระบุโรงเรียน ปี และภาคเรียนจริงทุกคำสั่ง ไม่เดาบริบทจากวันที่
- ชุดปัจจุบันของแต่ละภาคเรียนแยกกัน ไม่ยุบภาคเรียนอื่นหรือปีเก่าทั้งปี
- ทะเบียนรายวิชา ทะเบียนกิจกรรม และฉบับหลักสูตรรักษา identity และ semantics เดิม
- รักษา pinned catalog/curriculum references ที่ยังมีความหมาย ไม่อัปเกรดฉบับต้นน้ำเงียบ ๆ
- ปี ภาคเรียน ห้องประจำ และรายชื่อรายปีมีเจ้าของเดิม ไม่ย้ายมาเป็นลูกของฉบับตารางเรียน
- ภาคเรียนที่ปิดยังมี lifecycle guard การยุบฉบับไม่เปิดช่วงการศึกษากลับเอง
- ไม่เพิ่ม future scheduling หรือ effective-date engine มาทดแทนการเลือกฉบับ

## Delivery เป็นข้อมูลเปิดสอนปัจจุบัน

ใช้ normalized registry ของ Delivery เป็น authority ของรายการเปิดสอน กลุ่ม สมาชิก ครู และเป้าหมาย
ก่อนเลิก snapshot เดิม ต้อง materialize สถานะที่เลือกลงแถวปัจจุบันให้ครบ
public typed provider ของ Delivery ให้ข้อมูล live offering/group/teacher/target รวม weekly target metadata ที่ผู้ใช้ข้อมูลต้องใช้
ผู้ใช้ข้อมูลหาสิทธิ์และรายการจาก provider นี้ร่วมกัน ไม่อ่าน JSON ฉบับเก่าหรือคิด winner เอง

การเพิ่มหรือแก้ข้อมูลบันทึกลง registry ปัจจุบันภายใต้ transaction และ revision check
การเปลี่ยนครู เวลา หรือห้องคง Attendance facts และคะแนนที่มีอยู่ตามปกติ
หากข้อมูลขัดกันให้คืนข้อผิดพลาดที่แก้ได้ ไม่ใช้การลบคะแนนเป็นผลข้างเคียงของการแก้
ตรวจความสอดคล้องกับหลักสูตรที่จุด integration โดยไม่แก้ฉบับหลักสูตร

เจ้าของอ้างอิง: [Delivery services](../../../backend-school/crates/school-academic-delivery/src/services/offerings.rs) และ [ports](../../../backend-school/crates/school-academic-delivery/src/ports.rs)

## Timetable เป็น aggregate เดียวต่อภาคเรียน

สร้าง current timetable aggregate UUID หนึ่งตัวต่อภาคเรียน โดยไม่ผูก Delivery publication/source pin
GET/POST `/api/academic/timetable` เป็นสัญญาปัจจุบันสำหรับอ่านหรือสร้าง aggregate ตามบริบท
block operations ใช้ `timetableId` และ workspace ส่ง `timetable` แทนโครงสร้าง version/draft
ชื่อ public `Draft*` ที่เป็นสัญญาของงานนี้เปลี่ยนเป็น `Timetable*` ให้ตรงความหมายเมื่อ cutover ผู้ใช้ข้อมูล

รักษา collaboration lease การตรวจ concurrent write และการเพิกถอน session ที่มีอยู่
key ของการประสานงานใช้ timetable/term ปัจจุบัน ไม่สร้างฉบับใหม่เพื่อหลบการชน
ยังตรวจครู ห้อง นักเรียนชนกัน รวมความถูกต้องของคาบและวันเรียน
การแก้คืน accepted state ของรายการที่เปลี่ยนและใช้ realtime เดิม invalidate ผู้ใช้ข้อมูลที่เกี่ยวข้อง

หน้าจัดตารางและหน้าที่เคยอ่านรายการ versions ต้องเปลี่ยนไปอ่าน aggregate เดียว
หน้าวันเก่าแสดงข้อเท็จจริงที่ materialize แล้วตามเจ้าของข้อมูล ไม่อ้างว่าสร้างตารางเก่าที่ไม่เคยบันทึกกลับได้
ไม่รักษา origin/source alignment workflow เป็น fallback
อ้างอิงเจ้าของปัจจุบัน: [Timetable services](../../../backend-school/crates/school-academic-timetable/src/services.rs)

## ตารางสอบเป็นข้อมูลปัจจุบันของรอบ

คง round ID และชนิดรอบที่แยกกลางภาค/ปลายภาค
แก้รอบ วัน วิชาสอบ ห้อง ผู้เข้าสอบ และผู้คุมโดยตรงผ่าน CRUD ที่มี revision/validation
เลิก round status/publish และ source-sync ที่เป็นกลไกฉบับของงานนี้
ยังตรวจข้อขัดแย้งของครู ห้อง ผู้เข้าสอบ และข้อจำกัดรอบตามความหมายจริง
เมื่อเอาวิชาออก ห้องรวมและบล็อกกลางที่วิชาอื่นใช้ต้องยังอยู่

อ้างอิง [Exam schedule services](../../../backend-school/crates/school-academic-assessment/src/exam_schedule/services.rs)

## สิทธิ์และขอบเขตคำสั่ง

การแก้โดยตรง intersect สิทธิ์ Manage เดิมกับ resource scope ของรายการเปิดสอน
การลบแบบทำลายข้อมูลเกี่ยวข้องต้องมี authority ระดับโรงเรียน พร้อม preview ผลกระทบและการยืนยันรายการจริง
สัญญาสิทธิ์ที่แน่นอนเป็นผล audit และแก้ผ่าน permission owner; ไม่สร้าง permission code จากเอกสารนี้
backend เป็นผู้ตัดสิน tenant/resource authorization หน้าจอแสดงเฉพาะ capability ที่ยอมรับได้

DTO เริ่มจาก Rust และ OpenAPI ของ owner สร้าง TypeScript contracts ตาม workflow เดิม
ไม่แก้ generated contracts ด้วยมือ และไม่คง endpoint เก่าเป็นผู้เขียนคู่ขนาน
ใช้ Academic HTTP owners หกตัวเดิม ไม่ย้ายกลับไป facade กลาง

## การลบรายการเปิดสอนและข้อมูลเกี่ยวข้อง

Application composition ประกอบคำสั่งลบข้ามโดเมนใน transaction เดียวผ่าน public provider ของแต่ละ owner
Delivery ไม่ import Assessment, Results หรือ Timetable เพื่อทำ orchestration
กำหนดลำดับล็อกและ revision fence เดียวกันสำหรับ save/delete/workers ก่อนเขียนจริง
transport ที่ผลลัพธ์ไม่แน่นอนต้อง reconcile ผลจริง ไม่ replay destructive command โดยไม่มี idempotency ที่พิสูจน์แล้ว

- Delivery ลบรายการ/กลุ่ม สมาชิก และการมอบหมายที่อยู่ในขอบเขตที่ยืนยัน
- Timetable ลบคาบและลูกเฉพาะรายการนั้น ไม่ลบ aggregate ทั้งภาคเรียน
- Assessment ลบโครงสร้างคะแนน คะแนน การประเมินและการยืนยันที่เป็นของรายการนั้น พร้อมลูกตารางสอบเฉพาะส่วน
- Attendance ลบ lesson sessions/records และปรับรายงานหรือ archived summaries ที่ยังมีส่วนร่วมจากข้อมูลนั้น
- Results เอาส่วนร่วมของรายการที่ลบออกจากผลรายวิชา/กิจกรรม GPA หน่วยกิต และยอดภาค/ปีที่บันทึกไว้
- Results ยังรักษาระบบ global revisions/policies/locks ของตน ไม่ยุบทั้งระบบตามฉบับตารางเรียน
- Supervision ตัด operational references ที่หาย โดยรักษาผลประเมินจริงและให้ผู้อ่านรองรับแหล่งที่ถูกลบ
- Behavior กลับเฉพาะ Attendance-derived contributions ของ lesson ที่ลบผ่าน provider เดิม ไม่เปลี่ยน manual Behavior
- Files จัดการ physical cleanup ตาม locator/reference ของตน หากมีไฟล์ภายนอกต้องมีขอบเขตหลัง commit ที่ชัดเจน

ไม่แตะการมาโรงเรียนรายวันหรือ special attendance ที่ไม่ใช่ลูกของวิชา
ไม่ลบครู นักเรียน รายชื่อรายปี ห้องจริง bell periods หรือทะเบียนร่วมเพราะลบรายการเปิดสอน
คำสั่งไม่ย้อนการขึ้นปี ย้ายห้อง จบการศึกษา หรือย้ายออกที่ยืนยันไปแล้ว
Lifecycle ปัจจุบันใช้ direct closure/manual rollover โดยไม่มีเงื่อนไขบังคับ locked Results/GPA/promotion policy

## การเลือกและแปลงข้อมูลเดิม

ทุกภาคเรียนที่ระบุชัดเจน รวมภาคเรียนที่ปิดแล้ว เลือก publication ที่ใช้งานจริง ณ cutover civil date เดียวของ Bangkok ที่จับหลังล็อกต้นทาง
เลือกเลข publication สูงสุดที่ effective_from ไม่เกินวันนั้น วงจรภาคเรียนกำหนดสิทธิ์แก้ไข ไม่ใช้สร้างวันที่ปิดย้อนหลัง
การมีเลขฉบับสูงกว่า draft ล่าสุด หรือ future plan ไม่พิสูจน์ว่าใช้งานจริง
UI ที่เลือก draft เป็นค่าเริ่มต้นไม่ใช่หลักฐานเลือก migration source
ภาคเรียนอื่นต้องคงหนึ่งชุดต่อบริบท ไม่ทิ้งทั้งปีเก่าเพราะไม่ใช่ปี active

ตรวจ timeline 001–155 แล้ว ใช้ migration 156 สำหรับการลบ Results แบบระบุขอบเขต และ 157 สำหรับแปลง authority ปัจจุบัน
กรณีไม่มี publication ที่ถึงวัน cutover อนุญาต draft ที่บันทึกไว้เพียงชุดเดียว หรือ future plan เพียงชุดเดียว เฉพาะ planning/ready ที่ไม่เคย activate
ภาคเรียนว่างจริงคงชุดว่าง หลายแผนที่กำกวมหรือ registry ที่ไม่มีแหล่งแปลงต้องปฏิเสธ ไม่เลือกฉบับใหม่สุดเอง
ใช้ learning_offerings.weekly_period_target และ academic_delivery_state หนึ่งแถวต่อ term พร้อม row_version ไม่มี snapshot
ใช้ academic_timetables ไม่เกินหนึ่งแถวต่อ term รักษา UUID ตารางที่เลือกและ UUID ลูก เปลี่ยน FK เป็น timetable_id
บริบทที่ยังไม่มีตารางที่เลือกคงไม่มี parent จน POST สร้างชุดปัจจุบัน เพื่อไม่ล็อก bell setup ของภาคเรียนว่าง
คาบ Attendance เดิมเปลี่ยน source_key เป็น placement UUID ได้เฉพาะการจับคู่ term/group/period/weekday ที่ตรงเพียงรายการเดียว โดยคง session UUID และ ledger อื่นเดิม
ผลนิเทศที่ snapshot ว่างและต้องตัด FK เก่ารักษาข้อเท็จจริง source UUID กับ observedAt ที่พิสูจน์ได้ใน snapshot เดิม ไม่แต่งชื่อหรือเหตุผล manual
teacher assignment ปัจจุบันคง UUID ที่เลือกและ unique(group,teacher); registration_open เป็นค่าปัจจุบันรายกลุ่ม
การลบกลุ่มจาก archived Attendance ที่รวมไว้ระดับ offering และไม่มีหลักฐานแบ่งกลุ่ม หรือผลรวมรายวิชาที่ไม่มี resultId ระบุกลุ่ม ต้องปฏิเสธเฉพาะกรณีที่แปลงไม่ได้จริง
ก่อนลบ lesson ที่ระบุ UUID แน่นอน migration consume เฉพาะ pending notices แบบ session/correction ของ UUID นั้นเป็น skipped โดยรักษา receipt/inbox/dedup และไม่แตะ arrival/special/digest
queue term/date ที่ได้รับผลด้วย generation ปัจจุบันและ reset=false ให้ Behavior provider เดิมกลับผลที่ถูกลบ โดยไม่แก้ manual หรือกลับทั้งวันแบบ reset
ถ้า archived lesson totals ไม่มี session attribution แต่ยังมี derived Behavior totals หรือผลที่เลือกอยู่ใน cycle/suppression ที่ provider เดิมคุ้มครอง ต้องปฏิเสธด้วย bounded finding แทนการเดาหรือรายงานว่ากลับผลแล้ว
กรณีล้าง Behavior แบบ details_only แต่ Attendance session ยังอยู่ ต้องตรวจจาก roster/records/source snapshot ที่บันทึกจริง: nonzero lesson total ที่ details_purged หรือรายละเอียดที่เหลืออธิบายยอดไม่ได้ต้องปฏิเสธอย่าง atomic แม้ไม่มี Attendance archive; ไม่ใช้สมาชิกกลุ่มปัจจุบันอนุมานผลเดิม
การพิสูจน์ว่าข้อมูลคงอยู่ต้องเลือกตาม retained group สำหรับคะแนน/ผล และ retained session สำหรับ Attendance รวม session ที่ไม่มี group แต่ระบุ retained offering ชัดเจน; facts ของ omitted group ไม่เป็นส่วนของ preservation fingerprint
ถ้าสถานะกำกวมหรือ Delivery/Timetable source graphs เข้ากันไม่ได้ ให้หยุดด้วย bounded finding counts ไม่เดาข้อมูลหรือเผยแถวส่วนตัว

ย้าย offerings/groups/teachers/students/targets ที่เลือกลง normalized current structure ก่อน purge
รักษาข้อเท็จจริง Attendance คะแนน และผลที่ยังเกี่ยวข้อง แม้เดิมอ้าง UUID ของ old version
remap references ที่ถูกต้องและรักษา identity ที่ยังมีความหมายก่อนลบตัวครอบเก่า
ทรัพยากรร่วมไม่ถูกลบเพราะอยู่ใน snapshot; ลูกที่ exclusive ต่อกราฟที่ทิ้งใช้ขอบเขต explicit offering deletion ข้างต้น

## Cutover และการเลิกโครงสร้างเก่า

Reconciliation ต้องยืนยัน counts identities relationships และ semantic invariants ของสถานะที่เลือก
ครอบคลุมข้อมูล current ที่เก็บ การเปลี่ยน reference คะแนน/เช็คชื่อ และข้อมูลที่ตั้งใจลบตามขอบเขต
หลักฐานที่ขาด ล้าสมัย หรือไม่ผ่านบล็อกทั้ง consumer switch และ destructive cleanup

เมื่อผ่านแล้ว cutover ไป authority ปัจจุบันพร้อมเลิกผู้อ่าน/ผู้เขียนเดิมและลบข้อมูล version mechanism ที่หมดหน้าที่
ลบ obsolete operational versions/drafts/snapshots/journals/JSON และ artefacts การเผยแพร่สอบที่ audit ยืนยันว่าเลิกใช้
ไม่เก็บ hidden history tables, soft-deleted history blobs, archive หรือ old-version runtime fallback
ไฟล์ applied SQL migrations คงเดิมเป็น ledger และใช้ forward migrations ตามกติกา
งานค้างและคำขอจากโปรแกรมเก่าต้องถูก fence ไม่ให้ข้อมูลเก่าฟื้นหรือเขียนทับโครงสร้างใหม่

วาง maintenance release และขอบเขตห้ามใช้ binary เก่าหลัง schema cutover ตาม [Operations](../../OPERATIONS.md)
ซ้อมกับ fixture แยกก่อน ไม่มี immediate production purge จากการอนุมัติแบบนี้

## การแจ้งเตือนและการตรวจรับ

เลื่อน delivery messaging จนตกลงเหตุการณ์ ผู้รับ และการรวมข้อความ; ไม่ส่งข้อความภายนอกในระยะนี้
realtime ที่มีอยู่ยังแจ้งให้หน้าปัจจุบันอ่านสถานะใหม่ โดยไม่เป็น authority แทนฐานข้อมูล
Notifications คงเป็นเจ้าของการส่ง หากเพิ่มภายหลังให้โดเมนต้นทางกำหนดความหมายและผู้รับผ่าน provider เดิม

ตรวจ direct save ไม่มีฉบับแฝง, concurrency/session revocation, denied tenant/scope และ teacher permission changes
ตรวจลบเมื่อมี Attendance/คะแนนแล้ว รวม archived summaries ผลรวมภาค/ปี และการรักษาทรัพยากรร่วมในห้องสอบ
ตรวจ mapping สำเร็จ/กำกวม/missing source, dangling references, retries และ no resurrection จาก workers/old requests
แก้ test expectations เฉพาะ approved semantics ที่เปลี่ยน พร้อมรักษา regression coverage ที่ยังเกี่ยวข้อง
ใช้ [Testing](../../TESTING.md) สำหรับ recipes และขอบเขตตรวจ โดยไม่คัดลอกคำสั่งไว้ในแบบนี้
freeze candidate ให้ Reviewer และ QA อิสระตรวจ แล้วบันทึก draft PR checkpoints ก่อน coordinated release ที่ผ่าน gates
