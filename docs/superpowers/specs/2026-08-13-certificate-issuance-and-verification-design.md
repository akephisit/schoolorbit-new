# Certificate Issuance and Verification Design

## เป้าหมาย

เพิ่มระบบออกเกียรติบัตรของโรงเรียนที่รองรับผู้รับสามกลุ่ม ได้แก่ นักเรียนในระบบ บุคลากรในระบบ และบุคคลภายนอก ระบบต้องออกเลขเกียรติบัตรแบบตัวเลขเรียงลำดับ รองรับหลายกิจกรรมและหลายแบบเกียรติบัตรในกิจกรรมเดียว เชื่อมใบที่ออกให้กับบัญชีภายใน และเปิดให้บุคคลทั่วไปตรวจสอบหรือดาวน์โหลดผ่าน QR Code และการกรอกข้อมูล

ระบบนี้เป็นโมดูล `certificates` ใหม่ ไม่ขยายตาราง `staff_achievements` เดิม เพราะโมดูล achievement เป็นบันทึกผลงานที่ผู้ใช้จัดการเอง ไม่มีวงจรออกเลข การเพิกถอน ผู้รับภายนอก แม่แบบ หรือการตรวจสอบสาธารณะ หน้าเกียรติบัตรของบุคลากรจะแสดงข้อมูลจากทั้งสองโมดูลโดยไม่คัดลอกข้อมูลข้ามตาราง

## ผลลัพธ์ที่ผู้ใช้จะได้รับ

- ผู้ดูแลสร้างชุดออกเกียรติบัตรตามปีการศึกษา และเพิ่มแบบเกียรติบัตรได้หลายแบบในกิจกรรมเดียว
- แต่ละแบบมี PDF พื้นหลัง ข้อความ รูปภาพ ฟอนต์ QR Code และตำแหน่งองค์ประกอบของตัวเอง
- เพิ่มผู้รับได้จากการค้นหาบัญชี การกรอกบุคคลภายนอก และการนำเข้า `.xlsx` หรือ UTF-8 `.csv`
- ระบบตรวจการเชื่อมบัญชี ชื่อไม่ตรง ข้อมูลขาด และการเลือกแบบก่อนออกจริง
- ออกเกียรติบัตรหลายรอบในกิจกรรมเดียว โดยเลขเรียงต่อกันและไม่ใช้เลขที่เคยออกซ้ำ
- นักเรียนและบุคลากรที่เชื่อมบัญชีเห็นใบของตนเองและดาวน์โหลดได้
- บุคคลทั่วไปตรวจสอบและดาวน์โหลดผ่าน QR Code หรือเลขเกียรติบัตรพร้อมชื่อและนามสกุล
- ใบที่ผิดถูกเพิกถอนและออกใหม่ ไม่แก้ไขข้อมูลเฉพาะบุคคลทับใบที่ออกแล้ว

## แนวทางที่พิจารณา

### ขยายโมดูล achievement เดิม

แนวทางนี้ลดจำนวนโมดูล แต่จะทำให้บันทึกผลงานส่วนบุคคลต้องรับผิดชอบกิจกรรม แม่แบบ การนำเข้า เลขเรียง การตรวจสอบสาธารณะ และการเพิกถอน ซึ่งมีวงจรข้อมูลและสิทธิ์ต่างจากของเดิมอย่างชัดเจน จึงไม่เลือกแนวทางนี้

### ใช้ Spreadsheet เป็นโครงสร้างทั้งหมด

แนวทางนี้ยืดหยุ่นสูง แต่ระบบไม่สามารถรับประกันชนิดผู้รับ การเชื่อมบัญชี หรือข้อมูลจำเป็นได้ และความผิดพลาดของชื่อคอลัมน์จะกลายเป็นเกียรติบัตรผิดจำนวนมาก จึงไม่เลือกแนวทางนี้

### โมดูลแยก พร้อมคอลัมน์หลักตายตัวและคอลัมน์เสริม — เลือกใช้

โมดูล certificate เป็นเจ้าของชุดออก แบบเกียรติบัตร รายการเตรียมออก ใบที่ออกแล้ว การเพิกถอน และการตรวจสอบ ระบบใช้คอลัมน์หลักแบบตายตัวสำหรับข้อมูลที่ต้องตรวจสอบ และยอมให้คอลัมน์อื่นเป็นตัวแปรข้อความแบบยืดหยุ่น วิธีนี้รักษาความถูกต้องของข้อมูลหลักโดยไม่จำกัดรูปแบบกิจกรรม

## คำศัพท์และขอบเขตข้อมูล

### ชุดออกเกียรติบัตร

หนึ่งชุดแทนกิจกรรมหลักหนึ่งกิจกรรม เช่น “กิจกรรมวันภาษาไทย” มีปีการศึกษา วันที่จัดกิจกรรม และตัวนับเลขของตัวเอง ชุดเดียวสามารถออกหลายรอบและมีแบบเกียรติบัตรหลายแบบ

### ประเภทผู้รับ

ประเภทผู้รับบอกวิธีเชื่อมบัญชี ไม่ได้บอกหน้าตาของเกียรติบัตร:

- `student` — นักเรียนในระบบ ต้องเชื่อมด้วยรหัสนักเรียน
- `staff` — บุคลากรในระบบ ต้องเชื่อมด้วยชื่อผู้ใช้
- `external` — บุคคลที่ไม่มีบัญชีในโรงเรียน รวมถึงนักเรียนจากโรงเรียนอื่น

### แบบเกียรติบัตร

แบบเกียรติบัตรบอกพื้นหลัง ข้อความ ตัวแปร และการจัดวาง เช่น รางวัลการแข่งขัน วิทยากร กรรมการ หรือเข้าร่วมกิจกรรม แต่ละแบบระบุประเภทผู้รับที่อนุญาตได้มากกว่าหนึ่งประเภท ตัวอย่างเช่น แบบรางวัลการแข่งขันใช้ได้กับทั้ง `student` และ `external` ส่วนแบบวิทยากรใช้ได้กับ `staff` และ `external`

