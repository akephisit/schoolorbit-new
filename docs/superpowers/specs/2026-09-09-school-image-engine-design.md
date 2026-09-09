# School image engine extraction experiment

## Goal and scope

Measure whether extracting image codec work from `backend-school` reduces release
build time after an ordinary application-source edit. Keep one executable and one
production container. This is an experiment, not a commitment to split other features.

The user approved trying the image-engine boundary in conversation. This written
design requires review before implementation. No production deployment is part of
the benchmark experiment.

## Existing behavior

- `src/modules/files/file_inspector.rs` detects content from bytes, applies the
  purpose registry's limits, fully decodes images, and constructs `ValidatedFile`.
  That type binds inspection to an exact borrowed payload and has private fields.
- `src/utils/file_processor.rs` accepts `ValidatedFile` before decoding a derivative
  source and writes WebP output.
- `src/modules/files/platform_service.rs` chooses 256/1024 thumbnail recipes and
  resizes with Lanczos3 before encoding. It owns derivative metadata and storage work.
- `tests/static_architecture.rs` guards against bypassing the validated decoder.
- Docker currently caches cargo-chef dependencies, then copies all application
  source before compiling the final executable. A new crate alone would not prove
  cross-run reuse of its real compiled implementation.

## Chosen architecture

Add an unpublished library at `backend-school/crates/school-image-engine`, inside
a Cargo workspace rooted at the existing backend-school manifest. Keep the backend
as the default workspace member so existing commands still target the application;
run engine tests explicitly as well. Keep one existing Cargo.lock and do not upgrade
dependencies or change Rust, optimization, codec features, or release settings.

Dependency direction is backend-school -> school-image-engine -> image. The engine
must not depend on Axum, SQLx, AppState, file-purpose policy, credentials, or storage.

The engine owns concrete, non-generic codec functions for reading image dimensions,
decoding an explicitly selected format, encoding WebP, and Lanczos3 resize plus WebP
encoding. Use an opaque decoded-image wrapper with private fields so application
code need not operate on codec internals. Expose only the operations needed by the
current call sites. An engine result is decoded content, not proof of authorization
or successful purpose inspection.

The backend retains content detection, format selection, byte/dimension/pixel limits,
the exact order of validation, PDF/font inspection, and construction of ValidatedFile.
Its private validated decode method calls the engine using the same borrowed bytes.
ImageProcessor keeps its validated-input boundary. The platform service continues
to select recipes and create checksums, object keys, metadata, and lifecycle records.
Do not combine the two existing decode calls or otherwise optimize runtime behavior.

Map codec failures back to the current log-safe inspection and platform errors.
Do not expose underlying decoder errors or raw file data in logs/API responses.

## Security and compatibility

- Keep ValidatedFile fields private; do not add a public constructor or raw-payload
  accessor to make the extraction easier.
- Extend the architecture guard to cover engine calls: raw-byte image operations
  in application runtime code remain restricted to the inspector. The engine's
  codec implementation is the explicit lower-level exception, not a policy bypass.
- Keep API envelopes, OpenAPI, permission contracts, session/CSRF, tenant isolation,
  migrations, realtime, storage and malware-scanning behavior unchanged.
- Do not move or duplicate database pools, schedulers, shared state, or containers.
- Keep image dimensions, alpha handling, resize behavior including small images,
  output format, and byte output for fixed fixtures unchanged with the pinned codec.

## Build and cache design

Retain the existing dependency-oriented cargo-chef layer and school-specific GHA
BuildKit scope. After cooking dependencies, copy the real image-engine sources and
compile that package in a dedicated layer. Only then copy the remaining application
inputs and build backend-school.

Do not overwrite the engine sources with a later broad COPY. Explicitly account for
Cargo manifests, lockfile, build.rs, migrations and application src; inspect for any
additional compile-time assets before finalizing the copy list. Preserve the Cargo
timing export target and runtime image settings. Ensure engine compilation uses the
same resolved dependency features as the final build.

Prove both the engine layer hit and Cargo reuse of the engine artifact after an
application edit. Conversely, an engine-source edit must rebuild the engine and its
consumer; no cache key may hide that change. A cache miss must still build real code
and run all normal checks. Do not add sccache, incremental cache mounts, paid runners,
or new external cache services in this experiment.

## Verification and measurements

Before moving code, capture deterministic synthetic PNG/JPEG/WebP fixtures and
baseline derivative outputs, including alpha, non-square and smaller-than-target
images. Test malformed/truncated input, unsupported formats, and existing purpose
and dimension limits. Retain all inspector/platform tests; add focused engine tests
and call-site boundary checks. Compare original and extracted output on identical
fixtures, not only against expectations generated by the new implementation.

Run backend formatting, static architecture, cargo check, engine tests, inspector
and platform-service tests. Verify exported OpenAPI is unchanged. Apply every
deployment verification command required by .rules if Docker/CI changes are retained;
update the canonical testing/operations documentation and scoped architecture rules
with the implementation. Missing required checks must be reported, not substituted.

Benchmark original and extracted builds on the same machine and toolchain without
concurrent compilation. Record seed-build cost separately from warm-source rebuilds.
Use at least three paired warm trials with an actual application-code change, then
exercise an added and used Rust source file and an engine-source change. Record wall
time, Cargo timings, cache hits, and image size. An unchanged rebuild is a cache sanity
check, not the headline result. Do not reuse earlier one-off timings as the controlled
baseline or label local image export as a GHCR push measurement.

For remote-cache proof, use an isolated builder and a local registry/cache artifact
before any CI experiment; restore exported cache into a second builder to exclude
accidental reuse from the original builder. Local proof is not a measured GHA speedup.
Any later CI benchmark must be build-only and must not trigger production deployment.

## Decision and rollback

Recommend retention only if correctness checks pass, the engine is demonstrably
reused, and paired measurements show a repeatable benefit. Use a 10% median reduction
in warm total build time as the proposed practical threshold, and report individual
samples and spread. Smaller or noisy gains do not justify expanding the split.

If the gain is insufficient, report it and stop. Do not compensate by weakening
validation, reducing optimization, or expanding the migration without review.
Reverting the isolated implementation restores the previous build layout without
database changes or data recovery. Never prune unrelated images, caches, or volumes.

Alternatives considered: extracting tiny validation helpers has lower coupling but
uncertain compile benefit; extracting the complete file subsystem crosses storage
and security boundaries and is outside this first experiment.
