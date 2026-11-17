-- Active requests use full counted days or exact Exit minutes. Original policies,
-- request/revision documents, signatures, coverage snapshots and duties are evidence
-- and are intentionally not rewritten or converted. The mutable boundary is no
-- longer an input; accepted historical intervals retain their own snapshot.
-- RESTRICT refuses any unexpected dependency instead of cascading into evidence.
DROP TABLE staff_leave_coverage_settings RESTRICT;