ชื่อแบบต้องไม่ซ้ำกันภายในชุดเดียวหลังตัดช่องว่างและเปรียบเทียบแบบไม่แยกตัวพิมพ์สำหรับอักษรละติน

## วงจรการทำงาน

1. ผู้ดูแลสร้างชุดออก เลือกปีการศึกษา ระบุชื่อกิจกรรมและวันที่จัดกิจกรรม
2. ผู้ดูแลเพิ่มแบบเกียรติบัตร อัปโหลด PDF พื้นหลังหนึ่งหน้า และจัดองค์ประกอบใน editor
3. ผู้ดูแลเพิ่มผู้รับด้วยการค้นหาบัญชี กรอกเอง หรือนำเข้า Spreadsheet
4. Backend ตรวจข้อมูลและแบ่งรายการเป็น `ready`, `needs_review` หรือ `invalid`
5. ผู้ดูแลแก้ไขหรือยืนยันรายการ เลือกแบบทีละรายการหรือหลายรายการ และพรีวิวข้อมูลจริง
6. ผู้ดูแลเลือกเฉพาะรายการ `ready` แล้วกดออก ระบบจองเลขและสร้างใบทั้งหมดใน transaction เดียว
7. รายการที่ยังไม่พร้อมคงอยู่เพื่อแก้ไขและออกในรอบถัดไป
8. ใบที่ออกแล้วปรากฏในพื้นที่ส่วนตัวของบัญชีที่เชื่อม และตรวจสอบจากหน้าสาธารณะได้
9. หากข้อมูลเฉพาะใบผิด ผู้มีสิทธิ์เพิกถอนใบเดิม สร้างรายการทดแทน แก้ข้อมูล และออกด้วยเลขใหม่

## สถานะและกติกาการแก้ไข

### ชุดออก

- `draft` — ยังไม่เคยออกใบ แก้ไขหรือลบได้
- `active` — ออกอย่างน้อยหนึ่งใบแล้ว และยังออกเพิ่มได้
- `closed` — ห้ามออกเพิ่มจนกว่าจะเปิดใช้งานอีกครั้ง
- `archived` — ซ่อนจากงานประจำ แต่ข้อมูลส่วนตัวและการตรวจสอบสาธารณะยังทำงาน

หลังออกครั้งแรก `academic_year_id` และเลขลำดับกิจกรรมเปลี่ยนไม่ได้ ชื่อกิจกรรมและวันที่จัดกิจกรรมเป็นข้อมูลร่วมที่แก้ได้ด้วยสิทธิ์ update และหน้าต่างยืนยัน การแก้ดังกล่าวมีผลต่อหน้าตรวจสอบและ PDF ที่สร้างใหม่ทุกใบในชุด และต้องบันทึก audit log

ชุดที่ยังไม่เคยออกใบลบจริงได้ ชุดที่มีใบแล้วลบไม่ได้และใช้ `closed` หรือ `archived` แทน

### แบบเกียรติบัตร

แบบที่ยังไม่เคยใช้ลบได้ หากมีรายการร่างอ้างอิงอยู่ต้องย้ายรายการก่อน แบบที่เคยใช้ออกใบแล้วลบไม่ได้และปิดใช้งานแทน การปิดใช้งานมีผลเฉพาะการเลือกสำหรับรายการใหม่ ใบที่ออกแล้วต้องยังสร้าง PDF ได้

ระบบนี้ไม่มีประวัติเวอร์ชันของแม่แบบและไม่มีการย้อนกลับแม่แบบ เมื่อบันทึกการเปลี่ยน PDF พื้นหลัง ฟอนต์ รูปภาพ ข้อความ หรือตำแหน่ง ใบเก่าที่เปิดหรือดาวน์โหลดใหม่จะใช้แบบปัจจุบันทันที ไฟล์ PDF ที่เคยดาวน์โหลดหรือพิมพ์ออกไปแล้วไม่สามารถเปลี่ยนย้อนหลังได้ ระบบเก็บผู้แก้ เวลา และสรุปการเปลี่ยนใน audit log แต่ไม่เก็บ layout เก่าที่นำกลับมาใช้ได้

ก่อนบันทึกแบบที่เคยใช้แล้ว ระบบตรวจตัวแปรกับข้อมูลใบที่ออกทั้งหมดและแสดงจำนวนใบที่ไม่มีค่า ผู้ดูแลต้องยืนยันหากตัวแปรว่างจะทำให้พื้นที่นั้นว่างในใบเก่า องค์ประกอบที่อ้างถึงไฟล์ซึ่งไม่มีอยู่หรือยังสแกนไม่ผ่านจะบันทึกไม่ได้

### รายการเตรียมออกและใบที่ออกแล้ว

- รายการที่ยังไม่ออกเลขเพิ่ม แก้ไข หรือลบได้
- ใบที่ออกแล้วห้ามเปลี่ยนเลข ผู้รับที่เชื่อมบัญชี ชื่อ snapshot แบบที่ใช้ รายการแข่งขัน รางวัล/บทบาท ค่าคอลัมน์เสริม และวันที่ออก
- การแก้ข้อมูลเฉพาะบุคคลใช้การเพิกถอนและออกใหม่เท่านั้น
- เลขของใบที่เพิกถอนจะไม่ถูกนำกลับมาใช้
- ใบเดิมเก็บเหตุผล ผู้เพิกถอน เวลา และใบทดแทนถ้ามี
- หน้าตรวจสอบของใบเดิมแสดง “เพิกถอนแล้ว” แต่ไม่เปิดดาวน์โหลด

## รูปแบบเลขเกียรติบัตร

รูปแบบแสดงผลคือ:

```text
2569-0042-000123-4
```

