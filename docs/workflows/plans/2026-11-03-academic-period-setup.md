# Academic period setup implementation

1. Backend period writer owns capacity-only operations, schedule preparation guards,
   exact diagnostics and their focused tests. Data writer owns additive migration
   154, annual membership, date-free manual rollover and interval consumers.
   Shared models and student-year services transfer through explicit handoffs.
2. Preserve old rows and immutable ledgers with migration fingerprints. Introduce
   annual membership scope, nullable civil starts and a shared interval predicate.
3. Remove rollover dates at DTO, preview, checksum, command, persistence and audit
   owners. Preserve replay, hold resolution, capacity validation and stable IDs.
4. Adapt activation, transfer, historical rosters, attendance and current advisor
   authorization. Apply dated-transfer precedence with NULL starts ordered last.
5. Frontend writer consumes generated contracts, removes retired inputs, supports
   capacity/schedule preparation, and renders exact findings and annual history.
6. Coordinator freezes a meaningful contract/migration checkpoint, then selects
   focused checks through `scripts/test_backend_school.sh`, frontend contract and
   application runners. One Cargo owner runs at a time.
7. Independent Reviewer and QA assess the complete frozen candidate. Writers repair
   findings before renewed affected review. Coordinator owns Git, PR, latest-main
   checks, integration and selected deployed acceptance.

Required cases include populated pre154 preservation, immutable history and
unresolved-execution refusal, 501-student preparation and exact replay, stale
preview/capacity rejection, hold resolution, activation, annual-to-dated transfer,
old-year advisor denial, selected bell schedule validation and frontend workflows.
