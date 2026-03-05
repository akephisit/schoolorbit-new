# ระบบรับสมัครนักเรียน (Admission System)

**Last Updated:** 2026-03-05  
**Status:** Design Phase

---

## 📋 ภาพรวม

ระบบรับสมัครนักเรียนใหม่ ครอบคลุม 6 ขั้นตอนหลัก ตั้งแต่การสมัคร → ตรวจสอบ → สอบ → จัดห้อง → ยืนยัน → มอบตัว และสร้าง account เข้าระบบหลัก

---

## 🔄 Flow การทำงาน

```
┌─────────────────────────────────────────────────────────────────────┐
│  🚀 Pre-setup: ครูตั้งค่ารอบรับสมัคร                               │
│  - สร้าง Admission Round ผูกกับ academic_year                        │
│  - กำหนดสายการเรียน → ดึง study_plan + class_rooms (capacity)      │
│  - เพิ่ม/ลด วิชาที่สอบได้เอง (ยืดหยุ่น)                           │
│  - กำหนดวิชาที่ใช้เรียงคะแนนต่อสาย (ใน UI)                        │
└─────────────────────────────────────────────────────────────────────┘
          ↓ (เปิดรับสมัคร: status = "open")
┌─────────────────────────────────────────────────────────────────────┐
│  📝 ขั้นตอนที่ 1: สมัคร (Applicant Portal)                         │
│  - กรอกเลขบัตรประชาชน → ระบบตรวจว่าสมัครรอบนี้แล้วหรือยัง        │
│  - กรอกข้อมูลส่วนตัว, ผู้ปกครอง, ที่อยู่                           │
│  - เลือกสายการเรียน (แสดงรับกี่คน / เหลือกี่คน)                   │
│  - ยืนยันการสมัคร → login portal ด้วยเลขบัตรประชาชนได้เลย         │
└─────────────────────────────────────────────────────────────────────┘
          ↓ (status: submitted)
┌─────────────────────────────────────────────────────────────────────┐
│  ✅ ขั้นตอนที่ 2: ครูตรวจสอบข้อมูล                                │
│  - ดูรายชื่อผู้สมัคร กรอง/ค้นหา                                    │
│  - ตรวจเอกสาร/ข้อมูล → กด "ยืนยัน" หรือ "ปฏิเสธ"                │
│  - status: submitted → verified (หรือ rejected)                     │
└─────────────────────────────────────────────────────────────────────┘
          ↓ (status: exam_eligible)
┌─────────────────────────────────────────────────────────────────────┐
│  📊 ขั้นตอนที่ 3: สอบ / ขั้นตอนที่ 4: กรอกคะแนน + จัดห้อง       │
│  - ครูกรอกคะแนนแต่ละวิชา per นักเรียน (bulk edit ได้)             │
│  - ระบบคำนวณ "คะแนนรวมสาย" (เฉพาะวิชาที่ครูเลือก per track)      │
│  - Preview เรียงอันดับ → แสดงว่าแต่ละคนได้ห้องไหน                 │
│  - ครูกด "บันทึกการจัดห้อง" → บันทึกลง room_assignments           │
│  - status: scored → accepted                                        │
└─────────────────────────────────────────────────────────────────────┘
          ↓ (ประกาศผล: status round = "announced")
┌─────────────────────────────────────────────────────────────────────┐
│  🔍 ขั้นตอนที่ 5: นักเรียนตรวจสอบผล (Portal)                      │
│  - Login ด้วยเลขบัตรประชาชนอย่างเดียว (ไม่ต้องจำรหัสเพิ่ม)        │
│  - ดูผลการสอบ, คะแนน, ห้องที่ได้                                   │
│  - กด "ยืนยันเข้าเรียน" + กรอกแบบฟอร์มมอบตัวล่วงหน้า             │
│  - (enrollment form: ข้อมูลสุขภาพ, ขนาดชุด ฯลฯ)                  │
└─────────────────────────────────────────────────────────────────────┘
          ↓ (status: enrolled)
┌─────────────────────────────────────────────────────────────────────┐
│  🎓 ขั้นตอนที่ 6: มอบตัว (Staff)                                   │
│  - ค้นหาผู้สมัครที่มา (scan QR / ค้นชื่อ / เลขที่ใบสมัคร)        │
│  - ตรวจฟอร์มครบ → กด "ยืนยันมอบตัว"                               │
│  - ระบบ: สร้าง User account + enroll เข้า class_room อัตโนมัติ    │
│  - status: enrolled → (สร้าง user_id จริงในระบบหลัก)               │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 🗄️ Database Schema

### Migration: `046_create_admission_system.sql`

```sql
-- === ตาราง 1: Admission Rounds (รอบรับสมัคร) ===
CREATE TABLE admission_rounds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_year_id UUID NOT NULL REFERENCES academic_years(id),
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id), -- ชั้นที่รับ เช่น ม.1, ม.4

    name VARCHAR(200) NOT NULL,         -- "รับสมัครนักเรียน ม.1 ปีการศึกษา 2569"
    description TEXT,

    -- ช่วงรับสมัคร
    apply_start_date DATE NOT NULL,
    apply_end_date DATE NOT NULL,

    -- ช่วงสอบและประกาศผล
    exam_date DATE,
    result_announce_date DATE,

    -- ช่วงมอบตัว
    enrollment_start_date DATE,
    enrollment_end_date DATE,

    status VARCHAR(30) NOT NULL DEFAULT 'draft',
    -- draft | open | exam | scoring | announced | enrolling | closed

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- === ตาราง 2: Exam Subjects (วิชาที่สอบ — ยืดหยุ่น เพิ่ม/ลดได้เอง) ===
CREATE TABLE admission_exam_subjects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_round_id UUID NOT NULL REFERENCES admission_rounds(id) ON DELETE CASCADE,

    name VARCHAR(200) NOT NULL,         -- "วิชาคณิตศาสตร์", "วิชาวิทยาศาสตร์"
    code VARCHAR(50),                   -- "MATH", "SCI"
    max_score NUMERIC(8,2) NOT NULL DEFAULT 100,
    display_order INT DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- === ตาราง 3: Admission Tracks (สายการเรียนที่รับสมัคร) ===