- `2569` คือปีการศึกษาจาก `academic_years.year`
- `0042` คือลำดับชุดออกภายในปีการศึกษานั้น
- `000123` คือลำดับใบภายในชุดออก
- `4` คือ check digit แบบ Luhn จากตัวเลข 14 หลักก่อนหน้า

ตัวอย่างนี้คำนวณจาก `25690042000123` และได้ check digit `4` Check digit ช่วยตรวจการพิมพ์ผิดเท่านั้น ไม่ใช่กลไกยืนยันความแท้

เลขลำดับกิจกรรมยังไม่ถูกใช้ตอนสร้าง draft ระบบจองลำดับกิจกรรมถัดไปตอนออกใบครั้งแรก ลำดับใบเริ่มที่หนึ่งและใช้ร่วมกันทุกแบบในชุดเดียว การออกเพิ่มจึงเรียงต่อกันแม้ใช้คนละแบบ

Backend จองเลขด้วยแถว counter ของปีการศึกษาและ lock แถวชุดออกภายใน transaction ห้ามใช้ `MAX(...) + 1` หรือสร้างเลขใน browser การออกพร้อมกันต้องได้ช่วงเลขไม่ซ้ำกัน เลขที่ออกแล้วไม่ใช้ซ้ำและช่องว่างในลำดับเป็นสิ่งที่ยอมรับได้ ทั้งเลขเต็มและส่วนประกอบเก็บในฐานข้อมูล โดยเลขเต็มใช้ชนิดข้อความและมี unique constraint ต่อ tenant

ขนาดที่รองรับในรูปแบบนี้คือไม่เกิน 9,999 ชุดต่อปีการศึกษาและ 999,999 ใบต่อชุด เมื่อถึงขีดจำกัด ระบบต้องปฏิเสธก่อนเขียนข้อมูลและไม่ขยายรูปแบบเงียบ ๆ

## การนำเข้า Excel/CSV

Frontend โหลดไลบรารี Spreadsheet หลังผู้ใช้เลือกไฟล์ อ่าน `.xlsx` หรือ UTF-8 `.csv` ใน browser และส่งข้อมูลแถวแบบ typed ไปยัง Backend ไฟล์ต้นฉบับไม่ถูกเก็บใน File Platform Backend ตรวจทุกกติกาซ้ำและบันทึกเฉพาะข้อมูลที่จำเป็นต่อรายการเตรียมออก

ระบบมีไฟล์ตัวอย่างให้ดาวน์โหลด และรองรับคอลัมน์มาตรฐานต่อไปนี้:

| คอลัมน์ | กติกา |
| --- | --- |
| `ประเภทผู้รับ` | จำเป็น: `นักเรียน`, `บุคลากร` หรือ `บุคคลภายนอก` |
| `รหัสนักเรียน` | จำเป็นเมื่อเป็นนักเรียน ใช้เชื่อมบัญชีภายในเท่านั้น |
| `ชื่อผู้ใช้บุคลากร` | จำเป็นเมื่อเป็นบุคลากร ใช้เชื่อมบัญชีภายในเท่านั้น |
| `คำนำหน้า` | ไม่บังคับ |
| `ชื่อ` | จำเป็น |
| `นามสกุล` | จำเป็น |
| `รายการกิจกรรม` | ไม่บังคับ เช่น การแข่งขันคำคม |
| `รางวัลหรือบทบาท` | ไม่บังคับ เช่น รองชนะเลิศอันดับที่ 1 หรือวิทยากร |
| `แบบเกียรติบัตร` | ไม่บังคับหากระบบเลือกแบบที่ใช้ได้เพียงแบบเดียว มิฉะนั้นต้องระบุหรือเลือกบนเว็บก่อนออก |

ไม่เพิ่มคอลัมน์สถานศึกษาหรือหน่วยงานเป็นคอลัมน์มาตรฐาน ผู้ใช้ยังเพิ่มเป็นคอลัมน์เสริมได้เมื่อกิจกรรมนั้นต้องใช้

ชื่อคอลัมน์เสริมต้องไม่ว่าง ไม่ซ้ำ และไม่ชนชื่อคอลัมน์มาตรฐานหรือตัวแปรระบบ ทุกค่าถูกแปลงเป็นข้อความตามค่าที่แสดงใน Spreadsheet และเก็บใน map แบบ `string -> string` คอลัมน์เสริมกลายเป็นตัวแปร เช่นคอลัมน์ `ครูผู้ควบคุม` ใช้เป็น `{ครูผู้ควบคุม}`

ระบบปฏิเสธหัวคอลัมน์ที่สื่อถึงเลขประจำตัวประชาชน เช่น `เลขบัตรประชาชน`, `เลขประจำตัวประชาชน`, `national_id` และ `citizen_id` ก่อนบันทึกแถว และห้าม log request body หรือค่าจาก Spreadsheet รหัสนักเรียนไม่ใช่เลขประจำตัวประชาชนและห้ามใช้เลขประจำตัวประชาชนแทน

### การเชื่อมบัญชี

- นักเรียนเชื่อมด้วย `student_info.student_id` แบบตรงกันเท่านั้น
- บุคลากรเชื่อมด้วย `users.username` แบบตรงกันตามกติกาชื่อผู้ใช้ของระบบ
- ห้ามเชื่อมจากชื่อหรือนามสกุลอย่างเดียว
- ถ้าพบบัญชี ผู้รับถูกล็อกเป็นประเภทภายในและเปลี่ยนเป็นบุคคลภายนอกไม่ได้
- ถ้าไม่พบบัญชี รายการเป็น `needs_review` และผู้ดูแลยืนยันให้เป็นบุคคลภายนอกได้ทีละรายการหรือหลายรายการ
- Backend ค้นหาอีกครั้งตอนยืนยันและตอนออก เพื่อป้องกันการแปลงเป็นบุคคลภายนอกหลังมีบัญชีเกิดขึ้นแล้ว
- ถ้ารหัสพบบัญชีแต่ชื่อไม่ตรง ผู้ดูแลเลือกใช้ชื่อจากบัญชีหรือชื่อจากไฟล์ ค่าเลือกถูกเก็บเป็นชื่อ snapshot ตอนออกจริง
- เมื่อแปลงเป็นบุคคลภายนอก ระบบไม่เก็บรหัสที่ใช้ค้นหาบัญชีไว้ในใบที่ออก

