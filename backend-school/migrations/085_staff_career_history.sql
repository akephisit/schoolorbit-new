-- Canonical career history and transactional current projections.
-- The central tenant runner owns this transaction under release maintenance.
LOCK TABLE staff_info, users, staff_job_positions IN ACCESS EXCLUSIVE MODE;

CREATE TEMP TABLE staff_career_original_info ON COMMIT DROP AS
SELECT id, user_id, to_jsonb(info) AS original FROM staff_info info;

CREATE TABLE staff_career_history (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind varchar(32) NOT NULL CHECK (kind IN ('personnel_type','job_position','academic_rank')),
    personnel_type varchar(32) CHECK (personnel_type IN (
        'civil_servant','government_employee','contract_employee','permanent_employee','other'
    )),
    job_position_id uuid REFERENCES staff_job_positions(id),
    academic_rank varchar(32) CHECK (academic_rank IN (
        'none','not_applicable','proficient','senior_proficient','expert','senior_expert'
    )),
    effective_date date,
    order_date date,
    order_number varchar(100) CHECK (order_number IS NULL OR (
        char_length(order_number)>0 AND order_number=btrim(order_number) AND order_number !~ '[[:cntrl:]]'
    )),
    note varchar(1000) CHECK (note IS NULL OR (
        char_length(note)>0 AND note=btrim(note) AND note !~ '[[:cntrl:]]'
    )),
    source varchar(32) NOT NULL CHECK (source IN ('existing_record','staff_entry')),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision>0),
    created_by uuid REFERENCES users(id) ON DELETE SET NULL,
    updated_by uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT NOW(),
    updated_at timestamptz NOT NULL DEFAULT NOW(),
    CONSTRAINT staff_career_history_fact_check CHECK (
        (kind='personnel_type' AND job_position_id IS NULL AND academic_rank IS NULL)
        OR (kind='job_position' AND personnel_type IS NULL AND academic_rank IS NULL)
        OR (kind='academic_rank' AND personnel_type IS NULL AND job_position_id IS NULL)
    )
);

CREATE INDEX staff_career_history_staff_date_idx ON staff_career_history
    (user_id, effective_date DESC NULLS LAST, created_at DESC, id DESC);

ALTER TABLE staff_info
    ADD COLUMN personnel_type varchar(32) CHECK (personnel_type IN (
        'civil_servant','government_employee','contract_employee','permanent_employee','other'
    )),
    ADD COLUMN current_personnel_type_history_id uuid REFERENCES staff_career_history(id) DEFERRABLE INITIALLY DEFERRED,
    ADD COLUMN current_job_position_history_id uuid REFERENCES staff_career_history(id) DEFERRABLE INITIALLY DEFERRED,
    ADD COLUMN current_academic_rank_history_id uuid REFERENCES staff_career_history(id) DEFERRABLE INITIALLY DEFERRED;

INSERT INTO staff_career_history(user_id,kind,job_position_id,source)
SELECT user_id,'job_position',job_position_id,'existing_record'
FROM staff_info WHERE job_position_id IS NOT NULL;

INSERT INTO staff_career_history(user_id,kind,academic_rank,source)
SELECT user_id,'academic_rank',academic_rank,'existing_record'
FROM staff_info WHERE academic_rank IS NOT NULL;

-- Backfill pointers without pretending that import is an HR edit.
ALTER TABLE staff_info DISABLE TRIGGER update_staff_info_updated_at;
UPDATE staff_info info SET current_job_position_history_id=history.id
FROM staff_career_history history WHERE history.user_id=info.user_id AND history.kind='job_position';
UPDATE staff_info info SET current_academic_rank_history_id=history.id
FROM staff_career_history history WHERE history.user_id=info.user_id AND history.kind='academic_rank';
-- Flush deferred FK events before altering trigger state on this table.
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE staff_info ENABLE TRIGGER update_staff_info_updated_at;
SET CONSTRAINTS ALL DEFERRED;

