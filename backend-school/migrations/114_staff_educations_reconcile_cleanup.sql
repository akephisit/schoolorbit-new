-- A fresh, locked comparison is required before removing the single-value owner.
LOCK TABLE users, staff_info, staff_educations, staff_profile_cutover_audit IN ACCESS EXCLUSIVE MODE;
DO $$
DECLARE evidence staff_profile_cutover_audit%ROWTYPE;
BEGIN
    SELECT * INTO evidence FROM staff_profile_cutover_audit WHERE migration_version=114;
    IF evidence.migration_version IS NULL OR evidence.completed
       OR evidence.user_count<>(SELECT count(*) FROM users)
       OR evidence.education_count<>(SELECT count(*) FROM staff_educations)
       OR evidence.users_fingerprint IS DISTINCT FROM (SELECT md5(coalesce(jsonb_agg(to_jsonb(u)-ARRAY['title_en','first_name_en','last_name_en'] ORDER BY id),'[]'::jsonb)::text) FROM users u)
       OR EXISTS(SELECT 1 FROM users WHERE title_en IS NOT NULL OR first_name_en IS NOT NULL OR last_name_en IS NOT NULL)
       OR evidence.staff_fingerprint IS DISTINCT FROM (SELECT md5(coalesce(jsonb_agg(to_jsonb(i)-ARRAY['education_level','major','university','education_revision'] ORDER BY id),'[]'::jsonb)::text) FROM staff_info i)
       OR EXISTS(SELECT 1 FROM staff_info i LEFT JOIN staff_educations e ON e.id=i.id
           WHERE (i.education_level IS NOT NULL OR i.major IS NOT NULL OR i.university IS NOT NULL)
           AND (e.id IS NULL OR e.staff_id IS DISTINCT FROM i.user_id
             OR e.education_level IS DISTINCT FROM i.education_level OR e.major IS DISTINCT FROM i.major
             OR e.university IS DISTINCT FROM i.university OR e.created_at IS DISTINCT FROM i.created_at
             OR e.updated_at IS DISTINCT FROM i.updated_at))
       OR EXISTS(SELECT 1 FROM staff_educations e JOIN users u ON u.id=e.staff_id WHERE u.user_type<>'staff')
       OR EXISTS(SELECT 1 FROM staff_info WHERE education_revision<>0) THEN
        RAISE EXCEPTION 'STAFF_PROFILE_RECONCILIATION_REQUIRED';
    END IF;
END $$;
ALTER TABLE staff_info DROP COLUMN education_level, DROP COLUMN major, DROP COLUMN university;
UPDATE staff_profile_cutover_audit SET completed=true,completed_at=now() WHERE migration_version=114;