รายการซ้ำทำให้เกิดคำเตือนแต่ไม่ถูกลบอัตโนมัติ เพราะบุคคลเดียวอาจได้รับหลายรางวัล รายการที่เหมือนกันทุกฟิลด์ในชุดและแบบเดียวกันต้องได้รับการยืนยันก่อนออกซ้ำ

### สถานะการตรวจข้อมูล

- `ready` — ข้อมูลจำเป็นครบ การเชื่อมบัญชีหรือสถานะบุคคลภายนอกชัดเจน และเลือกแบบที่รองรับแล้ว
- `needs_review` — ไม่พบบัญชี ชื่อไม่ตรง มีคำเตือนซ้ำ หรือยังต้องตัดสินใจเลือกแบบ
- `invalid` — ขาดข้อมูลจำเป็น ประเภทผู้รับผิด แบบไม่รองรับผู้รับ หัวคอลัมน์ผิด หรือตัวแปรอ้างอิงไม่ถูกต้อง

ผู้ดูแลแก้แต่ละแถวบนเว็บได้โดยไม่ต้องอัปโหลดใหม่ และออกเฉพาะรายการ `ready` ได้ หน้าสรุปต้องแสดงจำนวนของแต่ละสถานะก่อนกดออก

## ตัวแปรและ Editor

ตัวแปรจากคอลัมน์ใช้รูปแบบ `{ชื่อคอลัมน์}` และถูกแทนค่าเป็น plain text เท่านั้น ไม่ตีความเป็น HTML หรือคำสั่ง ระบบสงวนตัวแปรต่อไปนี้และไม่อนุญาตให้ Spreadsheet สร้างชื่อซ้ำ:

- `{ปีการศึกษา}`
- `{ชื่อกิจกรรมหลัก}`
- `{เลขเกียรติบัตร}`
- `{วันที่จัดกิจกรรม}`
- `{วันที่ออก}`
- `{ชื่อโรงเรียนผู้ออก}`
- `{QR_CODE}`

ค่าผู้รับ รายการแข่งขัน รางวัล/บทบาท คอลัมน์เสริม วันที่ออก และชื่อโรงเรียนผู้ออกถูก snapshot ตอนออก `{ชื่อกิจกรรมหลัก}` และ `{วันที่จัดกิจกรรม}` ใช้ค่าร่วมปัจจุบันของชุดตามกติกาการแก้ไขข้อมูลร่วมข้างต้น

Editor เป็นเครื่องมือเฉพาะเกียรติบัตร ไม่ใช่ Canva เต็มรูปแบบ โดยมีความสามารถดังนี้:

- ล็อก PDF พื้นหลังหนึ่งหน้าเป็นชั้นล่างสุด
- เพิ่มข้อความคงที่หรือข้อความที่ผสมตัวแปร
- เพิ่ม QR Code และรูปภาพ PNG, JPEG หรือ WebP
- ลาก ย้าย ปรับขนาด หมุน ทำซ้ำ จัดลำดับชั้น และจัดแนว
- กำหนดฟอนต์ น้ำหนัก ขนาด สี การจัดแนว ระยะบรรทัด ความกว้างสูงสุด auto-shrink และเงาข้อความ
- ใช้ Sarabun Regular/Bold เป็นฟอนต์มาตรฐานเริ่มต้น และอัปโหลด `.ttf`/`.otf` เพิ่มได้เมื่อผู้ดูแลยืนยันสิทธิ์การใช้งาน
- พรีวิวชื่อสั้น ชื่อปกติ ชื่อยาว และเลือกพรีวิวรายการจริงรายบุคคล

Layout เก็บเป็น JSONB ที่มี named schema และ `schemaVersion` สำหรับรูปแบบข้อมูลทางเทคนิค องค์ประกอบใช้ tagged union เช่น text, image และ QR ตำแหน่งและขนาดเก็บเป็น PDF points อ้างอิง CropBox ของหน้า ไม่เก็บพิกัดตามขนาดหน้าจอ การมี `schemaVersion` ไม่ใช่ประวัติเวอร์ชันของแม่แบบ และฐานข้อมูลเก็บ layout ปัจจุบันเพียงชุดเดียวต่อแบบ

## การสร้าง PDF

ระบบไม่เก็บ PDF สำเร็จรูปหนึ่งไฟล์ต่อผู้รับ Backend ส่ง render manifest ที่ผ่านการอนุญาต ซึ่งประกอบด้วยข้อมูล snapshot ข้อมูลร่วมปัจจุบัน layout และ grant อายุสั้นสำหรับอ่าน asset ส่วน Frontend โหลด renderer เฉพาะตอนพรีวิวหรือดาวน์โหลด

Renderer ใช้เส้นทางเดียวกันสำหรับ editor preview การดาวน์โหลดของผู้ดูแล พื้นที่ส่วนตัว และหน้าสาธารณะ:

1. ใช้ `pdfjs-dist` แสดง PDF พื้นหลังใน editor และรักษาขนาด/แนวหน้าจาก CropBox
2. โหลดฟอนต์ที่อนุญาตผ่าน FontFace
3. วาด text layer ที่ความละเอียดสำหรับงานพิมพ์ด้วย Canvas เพื่อให้ browser shaping ภาษาไทย เงา และ auto-shrink ตรงกับ preview
4. ใช้ `pdf-lib` วาง text layer โปร่งใส รูปภาพ และ QR ลงบนหน้า PDF เดิม
5. ส่งออก PDF หนึ่งหน้า หรือรวมรายการที่ผู้ดูแลเลือกเป็น PDF หลายหน้า