CREATE FUNCTION assert_staff_career_current(staff_user_id uuid) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE info staff_info%ROWTYPE; current_fact record; history staff_career_history%ROWTYPE;
BEGIN
    SELECT * INTO info FROM staff_info WHERE user_id=staff_user_id;
    IF NOT FOUND THEN RETURN; END IF;
    FOR current_fact IN
        SELECT * FROM (VALUES
            ('personnel_type',info.current_personnel_type_history_id,info.personnel_type::text),
            ('job_position',info.current_job_position_history_id,info.job_position_id::text),
            ('academic_rank',info.current_academic_rank_history_id,info.academic_rank::text)
        ) AS facts(kind,entry_id,value)
    LOOP
        IF current_fact.entry_id IS NULL THEN
            IF current_fact.value IS NOT NULL THEN
                RAISE EXCEPTION 'STAFF_CAREER_CURRENT_REFERENCE_REQUIRED' USING ERRCODE='23514';
            END IF;
        ELSE
            SELECT * INTO history FROM staff_career_history WHERE id=current_fact.entry_id;
            IF NOT FOUND OR history.user_id<>staff_user_id OR history.kind<>current_fact.kind
                OR current_fact.value IS DISTINCT FROM (CASE history.kind
                    WHEN 'personnel_type' THEN history.personnel_type::text
                    WHEN 'job_position' THEN history.job_position_id::text
                    WHEN 'academic_rank' THEN history.academic_rank::text END)
            THEN
                RAISE EXCEPTION 'STAFF_CAREER_CURRENT_MISMATCH' USING ERRCODE='23514';
            END IF;
        END IF;
    END LOOP;
END $$;

CREATE FUNCTION check_staff_career_projection() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP<>'DELETE' THEN PERFORM assert_staff_career_current(NEW.user_id); END IF;
    IF TG_OP<>'INSERT' THEN PERFORM assert_staff_career_current(OLD.user_id); END IF;
    RETURN NULL;
END $$;

CREATE FUNCTION check_staff_career_history_owner() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP<>'DELETE' THEN
        IF EXISTS(SELECT 1 FROM staff_career_history WHERE id=NEW.id)
            AND NOT EXISTS(SELECT 1 FROM users WHERE id=NEW.user_id AND user_type='staff')
        THEN
            RAISE EXCEPTION 'STAFF_CAREER_STAFF_OWNER_REQUIRED' USING ERRCODE='23514';
        END IF;
        PERFORM assert_staff_career_current(NEW.user_id);
    END IF;
    IF TG_OP<>'INSERT' THEN PERFORM assert_staff_career_current(OLD.user_id); END IF;
    RETURN NULL;
END $$;

CREATE FUNCTION check_staff_career_user_type() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM users WHERE id=NEW.id AND user_type<>'staff')
        AND EXISTS(SELECT 1 FROM staff_career_history WHERE user_id=NEW.id)
    THEN RAISE EXCEPTION 'STAFF_CAREER_STAFF_OWNER_REQUIRED' USING ERRCODE='23514'; END IF;
    RETURN NULL;
END $$;

CREATE CONSTRAINT TRIGGER staff_career_projection_consistency
AFTER INSERT OR UPDATE OR DELETE ON staff_info DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION check_staff_career_projection();

CREATE CONSTRAINT TRIGGER staff_career_history_consistency
AFTER INSERT OR UPDATE OR DELETE ON staff_career_history DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION check_staff_career_history_owner();

CREATE CONSTRAINT TRIGGER staff_career_user_type_consistency
AFTER UPDATE OF user_type ON users DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION check_staff_career_user_type();

DO $$
DECLARE bad_count bigint; staff_user_id uuid;
BEGIN
    SELECT count(*) INTO bad_count FROM staff_career_original_info original
    FULL JOIN staff_info info ON info.id=original.id
    WHERE original.id IS NULL OR info.id IS NULL OR original.original IS DISTINCT FROM
        (to_jsonb(info)-ARRAY['personnel_type','current_personnel_type_history_id',
            'current_job_position_history_id','current_academic_rank_history_id']);
    IF bad_count<>0 THEN RAISE EXCEPTION 'STAFF_CAREER_PRESERVATION_FAILED count=%',bad_count; END IF;

    SELECT count(*) INTO bad_count FROM staff_career_history history
    LEFT JOIN users staff ON staff.id=history.user_id AND staff.user_type='staff'
    WHERE staff.id IS NULL OR history.effective_date IS NOT NULL OR history.order_date IS NOT NULL
        OR history.order_number IS NOT NULL OR history.created_by IS NOT NULL
        OR history.source<>'existing_record';
    IF bad_count<>0 THEN RAISE EXCEPTION 'STAFF_CAREER_IMPORT_INVALID count=%',bad_count; END IF;

    IF (SELECT count(*) FROM staff_career_history) <> (
        SELECT count(job_position_id)+count(academic_rank) FROM staff_info
    ) THEN RAISE EXCEPTION 'STAFF_CAREER_IMPORT_COUNT_MISMATCH'; END IF;
    FOR staff_user_id IN SELECT user_id FROM staff_info LOOP
        PERFORM assert_staff_career_current(staff_user_id);
    END LOOP;
END $$;