-- ผูกกับ study_plan เพื่อรู้ว่าสายนี้คือแผนการเรียนไหน
-- ดึง capacity จาก class_rooms ที่ผูกกับ study_plan_version ใน academic_year นั้น
CREATE TABLE admission_tracks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_round_id UUID NOT NULL REFERENCES admission_rounds(id) ON DELETE CASCADE,
    study_plan_id UUID NOT NULL REFERENCES study_plans(id),

    name VARCHAR(200) NOT NULL,         -- "สายวิทย์-คณิต", "สายศิลป์-ภาษา"

    -- capacity: คำนวณจากห้องเรียนที่ครูสร้างไว้ใน academic_year นั้น
    -- ไม่ต้องเก็บ hardcode — query จาก class_rooms ได้เลย
    -- แต่เก็บ override ไว้เผื่อต้องการกำหนดเอง
    capacity_override INT, -- NULL = คำนวณจาก class_rooms อัตโนมัติ

    -- วิชาที่ใช้เรียงคะแนน (array of exam_subject id)
    -- ให้ user จัดการใน UI — ไม่ normalize ลงตารางแยก
    scoring_subject_ids JSONB DEFAULT '[]'::jsonb,
    -- เช่น: ["uuid-math", "uuid-sci"] → รวมคะแนนวิชาพวกนี้ก่อนเรียง

    -- Tie-breaking: ถ้าคะแนนเท่ากัน
    tiebreak_method VARCHAR(30) DEFAULT 'applied_at',
    -- 'applied_at' = สมัครก่อนได้ก่อน | 'gpa' = GPA เดิมสูงกว่าได้ก่อน

    display_order INT DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- === ตาราง 4: Applications (ใบสมัคร) ===