พื้นหลัง PDF เดิมจึงยังคงเป็น PDF ขณะที่ text layer ถูกฝังเป็นภาพความละเอียดสูง ข้อความในใบไม่รับประกันว่าจะค้นหาหรือเลือกคัดลอกได้ Renderer, PDF parser, Spreadsheet parser และไลบรารีรูปภาพต้อง lazy-load หลังการกระทำของผู้ใช้และใช้ server stub น้ำหนักเบาระหว่าง SSR ตามข้อกำหนด frontend ของโครงการ

หาก renderer หรือ asset โหลดไม่สำเร็จ ระบบไม่สร้างไฟล์แบบขาดองค์ประกอบ แต่แสดงข้อผิดพลาดและให้ลองใหม่ การสร้าง PDF ไม่เปลี่ยนสถานะใบและทำซ้ำได้

## โครงสร้างฐานข้อมูล

การ implement ใช้ migration ลำดับถัดไป ณ เวลานั้นและห้ามแก้ migration ที่เคยใช้แล้ว ตารางหลักที่เสนอคือ:

### `certificate_academic_year_counters`

เก็บ `academic_year_id` และลำดับกิจกรรมถัดไป ใช้ row lock ตอนชุดออกใบแรก มี unique key ต่อปีการศึกษา

### `certificate_campaigns`

เก็บ UUID, ปีการศึกษา, ชื่อกิจกรรม, วันที่จัดกิจกรรม, activity sequence ที่ยังเป็น null ได้ก่อนออกครั้งแรก, ลำดับใบถัดไป, สถานะ และผู้สร้าง/ผู้แก้

### `certificate_templates`

เก็บ UUID, campaign ID, ชื่อแบบ, PDF background file ID, allowed recipient types, typed layout JSONB, active flag และผู้สร้าง/ผู้แก้ ตารางนี้เก็บสถานะปัจจุบันเท่านั้นและไม่มีตาราง template versions

### `certificate_template_assets`

ผูก template กับ File Platform file ID และชนิด `font` หรือ `image` Layout อ้าง asset ID จากตารางนี้แทนการฝัง data URL เพื่อให้ resource policy และ referential checks ทำงานได้

### `certificate_import_batches`

เก็บแหล่งที่มาแบบ `xlsx`, `csv`, `manual` หรือ `account_search`, จำนวนแถว สถานะ ผู้สร้าง และเวลา ไม่เก็บไฟล์ต้นฉบับหรือ raw request body

### `certificate_candidates`

เก็บรายการที่ยังไม่ออก ได้แก่ batch/campaign/template, recipient type, optional user ID, ชื่อที่นำเข้า ชื่อจากบัญชี ชื่อที่ผู้ดูแลเลือก รายการแข่งขัน รางวัล/บทบาท custom string map สถานะการจับคู่ และข้อมูลเตือน รายการที่ออกแล้วชี้ไปยัง certificate ID เพื่อป้องกันการออกซ้ำจากการ retry

### `certificate_issue_runs`

เก็บ idempotency key ต่อคำสั่งออก ผู้สั่ง เวลา และผลรวม ใช้ unique constraint ป้องกัน browser retry สร้างเลขชุดใหม่

### `certificates`

เก็บ UUID ภายใน, campaign/template/candidate ID, เลขเต็มและส่วนประกอบ, recipient type, optional user ID, snapshot คำนำหน้า/ชื่อ/นามสกุล, รายการแข่งขัน, รางวัล/บทบาท, custom string map, ชื่อโรงเรียนผู้ออก, วันที่ออก, สถานะ `issued` หรือ `revoked`, ข้อมูลเพิกถอน, optional replacement certificate ID และหลักฐานสำหรับ QR

QR proof เป็นค่าสุ่ม entropy สูงต่อใบ ไม่ใช่เลขเกียรติบัตรอีกชุด ระบบเก็บค่าเข้ารหัสสำหรับนำไปสร้าง QR ใหม่และเก็บ hash สำหรับตรวจสอบ โดยใช้ utility encryption ที่มีอยู่ ห้าม log ค่า proof ทั้งแบบเต็มและบางส่วน ใบที่เพิกถอนยังรักษา proof ไว้เพื่อแสดงสถานะเดิม

ทุก foreign key, unique constraint และ index ต้องรองรับ tenant database ปัจจุบัน โดยเฉพาะเลขเต็ม, activity sequence ต่อปี, candidate issuance และ issue idempotency ห้าม cascade-delete ไปยังใบที่ออกแล้ว

## File Platform

เพิ่ม purpose และ domain policy สำหรับ certificate template แยกตามชนิด PDF background, image asset และ font asset แทนการใช้ `FilePurpose::Certificate` ที่ปัจจุบันอยู่ใน identity policy แต่ยังไม่มี explicit certificate-domain authorization

- PDF ต้องเป็นไฟล์หนึ่งหน้า ไม่เข้ารหัส ขนาดไม่เกิน policy และผ่าน malware scan
- รูปภาพรับ PNG, JPEG และ WebP ตาม MIME sniffing และ dimension limits
- ฟอนต์รับ TTF/OTF ที่ parser เปิดได้ ขนาดตาม policy และผ่าน malware scan
- ไฟล์ทุกชนิดเป็น private
- การสร้าง แทนที่ และลบ asset ใช้ certificate update permission และตรวจว่าเป็น resource ของ template/campaign เดียวกัน
- render manifest ของผู้ดูแล เจ้าของใบ หรือผู้ผ่าน public verification ออก delivery grant อายุสั้นเฉพาะ asset ที่จำเป็น
- ไฟล์ที่ยังถูกอ้างโดย template ปัจจุบันลบไม่ได้ ไฟล์ที่ถูกแทนและไม่ถูกอ้างอีกจึงลบได้ เพราะระบบไม่เก็บแม่แบบเวอร์ชันเก่า

