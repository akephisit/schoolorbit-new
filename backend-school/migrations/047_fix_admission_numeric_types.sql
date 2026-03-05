-- เปลี่ยนชนิดข้อมูลจาก NUMERIC เป็น DOUBLE PRECISION (FLOAT8) 
-- เพื่อให้เข้ากันได้ง่ายกับ f64 ใน Rust SQLx (ที่ใช้ RETURNING *)
-- ลดปัญหา "mismatched types; Rust type Option<f64> is not compatible with SQL type NUMERIC"

ALTER TABLE admission_applications ALTER COLUMN previous_gpa TYPE DOUBLE PRECISION;
ALTER TABLE admission_exam_scores ALTER COLUMN score TYPE DOUBLE PRECISION;
ALTER TABLE admission_room_assignments ALTER COLUMN total_score TYPE DOUBLE PRECISION;
ALTER TABLE admission_room_assignments ALTER COLUMN full_score TYPE DOUBLE PRECISION;