CREATE TABLE admission_applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_round_id UUID NOT NULL REFERENCES admission_rounds(id),
    admission_track_id UUID NOT NULL REFERENCES admission_tracks(id),

    -- เลขที่ใบสมัคร (running per round)
    application_number VARCHAR(50) UNIQUE, -- "2569-0001"

    -- ข้อมูลผู้สมัคร
    national_id VARCHAR(13) NOT NULL,
    title VARCHAR(20),
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    gender VARCHAR(10),
    date_of_birth DATE,
    phone VARCHAR(20),
    email VARCHAR(200),

    -- ที่อยู่
    address_line TEXT,
    sub_district VARCHAR(100),
    district VARCHAR(100),
    province VARCHAR(100),
    postal_code VARCHAR(10),

    -- ข้อมูลโรงเรียนเดิม
    previous_school VARCHAR(200),
    previous_grade VARCHAR(50),
    previous_gpa NUMERIC(4,2),

    -- ข้อมูลบิดา
    father_name VARCHAR(200),
    father_phone VARCHAR(20),
    father_occupation VARCHAR(100),
    father_national_id VARCHAR(13),

    -- ข้อมูลมารดา
    mother_name VARCHAR(200),
    mother_phone VARCHAR(20),
    mother_occupation VARCHAR(100),
    mother_national_id VARCHAR(13),

    -- ข้อมูลผู้ปกครอง (กรณีไม่ใช่บิดา/มารดา)
    guardian_name VARCHAR(200),
    guardian_phone VARCHAR(20),
    guardian_relation VARCHAR(100),
    guardian_national_id VARCHAR(13),

    -- Status workflow
    status VARCHAR(30) NOT NULL DEFAULT 'submitted',
    -- submitted | verified | rejected | exam_eligible | scored | accepted | enrolled | withdrawn

    -- การดำเนินการของครู
    verified_by UUID REFERENCES users(id),
    verified_at TIMESTAMPTZ,
    rejection_reason TEXT,

    -- มอบตัว
    enrolled_by UUID REFERENCES users(id),
    enrolled_at TIMESTAMPTZ,

    -- หลังมอบตัวสำเร็จ: user account ที่สร้างขึ้น
    created_user_id UUID REFERENCES users(id),

    -- ข้อมูลเสริม
    metadata JSONB DEFAULT '{}'::jsonb,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 1 คน สมัครได้ 1 ครั้งต่อรอบ
    CONSTRAINT unique_national_id_per_round UNIQUE(national_id, admission_round_id)
);

CREATE INDEX idx_applications_national_id ON admission_applications(national_id);
CREATE INDEX idx_applications_round ON admission_applications(admission_round_id);
CREATE INDEX idx_applications_track ON admission_applications(admission_track_id);
CREATE INDEX idx_applications_status ON admission_applications(status);
CREATE INDEX idx_applications_number ON admission_applications(application_number);

-- === ตาราง 5: Exam Scores (คะแนนสอบ per วิชา) ===
CREATE TABLE admission_exam_scores (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    exam_subject_id UUID NOT NULL REFERENCES admission_exam_subjects(id) ON DELETE CASCADE,

    score NUMERIC(8,2),
    entered_by UUID REFERENCES users(id),
    entered_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT unique_app_subject UNIQUE(application_id, exam_subject_id)
);

-- === ตาราง 6: Room Assignments (ผลการจัดห้อง) ===
CREATE TABLE admission_room_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id),
    class_room_id UUID NOT NULL REFERENCES class_rooms(id),

    rank_in_track INT,      -- อันดับในสาย (1, 2, 3, ...)
    rank_in_room INT,       -- อันดับในห้อง (1, 2, 3, ...)
    total_score NUMERIC(10,2),  -- คะแนนรวม (จากวิชาที่ใช้เรียง)
    full_score NUMERIC(10,2),   -- คะแนนรวมทุกวิชา

    assigned_at TIMESTAMPTZ DEFAULT NOW(),
    assigned_by UUID REFERENCES users(id),

    -- นักเรียนยืนยัน
    student_confirmed BOOLEAN DEFAULT false,
    student_confirmed_at TIMESTAMPTZ,

    CONSTRAINT unique_application_assignment UNIQUE(application_id)
);