## การออกเลขแบบ Transactional

คำสั่งออกมี client-generated idempotency key และ candidate IDs ที่เลือก Service ทำตามลำดับนี้ใน transaction เดียว:

1. lock issue run key และคืนผลเดิมหากคำสั่งนี้สำเร็จแล้ว
2. lock campaign และ candidates ที่เลือก
3. ตรวจสิทธิ์ สถานะ `ready`, account match และ template compatibility ซ้ำ
4. หากเป็นการออกครั้งแรก lock academic-year counter และจอง activity sequence
5. จองช่วง recipient sequence จาก campaign counter
6. คำนวณเลขเต็ม/check digit สร้าง QR proof และ insert certificates
7. ทำเครื่องหมาย candidates ว่าออกแล้ว บันทึก issue run และ audit events
8. commit ทั้งชุด

ถ้ามี candidate ใดไม่ผ่านการตรวจซ้ำ คำสั่งทั้งชุดล้มเหลวก่อนจองเลขและตอบ typed conflict พร้อม row IDs ที่ต้องแก้ ไม่มีการออกบางส่วนโดยไม่แจ้ง ผู้ดูแลเลือกชุดใหม่และ retry ด้วย idempotency key ใหม่หลังแก้ข้อมูล

## การตรวจสอบสาธารณะและดาวน์โหลด

มี frontend route สาธารณะสำหรับการตรวจสอบสองทาง:

### QR Code

QR ใช้ canonical tenant URL ที่ Backend สร้างจาก tenant/subdomain ที่เชื่อถือได้ ไม่ใช้ Host จาก request โดยตรง รูปแบบแนวคิดคือ:

```text
https://<school-host>/verify/certificate/2569-0042-000123-4#proof=<opaque-proof>
```

ใช้ URL fragment เพื่อไม่ส่ง proof เข้า reverse-proxy access log Frontend อ่าน fragment แล้ว POST เลขกับ proof ไปยัง Backend โดยไม่ใส่ proof ใน query string จากนั้นลบ proof ออกจาก address bar หลังได้ผลลัพธ์

### กรอกด้วยตนเอง

ผู้ใช้กรอกเลขเกียรติบัตร ชื่อ และนามสกุลแยกช่อง Backend trim และยุบช่องว่าง ทำ Unicode normalization และเปรียบเทียบแบบ exact หลัง normalization โดยไม่ทำ fuzzy match ความต่างตัวพิมพ์อักษรละตินไม่ถือว่าต่าง

เลขไม่พบ ชื่อผิด นามสกุลผิด proof ผิด หรือข้อมูลต่าง tenant ใช้ข้อความเดียวว่า “ไม่พบข้อมูลที่ตรงกัน” และ response shape/status ที่ไม่เปิดเผยว่าฟิลด์ใดถูก ห้าม log ชื่อหรือ proof จากคำขอตรวจสอบ

เมื่อผ่านการตรวจสอบ Backend ตอบข้อมูลสาธารณะที่อนุญาตและ render grant อายุสั้น ข้อมูลที่แสดงมีเฉพาะ:

- สถานะใช้ได้หรือเพิกถอน
- เลขเกียรติบัตร
- คำนำหน้า ชื่อ และนามสกุล
- ชื่อกิจกรรมและปีการศึกษา
- ชื่อแบบเกียรติบัตร
- รายการแข่งขันและรางวัล/บทบาทเมื่อมี
- วันที่ออกและชื่อโรงเรียนผู้ออก

ห้ามส่ง user ID, รหัสนักเรียน, username, import data, account-match state, ข้อมูลติดต่อ หรือ custom columns ทั้งก้อน Public render manifest ส่งเฉพาะค่าตัวแปรที่แม่แบบปัจจุบันอ้างถึง หน้าสาธารณะแสดง custom value ได้เฉพาะภายใน PDF ที่ผู้ดูแลตั้งใจวางบนแม่แบบแล้วเท่านั้น

ใบสถานะ `issued` แสดงปุ่มดาวน์โหลด หลังผ่าน QR หรือ manual verification ใบ `revoked` แสดงข้อความกลางว่า “เกียรติบัตรถูกเพิกถอน” และเลขใบทดแทนเมื่อมี แต่ไม่เปิดเผยเหตุผลภายใน ไม่ให้ render grant และไม่แสดงปุ่มดาวน์โหลด

Endpoint ตรวจสอบและขอ render ใช้ rate limit ต่อ tenant/IP และต่อเลขเป้าหมาย การตรวจแบบ manual เป็น POST เท่านั้น และ response ป้องกัน cache สำหรับข้อมูลเฉพาะใบ

## พื้นที่ส่วนตัวและหน้าจัดการ

### ผู้ดูแล

เพิ่ม workspace สำหรับรายการชุดออก สถานะ จำนวนแบบ จำนวนรายการพร้อมออก จำนวนใบที่ออกและเพิกถอน แต่ละชุดมี route ย่อยสำหรับข้อมูลกิจกรรม แบบเกียรติบัตร/Editor รายชื่อและการนำเข้า รอบการออก และใบที่ออกแล้ว เส้นทางย่อยรองรับ deep link และ lazy-load แยกกัน

### บุคลากร

หน้าเกียรติบัตรเดิมแสดง route-backed tabs:

- “โรงเรียนออกให้” อ่านจากโมดูล certificate และแก้หรือลบเองไม่ได้
- “บันทึกด้วยตนเอง” ใช้โมดูล achievement เดิมโดยไม่เปลี่ยน data ownership

แต่ละแท็บโหลดข้อมูลเมื่อผู้ใช้มี permission ของโมดูลนั้นเท่านั้น เพื่อไม่ให้หน้า read-only ล้มจาก request ที่ไม่มีสิทธิ์

### นักเรียน

