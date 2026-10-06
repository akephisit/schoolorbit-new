-- Repair only the explicitly recognized, complete legacy secondary-school shape.
-- Stable program and requirement identities remain the owners of live references.
LOCK TABLE curricula, curriculum_versions, study_programs, curriculum_term_slots,
    curriculum_course_requirements, curriculum_activity_requirements,
    homerooms, student_academic_years, learning_offering_targets IN SHARE ROW EXCLUSIVE MODE;

DO $$
DECLARE
    source_count integer;
BEGIN
    SELECT count(*) INTO source_count FROM curricula
    WHERE code IN ('J-SCI-MATH','J-ART-LANG','J-CAR-TECH','S-SCI-MATH','S-ART-LANG');
    IF source_count NOT IN (0, 5) THEN
        RAISE EXCEPTION 'CURRICULUM_092_PARTIAL_LEGACY_HIERARCHY';
    END IF;
END $$;

CREATE TEMP TABLE curriculum_hierarchy_mapping ON COMMIT DROP AS
SELECT c.id AS old_curriculum_id, v.id AS old_version_id, p.id AS program_id,
       slot.id AS old_slot_id, c.code AS old_code,
       CASE WHEN left(c.code,1)='J' THEN 'J-SCI-MATH' ELSE 'S-SCI-MATH' END AS anchor_code,
       substring(c.code FROM 3) AS program_code,
       CASE substring(c.code FROM 3)
           WHEN 'SCI-MATH' THEN 'วิทยาศาสตร์-คณิตศาสตร์'
           WHEN 'ART-LANG' THEN 'ศิลป์-ภาษา'
           WHEN 'CAR-TECH' THEN 'การงานอาชีพ-เทคโนโลยี'
       END AS program_name,
       to_jsonb(c) AS original_curriculum, to_jsonb(v) AS original_version,
       to_jsonb(p) AS original_program, to_jsonb(slot) AS original_slot
FROM curricula c
JOIN curriculum_versions v ON v.curriculum_id=c.id
JOIN study_programs p ON p.curriculum_version_id=v.id
JOIN curriculum_term_slots slot ON slot.curriculum_version_id=v.id
WHERE c.code IN ('J-SCI-MATH','J-ART-LANG','J-CAR-TECH','S-SCI-MATH','S-ART-LANG');

DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM curricula WHERE code IN ('J-SCI-MATH','J-ART-LANG','J-CAR-TECH','S-SCI-MATH','S-ART-LANG'))
    AND (
        (SELECT count(*) FROM curriculum_hierarchy_mapping) <> 5
        OR EXISTS (
            SELECT 1 FROM curriculum_hierarchy_mapping m
            JOIN curriculum_versions v ON v.id=m.old_version_id
            JOIN curricula c ON c.id=m.old_curriculum_id
            JOIN study_programs p ON p.id=m.program_id
            JOIN curriculum_term_slots s ON s.id=m.old_slot_id
            WHERE NOT v.migration_provenance @> '{"migration":41}'::jsonb
               OR v.status<>'published' OR v.revision_year IS NULL
               OR p.code<>'DEFAULT' OR p.name_th<>'แผนมาตรฐาน' OR NOT p.is_default
               OR p.status<>'published' OR NOT c.is_active
               OR c.name_th<>m.program_name||CASE WHEN left(m.old_code,1)='J' THEN ' ม.ต้น' ELSE ' ม.ปลาย' END
               OR jsonb_array_length(c.grade_level_ids)<>3
               OR EXISTS (
                   SELECT 1 FROM jsonb_array_elements_text(c.grade_level_ids) selected(id)
                   LEFT JOIN grade_levels grade ON grade.id=selected.id::uuid
                   WHERE grade.id IS NULL OR grade.level_type<>'secondary'
                      OR grade.year NOT BETWEEN CASE WHEN left(m.old_code,1)='J' THEN 1 ELSE 4 END
                                             AND CASE WHEN left(m.old_code,1)='J' THEN 3 ELSE 6 END
               )
               OR EXISTS (SELECT 1 FROM curriculum_course_requirements r WHERE r.study_program_id=p.id AND (r.curriculum_version_id<>v.id OR r.term_slot_id<>s.id))
               OR EXISTS (SELECT 1 FROM curriculum_activity_requirements r WHERE r.study_program_id=p.id AND (r.curriculum_version_id<>v.id OR r.term_slot_id<>s.id))
               OR s.term_type<>'regular' OR s.type_occurrence<>1 OR s.sequence<>1
               OR (SELECT count(*) FROM curriculum_versions other WHERE other.curriculum_id=c.id)<>1
               OR (SELECT count(*) FROM study_programs other WHERE other.curriculum_version_id=v.id)<>1
               OR (SELECT count(*) FROM curriculum_term_slots other WHERE other.curriculum_version_id=v.id)<>1
               OR EXISTS (SELECT 1 FROM grade_level_progressions WHERE curriculum_id=c.id)
        )
        OR EXISTS (
            SELECT 1 FROM curriculum_hierarchy_mapping m
            JOIN curriculum_hierarchy_mapping anchor ON anchor.old_code=m.anchor_code
            WHERE (m.original_version->'revision_year') IS DISTINCT FROM (anchor.original_version->'revision_year')
               OR (m.original_curriculum->'owning_organization_unit_id') IS DISTINCT FROM (anchor.original_curriculum->'owning_organization_unit_id')
               OR (m.original_slot-'id'-'curriculum_version_id'-'created_at'-'updated_at'-'row_version')
                   IS DISTINCT FROM (anchor.original_slot-'id'-'curriculum_version_id'-'created_at'-'updated_at'-'row_version')
               OR NOT (m.original_curriculum->'grade_level_ids') @> (anchor.original_curriculum->'grade_level_ids')
               OR NOT (anchor.original_curriculum->'grade_level_ids') @> (m.original_curriculum->'grade_level_ids')
        )
    ) THEN
        RAISE EXCEPTION 'CURRICULUM_092_AMBIGUOUS_LEGACY_HIERARCHY';
    END IF;