CREATE INDEX idx_room_assignments_room ON admission_room_assignments(class_room_id);

-- === ตาราง 7: Enrollment Forms (แบบมอบตัว) ===
CREATE TABLE admission_enrollment_forms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id),

    -- ข้อมูลที่กรอกเพิ่มตอนมอบตัว (JSONB เพื่อความยืดหยุ่น)
    -- เช่น: { "shirt_size": "L", "blood_type": "A+", "allergy": "...", ... }
    form_data JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- สถานะ
    pre_submitted_at TIMESTAMPTZ,   -- นักเรียนกรอกล่วงหน้าออนไลน์
    completed_at TIMESTAMPTZ,       -- ครูยืนยันที่โรงเรียน (วันมอบตัว)
    completed_by UUID REFERENCES users(id),

    CONSTRAINT unique_enrollment_form UNIQUE(application_id)
);

-- Triggers for updated_at
CREATE TRIGGER update_admission_rounds_updated_at
    BEFORE UPDATE ON admission_rounds
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_admission_applications_updated_at
    BEFORE UPDATE ON admission_applications
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
```

---

## 🔐 Permission System

ใช้ระบบ permission เดิม (`has_permission(pool, "xxx.xxx")`)

```
admission.view      — ดูข้อมูลทั้งหมด (รายชื่อ, ผล ฯลฯ)
admission.manage    — สร้าง/แก้ไข รอบรับสมัคร, สาย, วิชา, ตั้งค่า
admission.verify    — ตรวจสอบ/ยืนยันใบสมัคร
admission.scores    — กรอกคะแนน + preview + บันทึกการจัดห้อง
admission.enroll    — รับมอบตัว + สร้าง user account เข้าระบบหลัก
```

**ตัวอย่างการใช้งานใน handler:**
```rust
// ตรวจสอบ permission ก่อนดำเนินการ
if !user.has_permission(&pool, "admission.verify").await? {
    return Err(AppError::Forbidden("ไม่มีสิทธิ์ตรวจสอบใบสมัคร".to_string()));
}
```

---

## 📁 โครงสร้าง Files

### Backend (`backend-school/src/modules/admission/`)
```
admission/
  ├── mod.rs                    ← Router + routes ทั้งหมด
  ├── models/
  │   ├── mod.rs
  │   ├── rounds.rs             ← AdmissionRound, AdmissionTrack, ExamSubject
  │   └── applications.rs       ← Application, ExamScore, RoomAssignment, EnrollmentForm
  └── handlers/
      ├── mod.rs                ← pub mod declarations
      ├── rounds.rs             ← CRUD rounds, tracks, exam subjects
      ├── applications.rs       ← สมัคร (public), list, verify, reject
      ├── scores.rs             ← กรอกคะแนน (bulk), คำนวณรวม
      ├── selections.rs         ← เรียงอันดับ, preview จัดห้อง, บันทึก
      └── portal.rs             ← Applicant portal: login, ดูผล, ยืนยัน, มอบตัว
