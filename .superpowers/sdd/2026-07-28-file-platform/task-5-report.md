# Task 5 Report — Content Inspection and Malware Scanning

## Files changed

- `backend-school/src/modules/files.rs`
- `backend-school/src/modules/files/file_inspector.rs`
- `backend-school/src/modules/files/malware_scanner.rs`
- `backend-school/src/utils/file_processor.rs`

## Interfaces

- `inspect_file(purpose, data) -> Result<InspectedFile, FileInspectionError>` detects only PNG, JPEG, WebP, and PDF from bytes. `InspectedFile` owns the detected type, canonical MIME/extension, and optional dimensions; its fields are private so callers cannot forge an inspection.
- `MalwareScanner` is an async provider-neutral port returning only `Clean`, `Infected`, `Unavailable`, `Timeout`, or `MalformedResponse`.
- `ClamdScanner` implements clamd `zINSTREAM\\0` framing with network-order chunk lengths, a terminating zero-length chunk, and separate connect/write/read timeouts. It bounds each chunk and the total response.
- `scan_allows_readiness(purpose, outcome)` returns true only for `Clean`; every initial purpose is checked by test.
- `ImageProcessor::decode_inspected_image` accepts an `InspectedFile`, so new derivative callers can only decode content that passed inspection.

## TDD evidence

RED:

```text
cargo test modules::files::file_inspector --bin backend-school
cargo test modules::files::malware_scanner --bin backend-school
error[E0432]: unresolved imports ... inspect_file, FileInspectionError,
ClamdConfig, ClamdScanner, MalwareScanner, ScanOutcome
```

The failure was the intended missing inspector/scanner API state.

GREEN:

```text
cargo test modules::files::file_inspector --bin backend-school   # 4 passed
cargo test modules::files::malware_scanner --bin backend-school  # 5 passed
cargo test utils::file_processor --bin backend-school            # 1 passed
cargo test --test static_architecture                             # 118 passed
cargo fmt --all -- --check                                        # passed
cargo check                                                       # passed
git diff --check                                                  # passed
```

## Safety limits and handling

- Purpose-registry `max_bytes` is checked before signature parsing; width, height, and decoded-pixel ceilings are checked before image decode.
- PNG requires signature, IHDR, and IEND; JPEG requires SOI/marker/EOI; WebP requires RIFF/WEBP, a matching declared size, and an image chunk. All images fully decode only after limits pass.
- PDFs require a valid version header and a bounded 64 KiB trailer window containing `trailer`, `/Root`, `startxref`, and terminal `%%EOF`; this is validation, not a full PDF parser.
- clamd responses are bounded by `max_response_bytes`; oversized or unrecognizable replies become `MalformedResponse`. Transport failures reveal no endpoint or response content and fail closed.
- No file bytes, submitted filenames, scanner raw responses, object keys, URLs, or credentials are logged by the new code.

## Self-review

- Confirmed no runtime `unwrap`/`expect` was added.
- Confirmed MIME and filename do not enter the inspection interface.
- Confirmed only typed, non-sensitive scanner outcomes leave the adapter.
- Confirmed `InspectedFile` is non-forgeable outside its module.
- Task 6 remains responsible for wiring the staged inspector/scanner contracts into the durable upload lifecycle and startup configuration.

## Commit

`feat: inspect and scan file content`
