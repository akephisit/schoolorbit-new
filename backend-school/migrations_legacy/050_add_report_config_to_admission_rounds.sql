ALTER TABLE admission_rounds
    ADD COLUMN IF NOT EXISTS report_config JSONB DEFAULT NULL;
