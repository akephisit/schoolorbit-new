-- Preserve validated human cancel/acknowledgement reasons without putting private
-- prose into the searchable audit projection or notification payloads.
ALTER TABLE care_audit ADD COLUMN private_detail_ciphertext TEXT;