เพิ่มหน้า “เกียรติบัตรของฉัน” แสดงใบที่ `certificates.user_id` ตรงกับ session user เท่านั้น ผู้ใช้ดูรายละเอียด ดาวน์โหลด และเปิดหน้าตรวจสอบได้ แต่แก้ไขหรือลบไม่ได้ ใบเพิกถอนยังแสดงพร้อมสถานะและไม่มีปุ่มดาวน์โหลด

### บุคคลภายนอก

ไม่มีพื้นที่บัญชี ผู้ดูแลดาวน์โหลดให้ หรือผู้รับใช้ QR/manual verification เพื่อดาวน์โหลดเอง

## สิทธิ์และ Resource Policy

เพิ่ม permission ผ่าน `contracts/permissions.json` และสร้าง generated registries ตามขั้นตอนโครงการ:

- `certificate.read.own` — ดูและดาวน์โหลดใบที่ `user_id` เป็นตนเอง
- `certificate.read.school` — ดูชุด แบบ รายการ และใบทั้งโรงเรียน
- `certificate.create.school` — สร้างชุด แบบ และรายการ
- `certificate.update.school` — แก้ชุด แบบ asset และรายการร่าง รวมถึง resolve import warnings
- `certificate.delete.school` — ลบเฉพาะ draft/unused resource ตาม lifecycle
- `certificate.issue.school` — ออกเลขและสร้างใบ
- `certificate.revoke.school` — เพิกถอนและสร้าง replacement draft
- `certificate.download.school` — ดาวน์โหลดใบเดี่ยวหรือรวมหลายใบในฐานะผู้ดูแล

Backend policy เป็นแหล่งตัดสินสิทธิ์ Frontend route guard และปุ่มเป็นเพียง UX การอ่านแบบ own ใช้ session user ID เท่านั้นและไม่รับ target user ID จาก client การกระทำกับ campaign/template/candidate/certificate ต้องตรวจ relationship ใน reusable `certificate_access_policy` และทุก public handler ต้อง resolve tenant ด้วย public tenant context ของระบบ

Migration ลงทะเบียน permission และ grants ที่จำเป็นสำหรับ built-in roles โดยไม่ใช้ raw permission strings ใน runtime Route metadata, menu registration และ capability controls ใช้ generated constants เท่านั้น

## API และ Contract

เพิ่ม typed DTOs, `utoipa::path` และ schema registration สำหรับกลุ่ม endpoint ต่อไปนี้:

- campaign list/detail/create/update/delete/close/archive
- template list/create/update/deactivate และ template asset operations
- candidate list/create/update/bulk update/delete และ import submission
- import validation summary และ account-match resolution
- issue command, issue-run result, revoke และ replacement draft
- issued certificate list/detail และ authorized render manifest
- current-user certificate list/detail/render
- public QR verification, manual verification และ public render manifest

ทุก JSON endpoint ใช้ `ApiResponse`/`ApiErrorResponse` envelope Binary file delivery ยังคงผ่าน File Platform grant ที่มีอยู่ Wire DTOs ถูก generate จาก OpenAPI และ frontend API wrapper ใช้ generated types ไม่มี `unknown`, response cast หรือ ad-hoc JSON Layout และ custom columns มี named contract แม้ภายในใช้ JSONB

Mutation ที่สำเร็จคืน resource/outcome ที่เปลี่ยนเพื่อให้ Frontend patch state เฉพาะรายการ ไม่มี realtime event ใหม่สำหรับ certificate ในรุ่นแรก เพราะ workflow นี้ไม่ต้องการ live collaboration Permission changes ยังคงใช้กลไก `permission_changed` เดิม

## Audit, Security และ PDPA

บันทึก audit สำหรับการสร้าง/แก้/ปิด/เก็บชุด การแก้หรือปิดแบบ การยืนยันชื่อ การแปลงรายการเป็นบุคคลภายนอก การออกแต่ละรอบ การเพิกถอน และการสร้างใบทดแทน Audit เก็บ IDs, counts, status transitions และฟิลด์ที่เปลี่ยนเท่าที่จำเป็น ไม่เก็บ raw Spreadsheet, QR proof, national ID หรือข้อมูลที่ไม่จำเป็น

ข้อกำหนดเพิ่มเติม:

- ห้ามใช้หรือรับ plaintext national ID สำหรับการจับคู่ผู้รับ
- ห้าม log request body ของ import/verification, ชื่อผู้ตรวจสอบ, delivery grant หรือ QR proof
- ใช้ private File Platform และ explicit domain policy สำหรับทุก asset
- public response ใช้ allowlist ไม่ serialize issued row หรือ custom JSON ตรง ๆ
- proof และ name comparison ใช้วิธีเปรียบเทียบที่ไม่เผย early-match timing อย่างมีนัยสำคัญ
- ตรวจ tenant relationship ทุกครั้งและตอบ not-found แบบเดียวกันเมื่อ resource อยู่คนละโรงเรียน
- render grant มีอายุสั้น ผูกกับ tenant, certificate, action และสถานะใบ
- input strings มีความยาวสูงสุด, Unicode normalization และการป้องกัน formula/HTML injection เมื่อแสดงหรือ export

## Error Handling

- PDF ไม่ถูกต้อง หลายหน้า เข้ารหัส หรือยังสแกนไม่ผ่าน: ปฏิเสธก่อนเลือกเป็นพื้นหลัง
- font/image ผิดชนิดหรือ parser เปิดไม่ได้: ปฏิเสธและไม่เพิ่ม asset reference
- Spreadsheet ไม่มีคอลัมน์จำเป็น หัวซ้ำ ใช้ตัวแปรสงวน หรือมีหัวข้อมูลต้องห้าม: ปฏิเสธทั้ง import พร้อมระบุหัว/แถวที่แก้ โดยไม่สะท้อนค่าลับ
- account เปลี่ยนระหว่าง review กับ issue: ยกเลิก issue transaction และคืน candidate เป็น `needs_review`
- template ถูกปิดหรือไม่รองรับประเภทผู้รับ: candidate ออกไม่ได้
- browser retry: issue idempotency คืนผลเดิมและไม่สร้างเลขใหม่
- concurrent issue: row locks ทำให้ช่วงเลขไม่ซ้ำ
- render ล้มเหลว: ใบยังคง `issued`; แสดงข้อผิดพลาดและ retry ได้
- public verification ผิดหรือ rate-limited: ตอบ generic และไม่ออก render grant
- ใบเพิกถอน: แสดงสถานะได้ แต่ render/download ถูกปฏิเสธทั้ง public, own และ admin

