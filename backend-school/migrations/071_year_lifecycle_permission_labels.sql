-- Keep existing school-scoped capabilities; clarify their year/term scope.
-- No new role grants or teacher permission inheritance.
UPDATE permissions SET name=definition.name,description=definition.description
FROM (VALUES
 ('academic_lifecycle.read.school','ดูความพร้อมปีการศึกษาและภาคเรียน','ดูสถานะและความพร้อมปิดปีการศึกษาและภาคเรียนทั้งโรงเรียน'),
 ('academic_lifecycle.manage.school','เตรียมและจัดการสถานะปีและภาคเรียน','เตรียมภาคเรียนและเริ่มหรือยกเลิกขั้นตอนกำลังปิดปีหรือภาคเรียน โดยไม่ใช่สิทธิ์ยืนยันปิด'),
 ('academic_lifecycle.close.school','ปิดปีการศึกษาและภาคเรียน','ยืนยันปิดปีการศึกษาหรือภาคเรียนหลังผลสรุปและความพร้อมผ่านการตรวจสอบ')
) AS definition(code,name,description)
WHERE permissions.code=definition.code;
