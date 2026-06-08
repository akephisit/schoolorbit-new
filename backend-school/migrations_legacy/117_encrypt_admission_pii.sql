-- Store admission national IDs as app-side AES-GCM ciphertext and use blind hashes for lookup.
-- New admission data is expected to be clean-slate; existing plaintext rows should be cleared before this migration.

ALTER TABLE admission_applications
    ALTER COLUMN national_id TYPE TEXT,
    ALTER COLUMN father_national_id TYPE TEXT,
    ALTER COLUMN mother_national_id TYPE TEXT,
    ALTER COLUMN guardian_national_id TYPE TEXT;

ALTER TABLE admission_applications
    ADD COLUMN IF NOT EXISTS national_id_hash TEXT,
    ADD COLUMN IF NOT EXISTS father_national_id_hash TEXT,
    ADD COLUMN IF NOT EXISTS mother_national_id_hash TEXT,
    ADD COLUMN IF NOT EXISTS guardian_national_id_hash TEXT;

ALTER TABLE admission_applications
    DROP CONSTRAINT IF EXISTS unique_national_id_per_round;

DROP INDEX IF EXISTS idx_applications_national_id;

CREATE UNIQUE INDEX IF NOT EXISTS idx_applications_national_id_hash_round
    ON admission_applications(national_id_hash, admission_round_id)
    WHERE national_id_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_applications_national_id_hash
    ON admission_applications(national_id_hash);

CREATE INDEX IF NOT EXISTS idx_applications_father_national_id_hash
    ON admission_applications(father_national_id_hash)
    WHERE father_national_id_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_applications_mother_national_id_hash
    ON admission_applications(mother_national_id_hash)
    WHERE mother_national_id_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_applications_guardian_national_id_hash
    ON admission_applications(guardian_national_id_hash)
    WHERE guardian_national_id_hash IS NOT NULL;

ALTER TABLE admission_applications
    ALTER COLUMN national_id_hash SET NOT NULL;

COMMENT ON COLUMN admission_applications.national_id IS 'Encrypted applicant national ID (Base64 AES-GCM; app-side).';
COMMENT ON COLUMN admission_applications.national_id_hash IS 'Blind index for applicant national ID lookup and uniqueness.';
COMMENT ON COLUMN admission_applications.father_national_id IS 'Encrypted father national ID (Base64 AES-GCM; app-side).';
COMMENT ON COLUMN admission_applications.father_national_id_hash IS 'Blind index for father national ID lookup.';
COMMENT ON COLUMN admission_applications.mother_national_id IS 'Encrypted mother national ID (Base64 AES-GCM; app-side).';
COMMENT ON COLUMN admission_applications.mother_national_id_hash IS 'Blind index for mother national ID lookup.';
COMMENT ON COLUMN admission_applications.guardian_national_id IS 'Encrypted guardian national ID (Base64 AES-GCM; app-side).';
COMMENT ON COLUMN admission_applications.guardian_national_id_hash IS 'Blind index for guardian national ID lookup.';
