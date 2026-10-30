-- Visibility controls discovery; operational lifecycle remains independently owned.
LOCK TABLE academic_years, academic_terms IN SHARE ROW EXCLUSIVE MODE;

ALTER TABLE academic_years ADD COLUMN is_visible boolean NOT NULL DEFAULT false;
ALTER TABLE academic_terms ADD COLUMN is_visible boolean NOT NULL DEFAULT false;

-- Preserve the old public endpoint's unpublished intent once, then never couple
-- visibility to lifecycle again. Managers retain explicit access to hidden rows.
ALTER TABLE academic_years DISABLE TRIGGER update_academic_years_updated_at;
ALTER TABLE academic_terms DISABLE TRIGGER update_academic_semesters_updated_at;
UPDATE academic_years SET is_visible = status <> 'planning';
UPDATE academic_terms SET is_visible = status NOT IN ('planning','cancelled');
ALTER TABLE academic_years ENABLE TRIGGER update_academic_years_updated_at;
ALTER TABLE academic_terms ENABLE TRIGGER update_academic_semesters_updated_at;

CREATE TABLE academic_context_settings (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    default_academic_year_id uuid REFERENCES academic_years(id) ON DELETE RESTRICT,
    default_academic_term_id uuid,
    row_version bigint NOT NULL DEFAULT 1 CHECK (row_version > 0),
    updated_by uuid REFERENCES users(id) ON DELETE RESTRICT,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (default_academic_term_id IS NULL OR default_academic_year_id IS NOT NULL),
    FOREIGN KEY (default_academic_term_id,default_academic_year_id)
        REFERENCES academic_terms(id,academic_year_id) ON DELETE RESTRICT
);
DO $$ BEGIN
    IF (SELECT count(*) FROM academic_years WHERE status IN ('active','closing'))>1
       OR (SELECT count(*) FROM academic_terms WHERE status IN ('active','closing'))>1
       OR EXISTS (
           SELECT 1 FROM academic_terms term WHERE term.status IN ('active','closing')
             AND NOT EXISTS (SELECT 1 FROM academic_years year WHERE year.id=term.academic_year_id AND year.status IN ('active','closing'))
       ) THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_146_RUNNING_CONTEXT_MISMATCH'; END IF;
END $$;
INSERT INTO academic_context_settings(singleton,default_academic_year_id,default_academic_term_id)
SELECT true,
       (SELECT id FROM academic_years WHERE status IN ('active','closing')),
       (SELECT id FROM academic_terms WHERE status IN ('active','closing'));

CREATE FUNCTION academic_context_validate_visible_defaults()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    selected_year uuid;
    selected_term uuid;
BEGIN
    SELECT default_academic_year_id,default_academic_term_id
      INTO selected_year,selected_term
      FROM academic_context_settings WHERE singleton FOR UPDATE;
    IF (selected_year IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM academic_years WHERE id=selected_year AND is_visible
    )) OR (selected_term IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id
        WHERE term.id=selected_term AND term.academic_year_id=selected_year
          AND term.is_visible AND year.is_visible
    )) THEN
        RAISE EXCEPTION 'ACADEMIC_CONTEXT_DEFAULT_MUST_BE_VISIBLE' USING ERRCODE='check_violation';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER academic_context_settings_visible_defaults
AFTER INSERT OR UPDATE ON academic_context_settings DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION academic_context_validate_visible_defaults();
CREATE CONSTRAINT TRIGGER academic_years_visible_defaults
AFTER UPDATE OF is_visible ON academic_years DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION academic_context_validate_visible_defaults();
CREATE CONSTRAINT TRIGGER academic_terms_visible_defaults
AFTER UPDATE OF is_visible ON academic_terms DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION academic_context_validate_visible_defaults();

DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM academic_context_settings settings
        WHERE settings.default_academic_term_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM academic_terms term WHERE term.id=settings.default_academic_term_id
              AND term.academic_year_id=settings.default_academic_year_id
        )
    ) THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_146_RUNNING_CONTEXT_MISMATCH'; END IF;
END $$;
