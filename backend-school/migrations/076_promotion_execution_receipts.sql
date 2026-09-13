CREATE TABLE academic_promotion_execution_batches (
    request_id UUID PRIMARY KEY,
    run_id UUID NOT NULL REFERENCES academic_promotion_runs(id) ON DELETE RESTRICT,
    approval_id UUID NOT NULL,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    item_ids UUID[] NOT NULL CHECK (cardinality(item_ids) BETWEEN 1 AND 100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(request_id,run_id,approval_id),
    FOREIGN KEY(approval_id,run_id) REFERENCES academic_promotion_run_approvals(id,run_id) ON DELETE RESTRICT
);
CREATE TRIGGER academic_promotion_execution_batches_immutable
BEFORE UPDATE OR DELETE ON academic_promotion_execution_batches
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();

CREATE TABLE academic_promotion_execution_receipts (
    item_id UUID PRIMARY KEY,
    run_id UUID NOT NULL,
    request_id UUID NOT NULL,
    approval_id UUID NOT NULL,
    source_year_id UUID NOT NULL,
    target_year_id UUID NOT NULL,
    student_id UUID NOT NULL,
    target_student_year_id UUID,
    target_placement_id UUID,
    source_row_version BIGINT NOT NULL CHECK(source_row_version > 0),
    executed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(item_id,run_id,source_year_id,target_year_id,student_id)
        REFERENCES academic_promotion_run_items(id,run_id,source_year_id,target_year_id,student_id) ON DELETE RESTRICT,
    FOREIGN KEY(request_id,run_id,approval_id)
        REFERENCES academic_promotion_execution_batches(request_id,run_id,approval_id) ON DELETE RESTRICT,
    FOREIGN KEY(target_student_year_id,target_year_id,student_id)
        REFERENCES student_academic_years(id,academic_year_id,student_id) ON DELETE RESTRICT,
    FOREIGN KEY(target_placement_id,target_student_year_id)
        REFERENCES homeroom_placements(id,student_academic_year_id) ON DELETE RESTRICT,
    CHECK(target_placement_id IS NULL OR target_student_year_id IS NOT NULL)
);
CREATE TRIGGER academic_promotion_execution_receipts_immutable
BEFORE UPDATE OR DELETE ON academic_promotion_execution_receipts
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
CREATE INDEX academic_promotion_execution_receipts_run_idx ON academic_promotion_execution_receipts(run_id);
