# Staff bilingual identity, qualifications and profile UI

Approved in chat on 2026-10-15. Base: 217a8b5b. Implementation stays inline.

## Outcomes and ownership

Staff owns the name-editing and education workflows. Existing user identity storage remains the owner of person names; Staff adds optional English title/given/family fields alongside the existing Thai fields. A shared read projection selects one complete language together, Thai first, then English, then preserves the existing partial Thai label for incomplete historical records. It exposes the selected name parts, language and fully formatted name. Read consumers use the projection; writes and the staff editor read the raw persisted language fields. Thai prefixes attach to the given name; English prefixes use spaces. No script-based relabeling, guessed prefixes, translated names or duplicated preferred-name persistence.

New staff and name-changing edits require at least one complete title/given/family set. Unrelated updates to historical incomplete names remain valid. Search includes both raw languages; directory ordering uses the selected given name, family name and identity before pagination. Staff list, profile, public staff profile, organization labels, teacher views and authenticated identity use the same resolved name. Student/parent editing workflows and historical issued document snapshots are outside the redesign.

Education becomes person-owned rows with stable UUID identities, nullable level for preserved incomplete historical records, optional major/institution, and an education-collection revision. New rows require a level. The existing three fields migrate as one row whenever any field exists, without losing partial text. Known levels display ascending, equivalent levels share a tier, and unknown/other entries are separated. Highest recognized level is a derived read summary for overview/filter equivalence, not another writable source.

Creation supplies education rows. Updates replace only the education collection with an expected revision and scoped existing IDs, inside the parent staff transaction/lock. Omission preserves it, an explicit empty collection clears it, and stale revisions return conflict while preserving the UI draft. Career history and its corrections keep their existing owner and concurrency protocol.

## Migration and recovery

Use new sequential migrations after 112, never modify applied files. Expand and backfill under locks; record bounded reconciliation counts/fingerprints for original users, education source values and unrelated staff/career data. Cleanup recomputes current evidence before dropping old education columns and refuses missing/stale/failed reconciliation. New runtime reads only canonical education rows. Register completion checks with the existing deployment audit owner. After contraction old education-writing binaries are prohibited; recovery is a reviewed roll-forward under universal maintenance. Names are additive; never copy or infer values into English fields.

## UI and state

Shared typed name fields and education-row fields serve create/edit, with field-local errors and live preferred-name preview. Keep the existing lazy permission-owned role/organization operations, draft ownership and review-before-create flow. Compact header/step controls, semantic sections, paired desktop fields and one-column mobile layout replace oversized identity/step surfaces. Edit actions retain drafts on failure/conflict and do not block unrelated regions.

Profile uses a compact horizontal identity summary and shared Card/dl sections: raw language names, personal/contact data, school employment/current career and license data, all qualifications, roles, affiliations, teaching/advisory work, career history and achievements. Show empty values explicitly and respect existing PII and action permissions. Independent regions retain loader streaming, retry, cancellation and stale-result guards. Preserve list return context and portraits through the existing authorized file owner.

## Verification and delivery

Use scoped migration preservation/refusal tests, name selection/validation/search/order cases, education collection/conflict/access tests and affected identity/teacher consumers. Compile affected producers/consumers in runtime and api-docs modes; regenerate/check API contracts. Run owning frontend check, changed-file format/lint and focused helper/browser cases including synthetic mobile/desktop light/dark, partial legacy data, English-only names and multiple degrees. Do not run unrelated database suites. Review the integrated diff and tested tree, open/attach a ready PR and follow the normal integration/deployment/acceptance owner. Remove these temporary workflow documents when the final PR records the implementation.
