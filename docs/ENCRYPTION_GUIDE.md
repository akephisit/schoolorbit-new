# Data Encryption Guide

## Current Standard

SchoolOrbit stores sensitive national ID values with app-side AES-256-GCM encryption and keyed HMAC blind indexes.

- `ENCRYPTION_KEY` is used to encrypt/decrypt fields that must be read back.
- `BLIND_INDEX_KEY` is used to generate deterministic HMAC-SHA256 hashes for exact-match lookup and uniqueness checks.
- Do not reintroduce legacy PostgreSQL `pgcrypto` helpers for app fields.

## Protected Fields

- `users.national_id` stores encrypted ciphertext.
- `users.national_id_hash` stores the keyed blind index.
- `admission_applications.national_id` and parent national ID fields store encrypted ciphertext.
- `admission_applications.*_national_id_hash` stores keyed blind indexes.

## Setup

Generate separate keys:

```bash
openssl rand -base64 32 # ENCRYPTION_KEY
openssl rand -base64 32 # BLIND_INDEX_KEY
```

Set both keys in local and production environments:

```env
ENCRYPTION_KEY=your-generated-encryption-key
BLIND_INDEX_KEY=your-generated-blind-index-key
```

Both keys must be treated as secrets. Do not commit them. Store them in the same secret-management path as `JWT_SECRET` and database credentials.

## Application Flow

When accepting a national ID:

```rust
let encrypted = field_encryption::encrypt(national_id)?;
let hash = field_encryption::hash_for_search(national_id)?;
```

Write both values:

- encrypted value goes into `national_id`
- hash value goes into `national_id_hash`

When searching, logging in through admission portal, or checking duplicates:

```rust
let hash = field_encryption::hash_for_search(input_national_id)?;
// WHERE national_id_hash = hash
```

When returning data to a UI that is allowed to display the ID:

```rust
let plaintext = field_encryption::decrypt(&row.national_id)?;
```

Do not query encrypted columns directly for equality. AES-GCM uses a random nonce, so encrypting the same value twice produces different ciphertext.

## Key Rotation

Rotating `ENCRYPTION_KEY` requires decrypting existing ciphertext with the old key and re-encrypting it with the new key.

Rotating `BLIND_INDEX_KEY` requires decrypting each existing national ID and rebuilding every `*_national_id_hash` value with the new key.

If there is no retained user/admission national ID data, both keys can be changed before new data is entered.

## Troubleshooting

### `ENCRYPTION_KEY not set`

Set `ENCRYPTION_KEY` in the backend-school runtime environment.

### `BLIND_INDEX_KEY not set`

Set `BLIND_INDEX_KEY` in the backend-school runtime environment. Any path that writes/searches `national_id_hash` needs this key.

### Decryption failed

The ciphertext was encrypted with a different `ENCRYPTION_KEY`, or the stored value is not valid AES-GCM ciphertext.

### Search or duplicate checks do not find existing rows

The row may have a `national_id_hash` generated with a different `BLIND_INDEX_KEY` or the old unkeyed SHA-256 format. Rebuild the blind indexes from decrypted national IDs.
