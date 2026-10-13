# แผนพัฒนาระบบพฤติกรรมนักเรียน

วันที่ 14 ตุลาคม 2026

อ้างอิง [แบบที่ตกลง](../specs/2026-10-14-student-behavior-design.md) และ [.rules](../../../.rules) แผนนี้จัดลำดับงานเพื่อพัฒนาระบบในอนาคต รอบที่จัดทำแผนนี้แก้เฉพาะเอกสาร ไม่แก้ runtime, schema, permissions หรือข้อมูลโรงเรียนจริง

## ผลลัพธ์ที่ต้องได้

1. ครูเลือกพฤติกรรมทำดี/ทำผิดจากรายการ คะแนนล็อกตามเกณฑ์ มีวันเวลาปัจจุบันที่แก้วันเกิดเหตุได้และรายละเอียดเพิ่มเติม
2. เลือกนักเรียนเป็นคน ห้อง ระดับชั้น และผสมข้ามห้องได้ บันทึกครั้งเดียวแต่แก้ผลผิดแยกรายคนได้
3. คะแนนความดีกับคะแนนที่ถูกหักเก็บแยก แสดงสุทธิ มีรอบปัจจุบัน ประวัติรอบ และยอดสะสมทั้งหมด
4. มาโรงเรียน รายคาบ และรอบพิเศษใช้สถานะ มา/ขาด/สาย/หนีเรียน/ลาป่วย/ลากิจ/กิจกรรม/ยังไม่ได้เช็ค รายคาบและรอบพิเศษแสดงสถานะมาโรงเรียนของแต่ละคน
5. ปิดยอดหลังเที่ยงคืนแล้วบันทึกขาดให้คนที่ยังไม่ได้เช็ก เฉพาะวันและรอบที่นับยอด ทำงานย้อนหลังหลังระบบหยุดได้โดยไม่สร้างผลซ้ำ
6. ตั้งคะแนนตามสถานะและประเภทเช็กชื่อได้ รอบพิเศษตั้งคะแนนเฉพาะรอบได้
7. ครูยกเลิกรายการพฤติกรรมของตนเองได้ คะแนนที่มาจากเช็กชื่อแก้ที่ต้นทาง
8. รีเซ็ตเองหรือรายเดือน/ภาคเรียน/ปีได้ เก็บยอดเก่าเป็นประวัติ เริ่มรอบใหม่ที่ศูนย์ และลบรายละเอียดจริงได้ แยกวัน ฝั่ง และที่มาที่จะล้างได้

ระบบติดตามช่วยเหลือไม่อยู่ในงานนี้

## เจ้าของงานและจุดเชื่อม

| งาน | เจ้าของหลัก | สิ่งที่ต้องประสาน |
| --- | --- | --- |
| สถานะ ผล และการปิดยอด | `school-attendance` | รายชื่อ/ตารางเรียนตามวัน, scan, correction, report, purge/reset |
| เกณฑ์และผลคะแนน รอบและการล้าง | domain พฤติกรรมที่ประเมินในขั้น 0 | Attendance ผ่าน interface แคบ ไม่รวมเป็นโมดูลเดียว |
| งานข้าม domain และ scheduler | application `backend-school` | transaction/outbox, tenant lifecycle, source generation |
| API/permissions/schema | Coordinator เป็นเจ้าของ shared files | domain writers ส่ง DTO/policy/migration requirements ให้รวมตามลำดับ |
| หน้าจอ | `frontend-school` | generated DTO, route metadata, shared PageShell/UI และ feature API |

อ้างอิง implementation ที่มีอยู่: [Attendance models](../../../backend-school/crates/school-attendance/src/models.rs), [sessions](../../../backend-school/crates/school-attendance/src/services/sessions.rs), [reports](../../../backend-school/crates/school-attendance/src/services/reports.rs), [specials](../../../backend-school/crates/school-attendance/src/services/specials.rs), [runtime](../../../backend-school/src/modules/attendance/runtime.rs), [roster projection](../../../backend-school/src/modules/attendance/services/projection.rs), [student services](../../../backend-school/crates/school-students/src/services.rs)

