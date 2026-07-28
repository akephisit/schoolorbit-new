# Task 5 Fix Round 2 — Inline Completion Report

## Closed findings

1. The `zINSTREAM` reader keeps the original overall read deadline and now
   reads through EOF after the first NUL record. Any byte after the first NUL,
   including a delayed second record, is rejected as `MalformedResponse`.
2. PDF validation now uses `lopdf` with default features disabled after the
   purpose-owned 20 MiB byte limit. The upload gate additionally requires:
   - a final bounded `%%EOF`;
   - a declared `/Size` equal to the parsed reference table and no more than
     1,000,000 objects;
   - every active normal/compressed xref entry to resolve to the matching
     parsed object;
   - a resolvable `/Root` catalog and `/Pages` tree.
3. The legacy image compatibility path now carries canonical output metadata.
   PNG, WebP, and JPEG inputs are converted to JPEG and the R2 content type,
   object extension, and database filename all use `image/jpeg` and `.jpg`,
   independent of submitted filename or MIME.
4. The validated-image static guard now scans the backend production tree
   rather than only the legacy service module.

## Regression coverage

- exact valid xref-table and xref-stream PDF fixtures;
- invalid object offset, unresolved root, and inconsistent xref-stream length;
- delayed post-NUL scanner bytes after a scheduling gap;
- PNG, WebP, and JPEG processing with spoofed submitted metadata, checking
  JPEG magic bytes, canonical MIME, object path, and stored filename;
- repository-wide raw image decoder boundary.

## Verification

```text
cargo test modules::files::file_inspector --bin backend-school  # 6 passed
cargo test modules::files::malware_scanner --bin backend-school # 8 passed
cargo test modules::files::services::tests --bin backend-school # 3 passed
cargo test utils::file_processor --bin backend-school           # 1 passed
cargo test --test static_architecture                           # 119 passed
cargo fmt --all -- --check                                      # passed
cargo check                                                     # passed
git diff --check                                                # passed
cargo tree -e features -i lopdf                                 # no lopdf default features
```

## Safety and scope

- Parser failures are mapped to the detail-free `MalformedContent` outcome.
- No file bytes, scanner reply, endpoint, credentials, object key, URL, or
  plaintext national ID are logged by this change.
- No migration, `backend-admin`, or `frontend-admin` file changed.
