# Domain HTTP route implementation

Base: `75aa47c46307a450b23c0b68b5dfb0c014e3f5d6`, branch `refactor/domain-http-routes`. Execute inline under `.rules` and the approved categorization scope in the paired design.

1. Inventory current route registrations and preserve a baseline outside the tracked tree for review. Add feature-owned registration modules; keep already-owned factories with their existing domains.
2. Add concrete Academic public/personal and Certificate public/protected factories to their existing HTTP crates. Bind their existing narrow states at root composition. Preserve public certificate headers, both 16 KiB auth limits, certificate 16 KiB limit, file upload override and global 20 MiB limit.
3. Reduce `app.rs` to global composition and platform boundaries. Keep session middleware applied once after every protected factory is bound.
4. Update architecture tests to inspect actual owners and retain route/OpenAPI coverage. Extend the existing composed-router fixture to cover domain protection and certificate public boundaries. Run formatting and focused checks, then `./scripts/pipeline verify --scope auto` with both feature modes and API artifact equality.
5. Review the full diff, open/attach a PR, record actual verification and limitations, remove these temporary workflow inputs after the PR records the outcome, and complete the existing merge/deployment/acceptance flow.