END $$;

CREATE TEMP TABLE curriculum_hierarchy_evidence ON COMMIT DROP AS
SELECT
 (SELECT jsonb_agg(to_jsonb(r)-'curriculum_version_id'-'term_slot_id'-'updated_at'-'row_version' ORDER BY r.id)
  FROM curriculum_course_requirements r) AS courses,
 (SELECT jsonb_agg(to_jsonb(r)-'curriculum_version_id'-'term_slot_id'-'updated_at'-'row_version' ORDER BY r.id)
  FROM curriculum_activity_requirements r) AS activities,
 (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM homerooms) AS rooms,
 (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM student_academic_years) AS students,
 (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM learning_offering_targets) AS targets,
 (SELECT jsonb_agg(id ORDER BY id) FROM study_programs) AS programs;

ALTER TABLE curriculum_versions DISABLE TRIGGER curriculum_versions_published_immutable;
ALTER TABLE study_programs DISABLE TRIGGER study_programs_published_curriculum_immutable;
ALTER TABLE curriculum_term_slots DISABLE TRIGGER curriculum_term_slots_published_immutable;
ALTER TABLE curriculum_course_requirements DISABLE TRIGGER curriculum_course_requirements_published_immutable;
ALTER TABLE curriculum_activity_requirements DISABLE TRIGGER curriculum_activity_requirements_published_immutable;

UPDATE study_programs p
SET is_default=false
FROM curriculum_hierarchy_mapping m
WHERE p.id=m.program_id AND m.old_code<>m.anchor_code;

UPDATE study_programs p
SET curriculum_version_id=anchor.old_version_id,
    code=m.program_code, name_th=m.program_name, name_en=NULL,
    row_version=p.row_version+1, updated_at=now()
FROM curriculum_hierarchy_mapping m
JOIN curriculum_hierarchy_mapping anchor ON anchor.old_code=m.anchor_code
WHERE p.id=m.program_id;

UPDATE curriculum_course_requirements r
SET curriculum_version_id=anchor.old_version_id, term_slot_id=anchor.old_slot_id,
    row_version=r.row_version+1
FROM curriculum_hierarchy_mapping m
JOIN curriculum_hierarchy_mapping anchor ON anchor.old_code=m.anchor_code
WHERE r.study_program_id=m.program_id AND m.old_version_id<>anchor.old_version_id;

UPDATE curriculum_activity_requirements r
SET curriculum_version_id=anchor.old_version_id, term_slot_id=anchor.old_slot_id,
    row_version=r.row_version+1
FROM curriculum_hierarchy_mapping m
JOIN curriculum_hierarchy_mapping anchor ON anchor.old_code=m.anchor_code
WHERE r.study_program_id=m.program_id AND m.old_version_id<>anchor.old_version_id;

UPDATE curriculum_versions v
SET version_name='ฉบับปรับปรุง พุทธศักราช '||v.revision_year::text,
    migration_provenance=v.migration_provenance||jsonb_build_object(
        'curriculumHierarchyRepair', jsonb_build_object('migration',92,'sources',(
            SELECT jsonb_agg(to_jsonb(m) ORDER BY m.old_code)
            FROM curriculum_hierarchy_mapping m WHERE m.anchor_code=anchor.old_code
        ))), row_version=v.row_version+1, updated_at=now()
