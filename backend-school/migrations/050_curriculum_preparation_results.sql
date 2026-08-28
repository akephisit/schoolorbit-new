-- Persist the complete idempotent result of curriculum preparation, including
-- generated learning groups, so retries return the same normalized outcome.

ALTER TABLE learning_delivery_apply_runs
    ADD COLUMN group_ids UUID[] NOT NULL DEFAULT ARRAY[]::uuid[],
    ADD COLUMN created_offering_count INTEGER NOT NULL DEFAULT 0
        CHECK (created_offering_count >= 0),
    ADD COLUMN retained_offering_count INTEGER NOT NULL DEFAULT 0
        CHECK (retained_offering_count >= 0),
    ADD COLUMN created_group_count INTEGER NOT NULL DEFAULT 0
        CHECK (created_group_count >= 0),
    ADD COLUMN retained_group_count INTEGER NOT NULL DEFAULT 0
        CHECK (retained_group_count >= 0),
    ADD COLUMN skipped_count INTEGER NOT NULL DEFAULT 0
        CHECK (skipped_count >= 0);
