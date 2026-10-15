-- Preserve existing language values and move qualifications into person-owned rows.
LOCK TABLE users, staff_info IN ACCESS EXCLUSIVE MODE;

CREATE TABLE staff_profile_cutover_audit (
    migration_version bigint PRIMARY KEY CHECK (migration_version = 114),
    user_count bigint NOT NULL,
    education_count bigint NOT NULL,
    users_fingerprint text NOT NULL,
    staff_fingerprint text NOT NULL,
    completed boolean NOT NULL DEFAULT false,
    completed_at timestamptz
);
INSERT INTO staff_profile_cutover_audit
SELECT 114, (SELECT count(*) FROM users),
    (SELECT count(*) FROM staff_info WHERE education_level IS NOT NULL OR major IS NOT NULL OR university IS NOT NULL),
    (SELECT md5(coalesce(jsonb_agg(to_jsonb(u) ORDER BY id),'[]'::jsonb)::text) FROM users u),
    (SELECT md5(coalesce(jsonb_agg(to_jsonb(i) - ARRAY['education_level','major','university'] ORDER BY id),'[]'::jsonb)::text) FROM staff_info i), false, NULL;

ALTER TABLE users ADD COLUMN title_en varchar(50),
    ADD COLUMN first_name_en varchar(100), ADD COLUMN last_name_en varchar(100);

CREATE FUNCTION resolve_person_name(title_th text, given_th text, family_th text,
    title_en text, given_en text, family_en text)
RETURNS TABLE(title text, first_name text, last_name text, name_language text, display_name text)
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    WITH choice AS (
        SELECT NOT (nullif(btrim(title_th),'') IS NOT NULL AND nullif(btrim(given_th),'') IS NOT NULL AND nullif(btrim(family_th),'') IS NOT NULL)
            AND nullif(btrim(title_en),'') IS NOT NULL AND nullif(btrim(given_en),'') IS NOT NULL AND nullif(btrim(family_en),'') IS NOT NULL AS english
    ), parts AS (
        SELECT coalesce(btrim(CASE WHEN english THEN title_en ELSE title_th END),'') AS title,
            coalesce(btrim(CASE WHEN english THEN given_en ELSE given_th END),'') AS given,
            coalesce(btrim(CASE WHEN english THEN family_en ELSE family_th END),'') AS family,
            english FROM choice
    ) SELECT title, given, family, CASE WHEN english THEN 'en' ELSE 'th' END,
        CASE WHEN english THEN concat_ws(' ', nullif(title,''), nullif(given,''), nullif(family,''))
            ELSE concat_ws(' ', nullif(title || given,''), nullif(family,'')) END FROM parts
$$;

-- A read projection, not a second persisted name owner. Raw fields remain in users.
CREATE VIEW user_display_names AS
SELECT u.id, u.national_id, u.national_id_hash, u.email, u.username, u.password_hash,
    CASE WHEN u.user_type='staff' THEN nullif(n.title,'') ELSE u.title END AS title,
    CASE WHEN u.user_type='staff' THEN n.first_name ELSE u.first_name END AS first_name,
    CASE WHEN u.user_type='staff' THEN n.last_name ELSE u.last_name END AS last_name, u.nickname, u.user_type, u.phone,
    u.emergency_contact, u.line_id, u.date_of_birth, u.gender, u.address,
    u.hired_date, u.resigned_date, u.status, u.metadata, u.created_at, u.updated_at,
    u.profile_image_file_id, u.title AS title_th, u.first_name AS first_name_th, u.last_name AS last_name_th, u.title_en, u.first_name_en, u.last_name_en, n.name_language,
    coalesce(nullif(n.display_name,''),u.username,u.id::text) AS display_name
FROM users u CROSS JOIN LATERAL resolve_person_name(u.title,u.first_name,u.last_name,
    CASE WHEN u.user_type='staff' THEN u.title_en END,
    CASE WHEN u.user_type='staff' THEN u.first_name_en END,
    CASE WHEN u.user_type='staff' THEN u.last_name_en END) n;

CREATE FUNCTION staff_education_tier(level text) RETURNS integer
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT CASE level WHEN 'primary' THEN 1 WHEN 'lower_secondary' THEN 2
        WHEN 'upper_secondary' THEN 3 WHEN 'vocational_certificate' THEN 3
        WHEN 'higher_vocational' THEN 4 WHEN 'diploma' THEN 4
        WHEN 'bachelor' THEN 5 WHEN 'master' THEN 6 WHEN 'doctorate' THEN 7 ELSE 0 END
$$;
ALTER TABLE staff_info ADD COLUMN education_revision bigint NOT NULL DEFAULT 0 CHECK (education_revision >= 0);
CREATE TABLE staff_educations (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    staff_id uuid NOT NULL REFERENCES staff_info(user_id) ON DELETE CASCADE,
    education_level varchar(100) CHECK (education_level IN ('primary','lower_secondary','upper_secondary',
        'vocational_certificate','higher_vocational','diploma','bachelor','master','doctorate','other')),
    major varchar(200) CHECK (major IS NULL OR (char_length(major)>0 AND major=btrim(major) AND major !~ '[[:cntrl:]]')),
    university varchar(200) CHECK (university IS NULL OR (char_length(university)>0 AND university=btrim(university) AND university !~ '[[:cntrl:]]')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (education_level IS NOT NULL OR major IS NOT NULL OR university IS NOT NULL)
);
CREATE INDEX staff_educations_owner ON staff_educations(staff_id);
INSERT INTO staff_educations(id,staff_id,education_level,major,university,created_at,updated_at)
SELECT id,user_id,education_level,major,university,created_at,updated_at FROM staff_info
WHERE education_level IS NOT NULL OR major IS NOT NULL OR university IS NOT NULL;

CREATE VIEW staff_highest_education AS
SELECT DISTINCT ON (staff_id) staff_id, education_level
-- Known tiers win; an other-only record stays a separate classification.
FROM staff_educations WHERE education_level IS NOT NULL
ORDER BY staff_id,staff_education_tier(education_level) DESC,education_level,id;
