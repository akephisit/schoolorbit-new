-- Migration 063: School Settings (logo, branding)
-- เก็บ logo_url ของโรงเรียนเป็น setting กลาง
-- ชื่อโรงเรียนดึงจาก backend-admin เสมอ ไม่เก็บซ้ำที่นี่

CREATE TABLE school_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    logo_url TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed 1 row เพื่อให้ UPDATE ได้เลยโดยไม่ต้อง upsert logic ซับซ้อน
INSERT INTO school_settings DEFAULT VALUES;
