# Activity zero weekly targets

User-approved scope: an independent or synchronized learner-development activity may have an actual weekly target of zero. It remains opened; curriculum workload, groups, teachers, rosters, assessments/results and published history retain their identities. Catalog defaults remain unchanged. Zero does not waive evaluation or attendance requirements.

Delivery owns nonnegative target validation for adding, inclusion, adjustment, readiness and publication. Its existing integer API and migration090 nonnegative journal constraint remain unchanged. Timetable rejects ordinary and synchronized placements with zero demand. Published timetables keep their original source; adopting a newly published Delivery source requires an explicit draft action and explicit removal of obsolete placements before publication.

A real HTTP regression exposed that the canonical projection ignores adjustments to offerings added to the same draft. Forward migration143 changes only the added-target projection arm to prefer that revision's explicit adjustment. Applied migration142 remains intact; date selection, exclusions, stop commands and teacher projection retain their existing owners and semantics.

Function replacement alone leaves previously cached draft targets stale and causes the existing ResourceStale publication guard to block. Migration143 therefore repairs only an existing cached offering weeklyPeriodTarget when both an add and an adjustment for that offering belong to the same writable draft revision. Preserve array order and every other JSON field. Do not capture/rebuild the graph, reintroduce excluded resources, or modify journals, registry rows, published/cancelled versions or downstream data. Increment the changed target version and its parent revision row versions once, invalidating stale requests. Unaffected drafts and repeated reconciliation are no-ops.

Use normal maintenance/migration deployment and roll forward with a compatible binary; never rewrite migration checksums or delete history to roll back. Independent review and QA assess the integrated candidate.
