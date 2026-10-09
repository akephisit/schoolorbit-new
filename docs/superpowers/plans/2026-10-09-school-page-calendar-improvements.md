# School page and calendar improvements

Implement all eight requested browser comments on the latest main, keeping each capability with its existing owner and avoiding a new crate or schema. Order homerooms by actual education level/year and natural room number; order annual students by grade, room, class number and a stable student-code tie breaker, including mutation patches. Join Thai titles directly to given names at the typed source.

Keep the existing logout/session safeguards and navigate successful logout to the school homepage. Add a compact gender/total summary grouped dynamically into kindergarten, primary, lower/upper secondary and other configured levels. Preserve the streamed statistics region and grade/room drilldown.

Render the public organization as a compact connected hierarchy with profile photos. Authorize photo delivery only through a current active staff membership in an active organization unit and the staff member's ready, owned profile-image relationship. Use existing File Platform delivery; never publish arbitrary private files, storage keys or account identifiers. Keep organization errors independent.

Reuse the existing bounded public Academic Core year/date options. Move public page/embed primary calendar reads into streamed route loaders. Keep the grid and navigation stable during month changes, cancel superseded reads, and intersect event ranges with the selected year's exact start/end dates. Add an accessible shared partial-text search dialog for public and staff calendars, with bounded cross-month results, pagination, year/date scope, loading/empty/error states and navigation to the chosen event. Public searches always enforce public visibility; staff searches retain existing read authorization.

Verify pure ordering/aggregation/search helpers, database ordering/search/privacy boundaries, generated API contracts, scoped pipeline suites, and focused Chromium workflows with sanitized fixtures on mobile/desktop in light/dark themes. Review diff/status, commit and fetch latest main, push a dedicated branch and open/attach a PR. Remove this workflow plan once the PR records the completed outcome.