```

### Routes ใน `mod.rs`
```rust
pub fn admission_routes() -> Router<AppState> {
    Router::new()
        // === Admin: จัดการรอบ ===
        .route("/rounds", get(handlers::rounds::list_rounds)
                          .post(handlers::rounds::create_round))
        .route("/rounds/{id}", get(handlers::rounds::get_round)
                               .put(handlers::rounds::update_round)
                               .delete(handlers::rounds::delete_round))
        .route("/rounds/{id}/status", put(handlers::rounds::update_round_status))

        // === Tracks (สายการเรียน) ===
        .route("/rounds/{id}/tracks", get(handlers::rounds::list_tracks)
                                      .post(handlers::rounds::create_track))
        .route("/tracks/{id}", put(handlers::rounds::update_track)
                               .delete(handlers::rounds::delete_track))
        // Capacity: ดึงจาก class_rooms ที่ผูกกับ study_plan ใน academic_year
        .route("/tracks/{id}/capacity", get(handlers::rounds::get_track_capacity))

        // === Exam Subjects (วิชาสอบ) ===
        .route("/rounds/{id}/subjects", get(handlers::rounds::list_subjects)
                                        .post(handlers::rounds::create_subject))
        .route("/subjects/{id}", put(handlers::rounds::update_subject)
                                 .delete(handlers::rounds::delete_subject))

        // === Applications (ใบสมัคร) ===
        // Public: ผู้สมัครยื่น (ไม่ต้อง auth)
        .route("/apply/{round_id}", post(handlers::applications::submit_application))
        // Staff: ดูและ manage
        .route("/rounds/{id}/applications", get(handlers::applications::list_applications))
        .route("/applications/{id}", get(handlers::applications::get_application))
        .route("/applications/{id}/verify", put(handlers::applications::verify_application))
        .route("/applications/{id}/reject", put(handlers::applications::reject_application))

        // === Scores ===
        .route("/rounds/{id}/scores", get(handlers::scores::get_all_scores))
        .route("/applications/{id}/scores", put(handlers::scores::update_scores))
        // Bulk update: ส่งทีละหลายคน
        .route("/rounds/{id}/scores/bulk", put(handlers::scores::bulk_update_scores))

        // === Selections (เรียงคะแนน + จัดห้อง) ===
        .route("/rounds/{id}/ranking", get(handlers::selections::get_ranking))
        .route("/tracks/{id}/ranking", get(handlers::selections::get_track_ranking))
        .route("/rounds/{id}/assign-rooms", post(handlers::selections::assign_rooms))

        // === Enrollment (มอบตัว) ===
        .route("/rounds/{id}/enrollment", get(handlers::applications::list_enrollment_pending))
        .route("/applications/{id}/enroll", post(handlers::applications::complete_enrollment))

        // === Portal (Applicant) — Stateless: ส่ง national_id + application_number ทุก request ===
        // ไม่มี login/JWT — ไม่เกี่ยวกับระบบ account หลักเลย
        .route("/portal/check", post(handlers::portal::check_application))   // ตรวจสอบใบสมัคร
        .route("/portal/status", post(handlers::portal::get_status))         // ดูสถานะ + ผล
        .route("/portal/confirm", post(handlers::portal::confirm_enrollment)) // ยืนยันเข้าเรียน
        .route("/portal/form", post(handlers::portal::get_enrollment_form)
                               .put(handlers::portal::submit_enrollment_form)) // แบบมอบตัว
}
```

### Frontend (`frontend-school/src/routes/`)
```
(app)/staff/admission/
  ├── +page.svelte                     ← Dashboard: รายการรอบรับสมัคร
  └── [roundId]/
      ├── +page.svelte                 ← Detail: ตั้งค่ารอบ, tracks, subjects
      ├── applications/
      │   └── +page.svelte             ← รายชื่อผู้สมัคร, filter, verify
      ├── scores/
      │   └── +page.svelte             ← กรอกคะแนน (table bulk edit)
      ├── results/
      │   └── +page.svelte             ← Preview เรียงอันดับ, จัดห้อง, บันทึก
      └── enrollment/
          └── +page.svelte             ← รับมอบตัว (ค้นหา, ยืนยัน)

(public)/admission/
  ├── +page.svelte                     ← หน้าสมัคร: กรอกข้อมูล
  └── portal/
      └── +page.svelte                 ← กรอก national_id + เลขที่ใบสมัคร → SvelteKit เก็บใน
                                          sessionStorage แล้วส่งทุก API call (ไม่มี account)
