# Timetable Query Plan Evidence — 2026-07-23

## Decision

**Decision: no migration.**

The candidate partial composite index improved median execution time by **13.10%**, below the
approved 20% threshold. It also did not remove the bitmap heap scan or explicit sort, and reduced
total buffer blocks by only **0.29%**. No
`backend-school/migrations/029_timetable_active_semester_slot.sql` was created.

## Candidate

```sql
CREATE INDEX idx_timetable_active_semester_slot
ON academic_timetable_entries (
    academic_semester_id,
    day_of_week,
    period_id
)
WHERE is_active = true;
```

The reproducible benchmark is
[`scripts/analyze-timetable-index.sql`](../../scripts/analyze-timetable-index.sql). It runs inside
a transaction and rolls back all fixture data and indexes.

## Environment and fixture

- PostgreSQL: 18.4
- Source database: `TEST_DATABASE_URL`, using its direct endpoint
- Runs: 5 independent `psql` sessions
- Fixture: 100,000 deterministic timetable-shaped rows
- Semesters: 5
- Active rows: 80,000 (80%)
- Target semester: `00000000-0000-0000-0000-000000000003`
- Returned active target rows: 16,000
- Baseline indexes:
  - `(academic_semester_id)`
  - `(day_of_week, period_id)`
- Each access path was warmed once before its measured plan.

The measured query was:

```sql
SELECT id, academic_semester_id, day_of_week, period_id
FROM timetable_plan_fixture
WHERE academic_semester_id = '00000000-0000-0000-0000-000000000003'
  AND is_active = true
ORDER BY day_of_week, period_id;
```

## Results

| Run | Before (ms) | After (ms) | Improvement |
| ---: | ----------: | ---------: | ----------: |
| 1 | 11.271 | 9.987 | 11.39% |
| 2 | 12.071 | 10.320 | 14.51% |
| 3 | 11.671 | 10.564 | 9.49% |
| 4 | 12.198 | 10.331 | 15.31% |
| 5 | 11.889 | 10.521 | 11.51% |
| **Median** | **11.889** | **10.331** | **13.10%** |

### Plans and buffers

| Metric | Before | After |
| --- | ---: | ---: |
| Root plan | Sort | Sort |
| Scan path | Bitmap Heap Scan + Bitmap Index Scan | Bitmap Heap Scan + Bitmap Index Scan |
| Index used | `timetable_plan_fixture_semester_idx` | `timetable_plan_fixture_active_semester_slot_idx` |
| Shared hit/read blocks | 0 / 0 | 0 / 0 |
| Local hit/read blocks | 190 / 860 | 891 / 156 |
| Total local blocks | 1,050 | 1,047 |
| Total fixture index bytes | 5,906,432 | 6,471,680 |
| Candidate index bytes | 0 | 565,248 |

The fixture is a PostgreSQL temporary table, so its relation buffers are reported as local rather
than shared buffers. Shared hit/read blocks are therefore zero in both plans. Comparing the total
local blocks provides the equivalent I/O-work check for this benchmark: the candidate saved only
3 blocks while adding 565,248 bytes of index storage.

## Interpretation

The candidate is selected by the planner and provides a modest timing improvement, but PostgreSQL
still performs the same bitmap heap scan and explicit sort. It therefore fails both parts of the
approved evidence gate:

1. it does not remove the avoidable bitmap/sort work; and
2. it improves median execution time by 13.10%, not at least 20%, while total buffer work is
   effectively unchanged.

Keeping the existing indexes avoids extra write amplification and index maintenance. Revisit this
decision only with production-scale query plans or a materially different data distribution.

## Reproduction

```bash
for run_number in 1 2 3 4 5; do
  psql "$TEST_DATABASE_URL" -v ON_ERROR_STOP=1 \
    -f scripts/analyze-timetable-index.sql \
    > "/tmp/schoolorbit-timetable-plan-${run_number}.txt"
done
```

The five temporary result files were removed after the aggregate above was recorded.
