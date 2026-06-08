-- Clarify that national ID blind indexes are keyed HMAC-SHA256 values.
-- Do not edit older migrations that have already been applied; add comment changes here.

COMMENT ON COLUMN users.national_id_hash IS
    'Blind index for national ID lookup and uniqueness (HMAC-SHA256 hex; keyed by BLIND_INDEX_KEY).';

COMMENT ON COLUMN admission_applications.national_id_hash IS
    'Blind index for applicant national ID lookup and uniqueness (HMAC-SHA256 hex; keyed by BLIND_INDEX_KEY).';

COMMENT ON COLUMN admission_applications.father_national_id_hash IS
    'Blind index for father national ID lookup (HMAC-SHA256 hex; keyed by BLIND_INDEX_KEY).';

COMMENT ON COLUMN admission_applications.mother_national_id_hash IS
    'Blind index for mother national ID lookup (HMAC-SHA256 hex; keyed by BLIND_INDEX_KEY).';

COMMENT ON COLUMN admission_applications.guardian_national_id_hash IS
    'Blind index for guardian national ID lookup (HMAC-SHA256 hex; keyed by BLIND_INDEX_KEY).';