```

---

## 🎯 Logic สำคัญ

### การคำนวณ Capacity ต่อ Track

```sql
-- ดึง capacity จาก class_rooms ที่ผูกกับ study_plan ใน academic_year นั้น
SELECT 
    t.id AS track_id,
    t.name,
    COUNT(DISTINCT cr.id) AS room_count,
    -- capacity_override ถ้ามี ใช้นั้น ถ้าไม่มีให้ user กำหนดเอง (หรือนับจากห้อง)
    COALESCE(t.capacity_override, COUNT(DISTINCT cr.id) * 40) AS total_capacity
FROM admission_tracks t
JOIN study_plans sp ON t.study_plan_id = sp.id
JOIN study_plan_versions spv ON spv.study_plan_id = sp.id
JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
                   AND cr.academic_year_id = (
                       SELECT academic_year_id FROM admission_rounds WHERE id = t.admission_round_id
                   )
WHERE t.admission_round_id = $1
GROUP BY t.id, t.name, t.capacity_override;
```

### การเรียงคะแนนและจัดห้อง

```
1. เลือก track (เช่น "สายวิทย์-คณิต")
2. ดึง scoring_subject_ids จาก track → [uuid-math, uuid-sci]
3. คำนวณ total_score = SUM(score WHERE exam_subject_id IN scoring_subject_ids) per application
4. เรียงลำดับ:
   ORDER BY total_score DESC, 
            (tiebreak: applied_at ASC หรือ previous_gpa DESC)
5. จัดห้อง:
   - ห้องที่ 1 ← อันดับ 1-N (N = capacity ของห้อง นั้น)
   - ห้องที่ 2 ← อันดับต่อไป
   - ...
6. บันทึก admission_room_assignments
7. อัปเดต application status → "accepted"
```

### Portal Auth — Stateless Credentials (ไม่มี JWT ไม่มี Account)

```
แนวคิด:
  ไม่มี "login" จริง — ทุก request ของ portal ส่ง credentials ไปด้วยเลย
  Backend ตรวจ credentials ทุกครั้ง → ไม่ต้อง manage session/token
  แยกขาดจากระบบ users/JWT ของ staff โดยสิ้นเชิง

Credentials ที่ใช้:
  - national_id  (เลขบัตรประชาชน 13 หลัก)
  - application_number  (เลขที่ใบสมัคร เช่น "2569-0001" — ได้รับตอนสมัคร)

ทำไมต้องมีทั้งสองค่า:
  - national_id อย่างเดียว → ถ้า leak เลขบัตร คนอื่นดูข้อมูลได้
  - national_id + application_number → ต้องรู้ทั้งสองค่า ปลอดภัยกว่า
  - application_number ไม่ sensitive เท่า password ปกตินักเรียนจำได้

Flow Frontend (SvelteKit):
  1. หน้า portal: กรอก national_id + application_number
  2. กด "ตรวจสอบ" → call POST /portal/check
  3. ถ้าถูกต้อง → เก็บ { national_id, application_number } ใน sessionStorage
  4. ทุก request ถัดไปส่ง credentials ใน request body หรือ header
  5. ปิด browser → sessionStorage ล้าง → ต้องกรอกใหม่

Flow Backend (Rust) — ทุก portal endpoint:
  1. รับ national_id + application_number จาก request
  2. SELECT * FROM admission_applications
       WHERE national_id = $1 AND application_number = $2
  3. ถ้าไม่เจอ → 401 "ข้อมูลไม่ถูกต้อง"
  4. ถ้าเจอ → ดำเนินการต่อด้วย application record นั้น

หมายเหตุ:
  - ไม่มี session ไม่มี token ไม่มี cookie
  - ไม่เกี่ยวกับตาราง users เลยก่อนขั้นตอนมอบตัว
  - Rate limiting: จำกัด attempt ต่อ IP เพื่อป้องกัน brute force
