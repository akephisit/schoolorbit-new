# Encryption Status

## Current Standard

Use app-side AES-256-GCM from `backend-school/src/utils/field_encryption.rs` for new encrypted fields.

Do not extend `backend-school/src/utils/encryption.rs` / PostgreSQL `pgcrypto`; that path is legacy.

## Completed

- `users.national_id` stores encrypted data with `users.national_id_hash` as a keyed HMAC blind index.
- Staff/student/parent creation paths use `field_encryption`.
- Admission applicant and parent national IDs now store encrypted data in `admission_applications` with keyed HMAC `*_national_id_hash` blind indexes.
- Migration `backend-school/migrations/117_encrypt_admission_pii.sql` widens admission national ID columns to `TEXT`, adds hash columns, and replaces the old plaintext uniqueness constraint.

## Operational Notes

- Keep `ENCRYPTION_KEY` stable after encrypted data exists. Rotating it without a re-encryption job makes existing ciphertext unreadable.
- Keep `BLIND_INDEX_KEY` stable after hash data exists. Rotating it requires reindexing all `*_national_id_hash` values from decrypted ciphertext.
- Search/login/duplicate checks must use blind hashes, not plaintext columns.
- Decrypt only at response boundaries where the UI is allowed to display the value.
- Admission was migrated as a clean-slate change after existing application data was cleared. If old plaintext rows ever need migration, write a dedicated backfill job before enforcing encrypted-only reads.
