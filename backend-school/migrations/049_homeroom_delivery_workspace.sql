ALTER TABLE learning_groups
    ADD COLUMN generation_source TEXT NOT NULL DEFAULT 'manual',
    ADD COLUMN generation_key TEXT,
    ADD CONSTRAINT learning_groups_generation_source_check
        CHECK (generation_source IN ('manual', 'curriculum_prepare')),
    ADD CONSTRAINT learning_groups_generation_shape_check CHECK (
        (generation_source = 'manual' AND generation_key IS NULL)
        OR (
            generation_source = 'curriculum_prepare'
            AND generation_key IS NOT NULL
            AND btrim(generation_key) <> ''
        )
    );

CREATE UNIQUE INDEX learning_groups_curriculum_generation_key
    ON learning_groups (academic_term_id, learning_offering_id, generation_key)
    WHERE generation_source = 'curriculum_prepare';
