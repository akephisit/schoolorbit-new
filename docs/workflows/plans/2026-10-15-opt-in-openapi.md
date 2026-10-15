# Opt-in OpenAPI implementation plan

1. Preserve accepted OpenAPI/TypeScript hashes and inspect metadata owners and consumers.
2. Gate Utoipa dependencies, schema/parameter derives and attributes, manual schema implementations,
   application composition and export CLI behind default-off `api-docs` in existing owners.
3. Enable the feature in the local generator and contract/schema verification, preserving default
   production verification and explicit CLI rejection when export support is absent.
4. Update canonical rules/testing commands and benchmark export consumers coherently.
5. Run formatter, both workspace feature modes, contract/schema tests, byte equality and
   `./scripts/pipeline verify --scope auto`; inspect every changed file and diff.
6. Run a paired benchmark, record evidence in the implementation PR, remove these workflow inputs,
   and integrate through the Pipeline gate only after checks and useful performance evidence.
