#!/bin/sh
# Sourced by the coordinated release; the same filter is exercised with native Podman jq.
schoolorbit_personnel_cutover_filter() {
    cat <<'JQ'
(.personnelCutover as $p
 | $p.migrationVersion == 84
 and $p.status == "cutoverCompleted"
 and $p.passed == true
 and ($p.checks | type) == "array"
 and ($p.checks | map(.code) | sort) == ([
   "PERSONNEL_SIMPLIFICATION_STAFF_PRESERVED",
   "PERSONNEL_SIMPLIFICATION_POSITIONS_PRESERVED",
   "PERSONNEL_SIMPLIFICATION_EDUCATION_TEXT_PRESERVED",
   "PERSONNEL_SIMPLIFICATION_UNRELATED_FIELDS_PRESERVED",
   "PERSONNEL_SIMPLIFICATION_CANONICAL_SCHEMA_VALID",
   "PERSONNEL_SIMPLIFICATION_RETIRED_OWNERS_REMOVED",
   "PERSONNEL_MIGRATION_HISTORY_VALID"
 ] | sort)
 and all($p.checks[]; .passed == true and (.count | type) == "number" and .count >= 0 and (.count | floor) == .count))
JQ
}
