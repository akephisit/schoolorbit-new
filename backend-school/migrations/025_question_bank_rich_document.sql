-- Store question content as a versioned ProseMirror-compatible document.
-- HTML is intentionally not persisted; clients render the typed JSON tree safely.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM academic_question_bank_questions q
        WHERE NOT (
            jsonb_typeof(q.stem_content) = 'object'
            AND jsonb_typeof(q.stem_content -> 'schemaVersion') = 'number'
            AND q.stem_content ->> 'schemaVersion' = '1'
            AND jsonb_typeof(q.stem_content -> 'document') = 'object'
            AND q.stem_content -> 'document' ->> 'type' = 'doc'
            AND jsonb_typeof(q.stem_content -> 'document' -> 'content') = 'array'
        )
        OR q.explanation_content IS NOT NULL AND NOT (
            jsonb_typeof(q.explanation_content) = 'object'
            AND jsonb_typeof(q.explanation_content -> 'schemaVersion') = 'number'
            AND q.explanation_content ->> 'schemaVersion' = '1'
            AND jsonb_typeof(q.explanation_content -> 'document') = 'object'
            AND q.explanation_content -> 'document' ->> 'type' = 'doc'
            AND jsonb_typeof(q.explanation_content -> 'document' -> 'content') = 'array'
        )
        OR q.rubric_content IS NOT NULL AND NOT (
            jsonb_typeof(q.rubric_content) = 'object'
            AND jsonb_typeof(q.rubric_content -> 'schemaVersion') = 'number'
            AND q.rubric_content ->> 'schemaVersion' = '1'
            AND jsonb_typeof(q.rubric_content -> 'document') = 'object'
            AND q.rubric_content -> 'document' ->> 'type' = 'doc'
            AND jsonb_typeof(q.rubric_content -> 'document' -> 'content') = 'array'
        )
        OR EXISTS (
            SELECT 1
            FROM academic_question_bank_choices c
            WHERE NOT (
                jsonb_typeof(c.content) = 'object'
                AND jsonb_typeof(c.content -> 'schemaVersion') = 'number'
                AND c.content ->> 'schemaVersion' = '1'
                AND jsonb_typeof(c.content -> 'document') = 'object'
                AND c.content -> 'document' ->> 'type' = 'doc'
                AND jsonb_typeof(c.content -> 'document' -> 'content') = 'array'
            )
        )
    THEN
        RAISE EXCEPTION
            'Question bank contains legacy content. Migrate it explicitly before applying migration 025.';
    END IF;
END
$$;

ALTER TABLE academic_question_bank_questions
    DROP CONSTRAINT academic_question_bank_questions_stem_content_check;

ALTER TABLE academic_question_bank_choices
    DROP CONSTRAINT academic_question_bank_choices_content_check;

ALTER TABLE academic_question_bank_questions
    ALTER COLUMN stem_content SET DEFAULT
        '{"schemaVersion":1,"document":{"type":"doc","content":[]}}'::jsonb,
    ADD COLUMN search_text TEXT NOT NULL DEFAULT '',
    ADD CONSTRAINT academic_question_bank_questions_stem_content_check CHECK (
        jsonb_typeof(stem_content) = 'object'
        AND jsonb_typeof(stem_content -> 'schemaVersion') = 'number'
        AND stem_content ->> 'schemaVersion' = '1'
        AND jsonb_typeof(stem_content -> 'document') = 'object'
        AND stem_content -> 'document' ->> 'type' = 'doc'
        AND jsonb_typeof(stem_content -> 'document' -> 'content') = 'array'
    ),
    ADD CONSTRAINT academic_question_bank_questions_explanation_content_check CHECK (
        explanation_content IS NULL OR (
            jsonb_typeof(explanation_content) = 'object'
            AND jsonb_typeof(explanation_content -> 'schemaVersion') = 'number'
            AND explanation_content ->> 'schemaVersion' = '1'
            AND jsonb_typeof(explanation_content -> 'document') = 'object'
            AND explanation_content -> 'document' ->> 'type' = 'doc'
            AND jsonb_typeof(explanation_content -> 'document' -> 'content') = 'array'
        )
    ),
    ADD CONSTRAINT academic_question_bank_questions_rubric_content_check CHECK (
        rubric_content IS NULL OR (
            jsonb_typeof(rubric_content) = 'object'
            AND jsonb_typeof(rubric_content -> 'schemaVersion') = 'number'
            AND rubric_content ->> 'schemaVersion' = '1'
            AND jsonb_typeof(rubric_content -> 'document') = 'object'
            AND rubric_content -> 'document' ->> 'type' = 'doc'
            AND jsonb_typeof(rubric_content -> 'document' -> 'content') = 'array'
        )
    );

ALTER TABLE academic_question_bank_choices
    ALTER COLUMN content SET DEFAULT
        '{"schemaVersion":1,"document":{"type":"doc","content":[]}}'::jsonb,
    ADD CONSTRAINT academic_question_bank_choices_content_check CHECK (
        jsonb_typeof(content) = 'object'
        AND jsonb_typeof(content -> 'schemaVersion') = 'number'
        AND content ->> 'schemaVersion' = '1'
        AND jsonb_typeof(content -> 'document') = 'object'
        AND content -> 'document' ->> 'type' = 'doc'
        AND jsonb_typeof(content -> 'document' -> 'content') = 'array'
    );

DROP INDEX IF EXISTS idx_question_bank_questions_stem_trgm;
DROP INDEX IF EXISTS idx_question_bank_questions_stem_content;

CREATE INDEX idx_question_bank_questions_search_trgm
    ON academic_question_bank_questions
    USING GIN (search_text gin_trgm_ops)
    WHERE deleted_at IS NULL;

COMMENT ON COLUMN academic_question_bank_questions.stem_content IS
    'Versioned structured question document. schemaVersion 1 uses typed text, inline_math, math_block, and image nodes.';

COMMENT ON COLUMN academic_question_bank_questions.search_text IS
    'Plain text and LaTeX projection of stem_content maintained by the application for question search.';

COMMENT ON COLUMN academic_question_bank_choices.content IS
    'Versioned structured choice document using the same schema as question stem_content.';