FROM curriculum_hierarchy_mapping anchor
WHERE v.id=anchor.old_version_id AND anchor.old_code=anchor.anchor_code;

UPDATE curricula c
SET code=CASE WHEN left(m.old_code,1)='J' THEN 'J-SECONDARY' ELSE 'S-SECONDARY' END,
    identity_key=CASE WHEN left(m.old_code,1)='J' THEN 'j-secondary' ELSE 's-secondary' END,
    name_th=CASE WHEN left(m.old_code,1)='J' THEN 'ระดับมัธยมศึกษาตอนต้น' ELSE 'ระดับมัธยมศึกษาตอนปลาย' END,
    name_en=NULL, row_version=c.row_version+1, updated_at=now()
FROM curriculum_hierarchy_mapping m
WHERE c.id=m.old_curriculum_id AND m.old_code=m.anchor_code;

DELETE FROM curriculum_term_slots s USING curriculum_hierarchy_mapping m
WHERE s.id=m.old_slot_id AND m.old_code<>m.anchor_code;
DELETE FROM curriculum_versions v USING curriculum_hierarchy_mapping m
WHERE v.id=m.old_version_id AND m.old_code<>m.anchor_code;
DELETE FROM curricula c USING curriculum_hierarchy_mapping m
WHERE c.id=m.old_curriculum_id AND m.old_code<>m.anchor_code;

DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM curriculum_hierarchy_evidence e WHERE
        e.courses IS DISTINCT FROM (SELECT jsonb_agg(to_jsonb(r)-'curriculum_version_id'-'term_slot_id'-'updated_at'-'row_version' ORDER BY r.id) FROM curriculum_course_requirements r)
        OR e.activities IS DISTINCT FROM (SELECT jsonb_agg(to_jsonb(r)-'curriculum_version_id'-'term_slot_id'-'updated_at'-'row_version' ORDER BY r.id) FROM curriculum_activity_requirements r)
        OR e.rooms IS DISTINCT FROM (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM homerooms)
        OR e.students IS DISTINCT FROM (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM student_academic_years)
        OR e.targets IS DISTINCT FROM (SELECT jsonb_agg(jsonb_build_array(id,study_program_id) ORDER BY id) FROM learning_offering_targets)
        OR e.programs IS DISTINCT FROM (SELECT jsonb_agg(id ORDER BY id) FROM study_programs)
    ) OR EXISTS (
        SELECT 1 FROM curriculum_course_requirements r JOIN study_programs p ON p.id=r.study_program_id
        WHERE r.curriculum_version_id<>p.curriculum_version_id
        UNION ALL SELECT 1 FROM curriculum_activity_requirements r JOIN study_programs p ON p.id=r.study_program_id
        WHERE r.curriculum_version_id<>p.curriculum_version_id
    ) THEN RAISE EXCEPTION 'CURRICULUM_092_PRESERVATION_FAILED'; END IF;
END $$;

ALTER TABLE curriculum_versions ENABLE TRIGGER curriculum_versions_published_immutable;
ALTER TABLE study_programs ENABLE TRIGGER study_programs_published_curriculum_immutable;
ALTER TABLE curriculum_term_slots ENABLE TRIGGER curriculum_term_slots_published_immutable;
ALTER TABLE curriculum_course_requirements ENABLE TRIGGER curriculum_course_requirements_published_immutable;
ALTER TABLE curriculum_activity_requirements ENABLE TRIGGER curriculum_activity_requirements_published_immutable;

DO $$ BEGIN
    IF (SELECT count(*) FROM pg_trigger WHERE tgenabled='O' AND (tgrelid,tgname) IN (
        ('curriculum_versions'::regclass,'curriculum_versions_published_immutable'),
        ('study_programs'::regclass,'study_programs_published_curriculum_immutable'),
        ('curriculum_term_slots'::regclass,'curriculum_term_slots_published_immutable'),
        ('curriculum_course_requirements'::regclass,'curriculum_course_requirements_published_immutable'),
        ('curriculum_activity_requirements'::regclass,'curriculum_activity_requirements_published_immutable')
    ))<>5 THEN RAISE EXCEPTION 'CURRICULUM_092_IMMUTABILITY_NOT_RESTORED'; END IF;
END $$;

INSERT INTO academic_audit_events(event_code,entity_type,entity_id,payload)
SELECT 'curriculum.hierarchy.reconciled','curriculum_version',m.old_version_id,
       jsonb_build_object('migration',92,'passed',true,'programs',count(*))
FROM curriculum_hierarchy_mapping m
JOIN curriculum_hierarchy_mapping source ON source.anchor_code=m.old_code
WHERE m.old_code=m.anchor_code
GROUP BY m.old_version_id;
