-- An included summer/custom term cannot be omitted from year-closure readiness.
-- Repair only contradictory flags and invalidate previously fetched term versions.
UPDATE academic_terms
SET blocks_year_closure = true,
    row_version = row_version + 1,
    updated_at = now()
WHERE included_in_year_result AND NOT blocks_year_closure;

ALTER TABLE academic_terms
    ADD CONSTRAINT academic_terms_included_blocks_closure_check
    CHECK (NOT included_in_year_result OR blocks_year_closure);