Backend flow ใช้ session/tenant middleware เดิม → permission/resource policy → domain service/transaction → typed response → realtime หลัง commit Frontend ใช้ typed feature API ผ่าน generated contract โหลดหลักผ่าน route loader และรักษา draft ระหว่าง refresh/error

Coordinator owns router composition, OpenAPI composition, permission contract, migration ordering และ generated artifacts หากทำงานขนาน ผู้เขียนแต่ละคนต้องมี branch/worktree และ allowed paths ของตนเองตาม `.rules` แผนนี้ไม่กำหนดว่าต้องเปิด subagent ทุกบทบาท

## ขั้น 0 — ตรวจฐานข้อมูลเดิมและกำหนดขอบเขต cutover

- เริ่มจาก `origin/main` ล่าสุดบน feature branch ตรวจ migration timeline ทั้งหมด ห้ามเลือกเลข migration ล่วงหน้าหรือแก้ migration ที่ใช้แล้ว
- ตรวจสถานะและ summaries เดิม รวม unchecked, generic `leave`, sessions, roster snapshot, module generation, term archive และช่องทางเขียนทั้งหมด
- กำหนดการแยก `leave` เดิมจากข้อมูลที่เชื่อถือได้ ตรวจข้อมูลแบบสรุปที่ไม่มีรายละเอียด หากจับคู่ไม่ได้ให้รายงานเฉพาะจำนวนและ finding code และแก้แผนการย้ายข้อมูลก่อน cutover ห้ามเดา illness/personal หรือทิ้งข้อมูล
- กำหนดวันเริ่มปิดยอดอัตโนมัติและคะแนนอัตโนมัติ แยกจาก historical backfill เพื่อไม่ให้วันเก่าถูกหักโดยปริยาย
- ประเมิน owner ของ domain พฤติกรรมตามเกณฑ์ crate admission ใน `.rules`: interface แคบ dependency ไม่มี cycle lifecycle ต่างจาก Attendance และ focused coverage หากสร้าง crate ใหม่ต้องเก็บ compile-scope evidence ตามมาตรฐานก่อน/หลัง หากไม่ผ่านให้คง cohesive application module และบันทึกเหตุผล
- ยืนยันวิธีคืนสภาพและ binary/schema compatibility สำหรับสถานะใหม่และการล้างข้อมูล กำหนดจุดที่ห้ามใช้ binary เดิมและวิธี roll forward เมื่อจำเป็น
- ระบุ shared owners และ working agreement ก่อนเริ่ม implementation รวมข้อมูล สิทธิ์ contract/state flow และตรวจที่ต้องผ่าน

ผลส่งมอบ: mapping/cutover ที่ตรวจได้ owner ชัดเจน และไม่มีการเดาสถานะเดิมก่อน consumer switch

## ขั้น 1 — โครงสร้างข้อมูล สิทธิ์ และสัญญา API

- เพิ่ม schema ผ่าน forward migrations ตามลำดับที่ว่าง ณ เวลาเริ่มงาน ใช้ UUID เดิมของนักเรียนและ UUIDv7 สำหรับ persisted business IDs ใหม่
- รองรับรายการพฤติกรรม เกณฑ์แบบมีเวอร์ชัน batch/recipient ผลคะแนนต่อคน รอบคะแนนที่แยกฝั่งได้ ยอดรายวัน/ประวัติ และคำสั่งปรับยอดประวัติ
- กำหนดสถานะของแต่ละผลให้ปัจจุบัน ประวัติ หรือถูกลบมีเจ้าของเพียงชุดเดียว รายงานรายเดือน/ภาคเรียน/ปีเป็น rollup ไม่สร้างผลคะแนนซ้ำ
- เพิ่ม source-change intent/replay receipts และ deletion suppression metadata ที่จำเป็น ไม่เก็บรายละเอียดเหตุการณ์ที่สั่งล้างไว้ใน metadata
- เพิ่ม progress งานปิดยอด งานรีเซ็ต และ purge jobs รองรับ idempotency/revision/concurrency และ bounded batches
- ออกแบบ validation สำหรับคะแนนจำนวนบวกโดยเลือกฝั่ง ไม่รับ actor/tenant/คะแนนที่ล็อกจาก client เป็น authority ระบุวันเวลา เขตเวลา และ academic context แบบ typed
- เพิ่ม permissions ผ่าน `contracts/permissions.json` และ migration/grants ที่เกี่ยวข้อง ใช้ scopes เดิมของ repository แยกอ่าน บันทึก ยกเลิกตนเอง/โรงเรียน ตั้งค่า ปรับประวัติ และรีเซ็ต/ล้าง
- เพิ่ม typed DTO/OpenAPI และ generated permission/API artifacts พร้อม policy tests การเลือกหลายคนต้อง authorize ทุกคน

