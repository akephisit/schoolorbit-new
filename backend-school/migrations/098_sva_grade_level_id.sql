-- ============================================
-- study_plan_version_activities: add grade_level_id (pattern C — 1 row per grade)
-- ============================================
-- เดิม sva มีแค่ (plan, catalog, term) → ไม่มี grade → grid กระจายทุกระดับที่
-- catalog รองรับ (เกินขอบเขตที่ admin ต้องการ)
-- ใหม่: 1 sva row ต่อ (plan, grade, term, catalog) — match pattern sps (subjects)
-- ============================================

ALTER TABLE study_plan_version_activities
    ADD COLUMN IF NOT EXISTS grade_level_id UUID REFERENCES grade_levels(id) ON DELETE RESTRICT;

-- Backfill: expand each existing sva → N rows per grade ใน catalog.grade_level_ids
-- ถ้า catalog ไม่มี grade_level_ids (null/empty) → ลบ sva (orphan, admin re-add ได้)
INSERT INTO study_plan_version_activities (study_plan_version_id, activity_catalog_id, grade_level_id, term, display_order)
SELECT sva.study_plan_version_id,
       sva.activity_catalog_id,
       (grade_id_str)::UUID,
       sva.term,
       sva.display_order
FROM study_plan_version_activities sva
JOIN activity_catalog ac ON ac.id = sva.activity_catalog_id
CROSS JOIN LATERAL jsonb_array_elements_text(ac.grade_level_ids) AS grade_id_str
WHERE sva.grade_level_id IS NULL
  AND ac.grade_level_ids IS NOT NULL
  AND jsonb_array_length(ac.grade_level_ids) > 0
  -- กันซ้ำ (เผื่อ migration รันซ้ำ)
  AND NOT EXISTS (
      SELECT 1 FROM study_plan_version_activities sva2
      WHERE sva2.study_plan_version_id = sva.study_plan_version_id
        AND sva2.activity_catalog_id = sva.activity_catalog_id
        AND sva2.grade_level_id = (grade_id_str)::UUID
        AND sva2.term IS NOT DISTINCT FROM sva.term
  );

-- ลบ row เก่า (grade_level_id IS NULL) — ถูก expand ไปแล้ว หรือ orphan (catalog ไม่มี grade)
DELETE FROM study_plan_version_activities WHERE grade_level_id IS NULL;

-- บังคับ NOT NULL
ALTER TABLE study_plan_version_activities
    ALTER COLUMN grade_level_id SET NOT NULL;

-- Unique: 1 row ต่อ (plan, grade, term, catalog)
ALTER TABLE study_plan_version_activities
    DROP CONSTRAINT IF EXISTS unique_sva_plan_grade_term_catalog;

ALTER TABLE study_plan_version_activities
    ADD CONSTRAINT unique_sva_plan_grade_term_catalog
    UNIQUE (study_plan_version_id, grade_level_id, term, activity_catalog_id);

CREATE INDEX IF NOT EXISTS idx_sva_grade ON study_plan_version_activities(grade_level_id);

COMMENT ON COLUMN study_plan_version_activities.grade_level_id IS
    'ระดับชั้นที่ plan กำหนดให้กิจกรรมนี้ — 1 row ต่อ (plan, grade, term, catalog) (pattern เดียวกับ sps)';