```

---

## 📊 สถานะ (Status) ของ Application

| Status | ความหมาย | ดำเนินการโดย |
|--------|----------|-------------|
| `submitted` | ยื่นใบสมัครแล้ว | ผู้สมัคร |
| `verified` | ครูตรวจสอบแล้ว ผ่าน | Staff |
| `rejected` | ครูตรวจสอบแล้ว ไม่ผ่าน | Staff |
| `exam_eligible` | มีสิทธิ์สอบ (ระบบ set หลัง verify) | Auto |
| `scored` | กรอกคะแนนแล้ว | Staff |
| `accepted` | ได้รับการคัดเลือก มีห้องแล้ว | Auto (หลัง assign rooms) |
| `enrolled` | ยืนยันและมอบตัวแล้ว สร้าง account แล้ว | Staff |
| `withdrawn` | ถอนตัว | ผู้สมัคร / Staff |

---

## 💡 ข้อควรระวัง / Design Decisions

| ประเด็น | การตัดสินใจ |
|---------|------------|
| **Capacity** | คำนวณจาก `class_rooms` ที่ผูก `study_plan_version` ใน `academic_year` นั้น มี `capacity_override` สำหรับบางกรณี |
| **Portal Auth** | Stateless: ส่ง `national_id + application_number` ทุก request — ไม่มี JWT ไม่มี session ไม่เกี่ยวระบบหลัก |
| **วิชาเรียงคะแนน** | เก็บแค่ `scoring_subject_ids: JSONB` ใน track — UI drag เลือกจากวิชาทั้งหมดของรอบ |
| **Idempotency** | UNIQUE INDEX `(national_id, admission_round_id)` — สมัครซ้ำไม่ได้ในรอบเดียวกัน |
| **Tie-breaking** | กำหนดได้ต่อ track: `applied_at` (FIFO) หรือ `gpa` |
| **Enrollment Form** | ใช้ JSONB `form_data` — Flexible ไม่ต้อง migrate เมื่อเพิ่ม field |
| **ห้องเรียน** | ครูสร้างเองใน `academic/classrooms` ก่อน — ระบบ admission ดึงมาใช้ ไม่ duplicate |
| **สร้าง Account** | ตอน `complete_enrollment` → INSERT users + student_info ตามโครงสร้างเดิม |

---

## 📈 ลำดับการพัฒนา

- [ ] **Phase 1**: Migration SQL (`046_create_admission_system.sql`)
- [ ] **Phase 2**: Backend Models (`models/rounds.rs`, `models/applications.rs`)
- [ ] **Phase 3**: Backend Handlers — Rounds/Tracks/Subjects CRUD
- [ ] **Phase 4**: Backend Handlers — Applications (submit, list, verify)
- [ ] **Phase 5**: Backend Handlers — Scores (กรอก, bulk update)
- [ ] **Phase 6**: Backend Handlers — Selections (เรียงคะแนน, จัดห้อง)
- [ ] **Phase 7**: Backend Handlers — Portal (login, status, confirm, form)
- [ ] **Phase 8**: Backend Handlers — Enrollment (มอบตัว, สร้าง account)
- [ ] **Phase 9**: Frontend Staff — จัดการรอบ + tracks + subjects
- [ ] **Phase 10**: Frontend Staff — รายชื่อผู้สมัคร + ตรวจสอบ
- [ ] **Phase 11**: Frontend Staff — กรอกคะแนน (bulk table)
- [ ] **Phase 12**: Frontend Staff — เรียงอันดับ + จัดห้อง + บันทึก
- [ ] **Phase 13**: Frontend Staff — รับมอบตัว
- [ ] **Phase 14**: Frontend Public Portal — สมัคร + ดูผล + ยืนยัน + กรอกมอบตัว

---

*เอกสารนี้เป็น design document สำหรับทีมพัฒนา — อัปเดตเมื่อมีการเปลี่ยนแปลงใหญ่*