ผลส่งมอบ: schema/contracts/policies รองรับทั้ง allowed และ denied paths และผลหลาย write ที่เป็นการตัดสินใจเดียวกัน commit/rollback พร้อมกัน

## ขั้น 2 — ปรับเช็กชื่อและปิดยอดหลังเที่ยงคืน

- เปลี่ยนสถานะทั้งสามประเภทให้ตรงชุดใหม่ อัปเดต validation, DB constraints, reports, notifications, UI mappings และ summaries โดยรักษาข้อมูลเดิมตามแผนขั้น 0
- เพิ่มการอ่านสถานะมาโรงเรียนของ roster เป็น batch ภายใต้สิทธิ์เดิม ไม่ดึงข้อมูลทีละคนหรือเผยผลของนักเรียนที่ไม่มีสิทธิ์อ่าน
- แสดงสถานะมาโรงเรียนในรายคาบ/รอบพิเศษเป็นข้อมูลประกอบ โดยผลที่ครูเลือกยังเป็น draft แยกจากข้อมูลนี้
- ใช้ expected roster ตามวันที่จาก academic providers/session snapshot และวันนับยอดจริง ไม่สร้าง sparse record จากการเปิดหน้าจอ
- Finalizer ตรวจวันที่ครบกำหนดใน Asia/Bangkok หลัง 00:00 บันทึก absent เฉพาะผู้ไม่มีผลในรอบที่มีหน้าที่เข้าร่วมและนับยอด เก็บ origin อัตโนมัติ ไม่สร้าง skipped/หนีเรียนแทน
- ใช้ lock/revision เพื่อไม่เขียนทับครูที่กำลังบันทึก เก็บ completion/progress ในฐานข้อมูล ไม่ใช้ in-memory digest flag เป็นหลักฐานว่าปิดยอดแล้ว
- รองรับ startup registration และ bounded catch-up ทุกวันที่พลาดหลังวันเปิดใช้ รวมวันสุดท้ายของ term ก่อนการปิด/เก็บข้อมูล โดยไม่ต้องเปิด UI และไม่ poll idle tenants โดยไม่มีขอบเขต
- การแก้ย้อนหลัง/ยกเลิกรอบเผยแพร่ durable source change ที่ตรวจซ้ำได้ เพิ่มกติกาปิดยอดใหม่ใน `.rules` พร้อม code/tests ในขั้นนี้

ผลส่งมอบ: ผลเช็กชื่อในระบบเดียวกันถูกต้องก่อน/หลัง cutoff และ restart ไม่สร้างขาดหรือ score intent ซ้ำ

## ขั้น 3 — เกณฑ์คะแนนและการเชื่อม Attendance

