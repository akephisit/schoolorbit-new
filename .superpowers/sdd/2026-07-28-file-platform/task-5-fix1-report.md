# Task 5 Fix 1 Report — Security Review Repairs

## Important findings closed

1. clamd responses now use the `zINSTREAM` NUL convention: exactly one bounded NUL-terminated record is read under one overall read deadline. The parser recognizes only `stream: OK`, `stream: <detail> FOUND`, and `stream: <detail> ERROR`; missing terminators, malformed prefixes/suffixes, multiple records, and trailing bytes fail closed as `MalformedResponse`.
2. Inspection now returns `ValidatedFile<'a>`, which borrows the exact payload inspected. Image decoding and derivatives receive only that object; separate bytes and metadata cannot be supplied. The legacy file service now inspects before processing, and a static architecture guard blocks its former raw decoder calls.
3. PDF validation now requires a valid final numeric `startxref` offset pointing to either a parsed xref-table shape with a trailer `/Root` indirect reference, or an xref-stream object with `/Type /XRef`, `/W`, `/Length`, and `/Root`. The implementation remains bounded by the existing purpose byte ceiling and a 64 KiB terminal-structure window. No parser dependency was added because this narrow, bounded structural validation avoids a full document parser while meeting the upload gate requirements.
4. The scanner response read loop is wrapped in one `read_timeout`, so a slow drip cannot extend scan duration per byte/chunk.

## Added coverage

- NUL loopback clean, infected, unavailable/error, missing terminator, malformed prefix, embedded/multiple records, and trailing data.
- Slow-drip response deadline and bounded-response malformed outcome.
- JPEG/WebP truncation, invalid PDF offset, and accepted xref-stream fixture.
- Exact-payload validated derivative decoding and static proof that the legacy derivative path cannot invoke raw decoder helpers.

## TDD evidence

RED first failed because the intended one-argument validated decoder API did not yet exist:

```text
error[E0061]: this function takes 2 arguments but 1 argument was supplied
ImageProcessor::decode_inspected_image(&validated)
```

GREEN verification:

```text
cargo test modules::files::file_inspector --bin backend-school   # 6 passed
cargo test modules::files::malware_scanner --bin backend-school  # 7 passed
cargo test utils::file_processor --bin backend-school            # 1 passed
cargo test --test static_architecture                             # 119 passed
cargo fmt --all -- --check                                        # passed
cargo check                                                       # passed
git diff --check                                                  # passed
```

## Self-review

- New scanner failures carry only typed safe outcomes; no raw scanner response or endpoint is logged.
- `ValidatedFile` borrows rather than clones upload bytes.
- The PDF implementation does not interpret document content beyond the bounded terminal cross-reference structure.
- No backend-admin changes or migration edits were made.

## Commit

`fix: harden file inspection and scanner protocol`
