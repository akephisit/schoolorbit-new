-- Close the JSON NULL loophole in the question-bank rich-content constraints.
-- Migration 025 intentionally rejected legacy documents, but missing JSON keys
-- evaluate to NULL and PostgreSQL CHECK constraints accept NULL. Normalize the
-- only lossless legacy placeholder and fail closed for every other legacy shape.

UPDATE academic_question_bank_questions
SET stem_content = '{"schemaVersion":1,"document":{"type":"doc","content":[]}}'::jsonb
WHERE stem_content = '{"blocks":[]}'::jsonb;

UPDATE academic_question_bank_questions
SET explanation_content = '{"schemaVersion":1,"document":{"type":"doc","content":[]}}'::jsonb
WHERE explanation_content = '{"blocks":[]}'::jsonb;

UPDATE academic_question_bank_questions
SET rubric_content = '{"schemaVersion":1,"document":{"type":"doc","content":[]}}'::jsonb
WHERE rubric_content = '{"blocks":[]}'::jsonb;

UPDATE academic_question_bank_choices
SET content = '{"schemaVersion":1,"document":{"type":"doc","content":[]}}'::jsonb
WHERE content = '{"blocks":[]}'::jsonb;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM academic_question_bank_questions q
        WHERE NOT ((
            jsonb_typeof(q.stem_content) = 'object'
            AND jsonb_typeof(q.stem_content -> 'schemaVersion') = 'number'
            AND q.stem_content ->> 'schemaVersion' = '1'
            AND jsonb_typeof(q.stem_content -> 'document') = 'object'
            AND q.stem_content -> 'document' ->> 'type' = 'doc'
            AND jsonb_typeof(q.stem_content -> 'document' -> 'content') = 'array'
        ) IS TRUE)
        OR (
            q.explanation_content IS NOT NULL
            AND NOT ((
                jsonb_typeof(q.explanation_content) = 'object'
                AND jsonb_typeof(q.explanation_content -> 'schemaVersion') = 'number'
                AND q.explanation_content ->> 'schemaVersion' = '1'
                AND jsonb_typeof(q.explanation_content -> 'document') = 'object'
                AND q.explanation_content -> 'document' ->> 'type' = 'doc'
                AND jsonb_typeof(q.explanation_content -> 'document' -> 'content') = 'array'
            ) IS TRUE)
        )
        OR (
            q.rubric_content IS NOT NULL
            AND NOT ((
                jsonb_typeof(q.rubric_content) = 'object'
                AND jsonb_typeof(q.rubric_content -> 'schemaVersion') = 'number'
                AND q.rubric_content ->> 'schemaVersion' = '1'
                AND jsonb_typeof(q.rubric_content -> 'document') = 'object'
                AND q.rubric_content -> 'document' ->> 'type' = 'doc'
                AND jsonb_typeof(q.rubric_content -> 'document' -> 'content') = 'array'
            ) IS TRUE)
        )
    )
        OR EXISTS (
            SELECT 1
            FROM academic_question_bank_choices c
            WHERE NOT ((
                jsonb_typeof(c.content) = 'object'
                AND jsonb_typeof(c.content -> 'schemaVersion') = 'number'
                AND c.content ->> 'schemaVersion' = '1'
                AND jsonb_typeof(c.content -> 'document') = 'object'
                AND c.content -> 'document' ->> 'type' = 'doc'
                AND jsonb_typeof(c.content -> 'document' -> 'content') = 'array'
            ) IS TRUE)
        )
    THEN
        RAISE EXCEPTION
            'QUESTION_BANK_046_UNSUPPORTED_LEGACY_RICH_CONTENT';
    END IF;
END
$$;

ALTER TABLE academic_question_bank_questions
    DROP CONSTRAINT academic_question_bank_questions_stem_content_check,
    DROP CONSTRAINT academic_question_bank_questions_explanation_content_check,
    DROP CONSTRAINT academic_question_bank_questions_rubric_content_check,
    ADD CONSTRAINT academic_question_bank_questions_stem_content_check CHECK ((
        jsonb_typeof(stem_content) = 'object'
        AND jsonb_typeof(stem_content -> 'schemaVersion') = 'number'
        AND stem_content ->> 'schemaVersion' = '1'
        AND jsonb_typeof(stem_content -> 'document') = 'object'
        AND stem_content -> 'document' ->> 'type' = 'doc'
        AND jsonb_typeof(stem_content -> 'document' -> 'content') = 'array'
    ) IS TRUE),
    ADD CONSTRAINT academic_question_bank_questions_explanation_content_check CHECK (
        explanation_content IS NULL OR ((
            jsonb_typeof(explanation_content) = 'object'
            AND jsonb_typeof(explanation_content -> 'schemaVersion') = 'number'
            AND explanation_content ->> 'schemaVersion' = '1'
            AND jsonb_typeof(explanation_content -> 'document') = 'object'
            AND explanation_content -> 'document' ->> 'type' = 'doc'
            AND jsonb_typeof(explanation_content -> 'document' -> 'content') = 'array'
        ) IS TRUE)
    ),
    ADD CONSTRAINT academic_question_bank_questions_rubric_content_check CHECK (
        rubric_content IS NULL OR ((
            jsonb_typeof(rubric_content) = 'object'
            AND jsonb_typeof(rubric_content -> 'schemaVersion') = 'number'
            AND rubric_content ->> 'schemaVersion' = '1'
            AND jsonb_typeof(rubric_content -> 'document') = 'object'
            AND rubric_content -> 'document' ->> 'type' = 'doc'
            AND jsonb_typeof(rubric_content -> 'document' -> 'content') = 'array'
        ) IS TRUE)
    );

ALTER TABLE academic_question_bank_choices
    DROP CONSTRAINT academic_question_bank_choices_content_check,
    ADD CONSTRAINT academic_question_bank_choices_content_check CHECK ((
        jsonb_typeof(content) = 'object'
        AND jsonb_typeof(content -> 'schemaVersion') = 'number'
        AND content ->> 'schemaVersion' = '1'
        AND jsonb_typeof(content -> 'document') = 'object'
        AND content -> 'document' ->> 'type' = 'doc'
        AND jsonb_typeof(content -> 'document' -> 'content') = 'array'
    ) IS TRUE);