## การทดสอบ

### Pure/unit tests

- parse/format หมายเลขและ Luhn รวมถึงตัวอย่าง `2569-0042-000123-4`
- ขีดจำกัด activity/recipient sequence
- header normalization, reserved/forbidden headers และ standard/custom variable mapping
- recipient compatibility และ account-match decision table
- name normalization และ generic verification outcomes
- typed layout validation, variable interpolation, auto-shrink inputs และ missing-value report

### Backend service/database tests

- ออกครั้งแรกจอง activity sequence ตอน issue ไม่ใช่ตอนสร้าง draft
- สองคำสั่งพร้อมกันได้เลขไม่ซ้ำและเรียงตาม counter
- retry idempotency ไม่สร้างใบหรือใช้เลขเพิ่ม
- การตรวจ selected candidates เป็น atomic และไม่มี partial issue
- matched account เปลี่ยนเป็น external ไม่ได้ทั้ง single และ bulk
- snapshot, revoke, replacement link และเลขที่ไม่ถูกใช้ซ้ำ
- own/school policy ทั้ง allowed, denied และ wrong-tenant
- File Platform purpose/resource relationship และ referenced-asset deletion guard
- public QR/manual verification, invalid proof, revoked behavior, rate limit และ response allowlist
- audit ไม่มี proof, raw import หรือ national ID

### Frontend/static/browser tests

- ดาวน์โหลดไฟล์ตัวอย่างและนำเข้า `.xlsx`/`.csv`
- สถานะ ready/review/invalid, เลือกชื่อจากบัญชี/ไฟล์ และ bulk external confirmation
- template routing ตามประเภทผู้รับ รวม external ที่ใช้แบบรางวัลการแข่งขัน
- editor drag/resize/style/QR/image และ preview ชื่อสั้น/ยาว
- PDF fixture ภาษาไทย ฟอนต์อัปโหลด เงา และตำแหน่งตรงกันระหว่าง preview/download ที่ viewport คงที่
- ออกบางรายการ ออกเพิ่ม และแสดงผลในพื้นที่นักเรียน/บุคลากร
- public QR/manual flow, download success และ revoked download absence
- route/permission controls สำหรับผู้ดู อ่าน แก้ ออก เพิกถอน และดาวน์โหลด

การ implementation ต้องรัน focused tests และ matrix ที่เกี่ยวข้องใน `.rules`: permission generation/check/tests, API contract generation/check/tests, backend formatting/static architecture/check, frontend lint/Svelte check/static tests, focused Playwright, `git diff --check`, final diff review และ `git status --short`

## ผลกระทบต่อระบบ

- **Backend:** โมดูล certificate, services, policies, public verification และ render-manifest authorization ใหม่
- **Frontend:** management workspace, focused editor, import review, own pages, public verification และ lazy browser renderer
- **Database:** ตารางและ constraints ใหม่ผ่าน migration ลำดับถัดไป ไม่มีการแก้ migration เดิม
- **Permissions:** permission contract และ generated registries ใหม่
- **API:** Rust/OpenAPI/generated TypeScript contracts ใหม่
- **Files:** certificate-domain purposes/policies ใหม่บน File Platform เดิม
- **Realtime:** ไม่มี event certificate ใหม่
- **Deployment:** ไม่มี service หรือ secret ใหม่ ใช้ encryption utility และ storage delivery ที่มีอยู่; browser-only libraries ต้องไม่เข้า Cloudflare Worker bundle
- **Documentation:** ปรับ canonical testing/operations เฉพาะเมื่อ implementation เพิ่มขั้นตอนตรวจหรือข้อกำหนด runtime ที่ถาวร

## สิ่งที่ไม่อยู่ในขอบเขต

- Canva เต็มรูปแบบ รูปร่างอิสระ วิดีโอ animation หรือ collaborative editing
- การเก็บ PDF สำเร็จรูปถาวรหนึ่งไฟล์ต่อใบ
- template version history, rollback หรือการตรึงหน้าตาเดิมของใบที่เคยออก
- การสร้างบัญชีให้บุคคลภายนอกหรือเชื่อมข้อมูลระหว่างโรงเรียน
- การจับคู่ด้วยชื่ออย่างเดียวหรือเลขประจำตัวประชาชน
- ลายเซ็นดิจิทัลตามกฎหมาย PKI
- การส่งอีเมล/SMS อัตโนมัติและระบบรับรองจากหน่วยงานภายนอก
- การรวม certificate records ลงใน `staff_achievements`

## เกณฑ์ยอมรับ

ฟีเจอร์ถือว่าตรงตามแบบเมื่อผู้ดูแลสามารถสร้างกิจกรรมที่มีหลายแบบ นำเข้าหรือเพิ่มผู้รับทั้งสามกลุ่ม แก้รายการเตือน ออกหลายรอบด้วยเลขไม่ซ้ำ เชื่อมใบกับบัญชีภายใน เพิกถอนและออกใหม่ และให้บุคคลทั่วไปตรวจสอบพร้อมดาวน์โหลดใบที่ยังใช้ได้ โดยข้อมูลข้าม tenant ข้อมูลบัญชี และข้อมูลอ่อนไหวไม่รั่วไหล แม่แบบที่แก้แล้วต้องมีผลกับ PDF ที่สร้างใหม่ของใบเดิมตามที่ผู้ใช้เลือกไว้
