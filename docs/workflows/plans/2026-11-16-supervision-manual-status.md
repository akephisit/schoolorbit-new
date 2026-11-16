# Manual supervision status implementation

1. Update school-supervision models, SQL and request policy; strictly reject retired
   cycle API fields. Preserve academic context locks and submitted evaluation evidence.
2. Add forward-only migration 159 with RESTRICT and retained-data reconciliation.
3. Remove supervision-only preparation date requirements and copy draft configuration.
4. Update frontend cycle forms/booking and returned correction flow, exposing current
   stage and next responsible person while retaining return evidence.
5. Verify domain policy, service database flows, populated migration preservation and
   dependency refusal, preparation without cycle dates, generated OpenAPI/TypeScript,
   frontend static/type/build/browser coverage and independent review/QA.

Focused backend commands: `./scripts/test_backend_school.sh supervision -- --test-threads=1`,
`./scripts/test_backend_school.sh term_preparation_tests -- --test-threads=1`, and
`cargo test -p school-supervision`. Use the canonical API generator after source models
stabilize. Senior coordinates the shared Cargo/generator slot and owns Git checkpoints.