- เพิ่มรายการทำดี/ทำผิดและเกณฑ์ที่มีผลตามวัน ครูเลือกจำนวนเองไม่ได้ เก็บ rule version/amount ที่ใช้จริง ค่าคะแนนจริงเป็นข้อมูลที่โรงเรียนตั้ง ตัวอย่างในแชตไม่เป็นเกณฑ์ production
- เพิ่มเกณฑ์กลาง/ระดับชั้นที่มีลำดับชัดเจน แยกมาโรงเรียนกับรายคาบ รองรับเพดานรายวันและนโยบายการนับซ้ำ
- เพิ่มเกณฑ์รอบพิเศษใน workflow สร้างรอบ เลือกค่าเริ่มต้นหรือกำหนดเองแต่ละสถานะ เก็บ snapshot ต่อรอบ และใช้สิทธิ์ตั้งเกณฑ์คะแนนแยกจากสิทธิ์สร้างรอบ
- Default integration เป็นไม่คิดคะแนนจนกำหนดเกณฑ์ ให้ทุกสถานะเลือก none/merit/deduction ได้อย่างใดอย่างหนึ่ง ยังไม่ได้เช็คไม่คิดคะแนนระหว่างวัน
- Application adapter รับ source changes ผ่าน durable delivery ที่เขียนใน transaction ของ Attendance มี retry/reconciliation ไม่ใช้ best-effort callback เป็นทางเดียว
- Daily effect ใช้ผลเดียวต่อคน/วัน/term ส่วน lesson/special ใช้รอบ/คน Replay key ครอบคลุม tenant, module generation และ source revision ป้องกันการชนเมื่อ Attendance reset แล้วสร้าง deterministic session ID เดิม
- ครอบคลุม teacher save, correction, face scan, midnight finalizer, cancellation, count overrides, purge และ reset การแก้ผลในรอบเปิดถอนผลเดิมแล้วใช้ผลใหม่ ไม่แสดง committed write ว่าล้มเหลวเพราะ realtime ส่งไม่สำเร็จ
- กำหนด precedence ผลมาโรงเรียนและการหักรายวัน/คาบซ้ำอย่างชัดเจน เมื่อ daily outcome เปลี่ยนต้อง reevaluate ผลรายคาบที่ได้รับผลจากนโยบายนี้
- Source purge ที่เป็น retention ต้องไม่ cascade ลบคะแนนและต้องรอ delivery ครบ Source reset แสดงผลกระทบต่อคะแนนและใช้ reconciliation ที่ระบุไว้

ผลส่งมอบ: เกณฑ์ที่ใช้ตรวจย้อนหลังได้ ทุกช่องทางให้ผลเท่ากัน การส่งซ้ำและแก้ต้นทางไม่ทำให้คะแนนเพิ่ม/หักซ้ำ

## ขั้น 4 — หน้าบันทึก ประวัติ และรายงาน

- เพิ่มหน้าบันทึกตามลำดับ ประเภท → รายการ → นักเรียน → วันเวลา/รายละเอียด → สรุปและบันทึก มี field errors และรักษา draft เมื่อ validation/network/conflict ล้มเหลว
- นักเรียนเลือกทั้งระดับ ห้อง หรือรายคนผสมได้ ใช้ student UUID เป็น selection key รวมซ้ำ รักษาการเลือกข้าม paging/filter และระบุ select-all scope กับจำนวนชัดเจน
- ตรวจ membership ตามวันเกิดเหตุและสิทธิ์ก่อน commit หากชุดรายชื่อ/เกณฑ์เปลี่ยนให้ครูตรวจใหม่ ไม่บันทึกบางคนหรือจำนวนคะแนนใหม่เงียบ ๆ
- บันทึกแบบกลุ่มใน transaction และ replay-safe request หนึ่งครั้ง มีผลรายคนสำหรับยกเลิกบางคน/ทั้งกลุ่ม
- เพิ่ม profile summaries ปัจจุบัน/ประวัติ/สะสม แยก merit/deduction และสุทธิจากช่วงเดียวกัน ไม่แทน failed read ด้วยศูนย์
- เพิ่มรายงานค้นหา/filter/pagination และรายการที่ฉันบันทึก แสดงที่มา ผู้บันทึก รายละเอียดและ cancellation reason ที่ยังไม่ได้ purge
- ปุ่มยกเลิกใช้ exact capability และ server ownership ผู้บันทึกจาก session ไม่ใช้ client actor ครูยกเลิกของตัวเองได้ ผู้ดูแลแก้ข้ามผู้บันทึกได้ตามสิทธิ์ การยกเลิกซ้ำไม่สร้าง reversal ซ้ำ
- Attendance-origin rows เปิดต้นทางเพื่อแก้ตามสิทธิ์ ไม่มี manual cancellation ใน Behavior หลังเก็บประวัติแล้วแสดง read-only และใช้การปรับยอดประวัติสำหรับผู้มีสิทธิ์
- ใช้ PageShell/PageHeader, shared UI/theme และ route loading patterns เดิม ดู loading/empty/error/pending แยก region lazy-load settings/export/action-only data และ reconcile เฉพาะ region หลัง mutation

ผลส่งมอบ: ครูทำงานรายคน/กลุ่มและแก้ความผิดพลาดได้บนมือถือ/desktop โดยสิทธิ์และยอดไม่เปลี่ยนตามการค้นหา/เปิดหน้า

## ขั้น 5 — รีเซ็ต เก็บยอด และลบรายละเอียดจริง

- เพิ่ม preview ที่ระบุขอบเขตวัน/ปี/term ฝั่ง ที่มา จำนวนคน/รายการ และผลต่อคะแนนปัจจุบัน/ประวัติ/สะสม Require reason และ revision/preview token ที่ผูกกับขอบเขตและ actor
- รองรับล้างรายละเอียดเก็บผลไว้ในชุดเดิม ย้ายผลไปประวัติแล้วล้างรายละเอียด และลบรายละเอียดพร้อมผลคะแนน ตรวจยอดก่อนลบ ไม่ใช่เพียง soft-delete ซ่อนหน้าจอ
- เก็บ aggregates ต่อคน/วัน/ฝั่ง/รอบ/ที่มาที่จำเป็น หลัง archive ยังเลือกจัดการบางวัน/ที่มาได้ เก็บอย่างละผลเพียงครั้งเดียว ไม่เก็บคำอธิบายเหตุการณ์ที่ล้างไว้ใน JSON หรือ audit
- Serialize purge/reset กับ scoring/cancellation/finalization ที่เกี่ยวข้อง สำหรับ batch ใหญ่ใช้ durable jobs และ progress ที่ retry ได้ ไม่ประกาศ completed จน reconcile/cleanup สำเร็จ
- Snapshot/reconcile → ลบรายละเอียดที่ได้รับอนุญาต → เปลี่ยนเจ้าของผลและเปิดรอบใหม่ โดย reader ไม่เห็นผลซ้ำหรือช่วงยอดหาย Retry ไม่ archive ยอดซ้ำ
- Suppression/checkpoints ป้องกัน source replay/late correction ในขอบเขตที่ล้างหรือปิดแล้วสร้างคะแนนกลับมาในรอบใหม่ ปรับประวัติผ่านคำสั่งแยกพร้อมเหตุผลและ audit
- ตั้ง reset แบบ manual/month/term/year แยกต่อฝั่งได้ รายเดือนรอ finalization ของวันสุดท้ายและ score delivery ครบ ใช้เวลาโรงเรียนและ durable checkpoint จับงานที่ค้างหลัง restart
- ทดสอบ split reset: รีเซ็ตความดีอย่างเดียวไม่เปลี่ยนคะแนนหัก ช่วงสุทธิใช้ date range เดียวกันและแสดงขอบเขตแต่ละฝั่ง
- แยก Attendance retention/reset กับ Behavior cleanup ไม่ลบนักเรียน/เกณฑ์/ผลเช็กชื่อผ่านการล้างคะแนน สิทธิ์ล้างเป็นของผู้ดูแล ไม่ให้สิทธิ์ครูยกเลิกตนเองครอบคลุม purge
- บันทึก recovery/retention/impact ของการลบจริงใน owner `docs/OPERATIONS.md` ไม่ใส่ maintenance ที่ทำให้ระบบหยุดใน user flow และไม่สั่ง shrink/vacuum full ระหว่างคำขอผู้ใช้

ผลส่งมอบ: ตัวอย่าง 20 → reset เป็นปัจจุบัน 0/ประวัติ 20 → หักใหม่ 5 เป็น 5/20/25 ถูกต้อง แม้ retry/restart และตรวจได้ว่ารายละเอียดที่เลือกถูกลบจริง

## ขั้น 6 — ตรวจครบและเปิดใช้

ใช้ [Testing](../../TESTING.md) เป็นเจ้าของคำสั่งและ fixtures และ verification matrix ใน `.rules` ใช้ pipeline local runner เป็น flow หลักพร้อม coverage ที่เพิ่มตามพฤติกรรม ไม่เพิ่มชุดทดสอบลง GitHub Actions

| กลุ่ม | Acceptance ที่ต้องตรวจ |
| --- | --- |
| Migration | identities/counts/relationships คงเดิม, leave mapping, refusal เมื่อจับคู่ไม่ได้, retry, expanded-schema compatibility และ cleanup gate |
| ปิดยอด | ก่อน/หลัง Bangkok midnight, holiday/override, canceled/ไม่มีคาบ, roster ตามวัน, multi-day downtime, วันสิ้นเดือน/term, race กับครู, retry ไม่ซ้ำ |
| เกณฑ์/เชื่อม | ทุกสถานะ/ประเภท/action, round-specific rules, future versions, scan/save/finalizer, daily canonical result, caps/duplicate policy, module reset generation, outbox retry |
| บันทึกกลุ่ม | ห้อง+ข้ามห้อง, คนซ้ำ, select-all ข้ามหน้า, date/rule/roster conflict, unauthorized member, all-or-nothing rollback, ambiguous network retry |
| ประวัติ/สิทธิ์ | own/assigned/school union, ครูยกเลิกตนเอง/ปฏิเสธของผู้อื่น, cancel ซ้ำ, source-only correction, tenant isolation, ปิดรอบแล้วแก้ด้วยสิทธิ์ปรับประวัติ |
| รีเซ็ต/ล้าง | ปัจจุบัน+ประวัติ=สะสม, single-side reset, day/source filtering หลัง archive, detail-only vs archive vs delete-effects, stale preview, failed reconciliation ไม่ลบ, source replay ไม่คืนคะแนน |
| Browser | mobile/desktop light/dark, keyboard/touch, target size, narrow status controls, daily badges, draft retention, late read rejection, independent regions, loading/empty/error/pending |

ก่อน ready PR ต้องผ่าน generated permission/API consistency, focused domain/policy/service/database tests, pipeline owner suites, affected Playwright และ source/migration/cutover review ที่เกี่ยวข้อง Fixtures เป็นข้อมูลสังเคราะห์และฐานข้อมูล Docker ที่เป็นเจ้าของ ไม่ใช้ tenant จริง

การเปิดใช้ดำเนินตาม [Operations](../../OPERATIONS.md): exact candidate และ CI builds, shared production lock, Maintenance Mode, canonical tenant migrations, permission/menu sync, readiness และ authenticated deployed acceptance ต้องครบก่อนประกาศพร้อมใช้ ไม่ถือว่าการจัดทำแผนหรือ static checks เป็นหลักฐานว่าฟีเจอร์ใช้งานได้แล้ว

บันทึกผลตรวจและ outcome ใน implementation PR เมื่อเสร็จแล้วจึงนำ workflow spec/plan นี้ออกตาม `.rules` โดย Git history เก็บที่มาของแบบ

## ลำดับส่งมอบ

ขั้น 0 → 1 → 2 → 3 → 4 → 5 → 6 แต่ละขั้นส่งมอบขอบเขตตรวจได้และใช้ interface ที่ตกลงร่วมกัน งาน UI draft หรือ permission planning ทำคู่กับงาน domain ได้หลัง contract ชัดเจน Shared migrations/generation และ integration ต้องรวมโดยเจ้าของเดียว ไม่เปิด partial scoring/reset ให้ผู้ใช้ก่อน prerequisites ของขอบเขตนั้นครบ

ไม่กำหนดวันเสร็จหรือระยะเวลาโดยไม่มีข้อมูลทีม ปริมาณข้อมูล และผลตรวจ mapping เดิม ใช้ผลขั้น 0 เพื่อประเมินกำลังและแบ่ง PR ที่ deploy ได้อย่างสอดคล้อง
