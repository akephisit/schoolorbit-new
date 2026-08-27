# Academic Work Organization and Guided Workflows

**Date:** 2026-08-27

**Status:** Approved

**Scope:** `backend-school` menu administration and academic read contracts, `frontend-school` academic navigation and workflow pages, generated API contracts, one forward-only tenant migration for recommended menu sections and route-recommendation metadata, and focused tests

## Context

SchoolOrbit now has a normalized Academic Core, Learning Delivery boundary, explicit academic-year and academic-term context, and configurable menu hierarchy. The domain model correctly distinguishes reusable catalogs and curricula from term-specific offerings, actual learning groups, and published rosters. The current staff experience does not yet communicate those boundaries clearly.

Most academic routes still declare the single generic menu group `academic`, while the database also contains an older set of broad default groups such as `academic_foundation`, `academic_timetable`, and `academic_quality`. The resulting menu does not match how Thai schools commonly divide academic work. It also places otherwise independent work in what appears to be one fixed sequence.

The curriculum and learning-delivery pages expose internal identifiers in editable text fields. Teachers must know grade-level, catalog-version, study-program, homeroom, organization-unit, room, teacher, or student UUIDs to complete ordinary academic work. The delivery page also uses the user-facing term `ชุดการเรียน` for `learning_offerings`, although an offering is the published subject or activity available in a term, while a `learning_group` is the actual group of teachers and learners.

Several dependent pages already require an academic year or term, but they do not consistently distinguish these cases:

- no academic context has been selected;
- the page is valid but contains no records yet;
- a specific create or publish action lacks prerequisite data;
- the request failed;
- the user may read the page but lacks permission for a management lookup or mutation.

A central academic-preparation dashboard was considered during design discussion and rejected. Some work, including score structures, exam schedules, and supervision setup, may legitimately be completed after a term starts. Readiness must therefore be local to the page and action that needs it.

## Research Basis and Terminology

The internal structure of Thai academic administration is not identical across schools, but the reviewed official school sources repeatedly separate or combine the same core responsibilities:

- curriculum and teaching management;
- student registration;
- measurement and evaluation;
- learner-development activities;
- supervision and instructional development;
- student admission.

The reviewed sources include the [academic administration manual listing curriculum, evaluation, registration, learner-development, information, media, and quality groups](https://www.nmt.ac.th/file/group/vichakarn/academic/01.pdf), the [Ko Samui School academic manuals](https://samuischool.ac.th/mainpage), the [Watcharawittaya academic service structure](https://academic.wr.ac.th/e-service), and the [Jaeng Witthaya academic administration structure](https://jvs.ac.th/images/PDF/jvs01.pdf).

Because schools use `ฝ่าย`, `กลุ่มงาน`, and `งาน` differently, SchoolOrbit uses the neutral prefix `งาน` for its recommended section names. A school may rename each persisted section without changing routes, permissions, or domain behavior.

## Goals

- Organize the Academic Administration workspace by recognizable Thai-school work areas.
- Keep workspace and section placement configurable and school-owned.
- Let an authorized school administrator preview and explicitly apply the recommended academic menu structure.
- Preserve route identity, permission filtering, school-customized labels, icons, active states, and later manual rearrangement.
- Make each academic page check only the context and prerequisite data needed by that page or action.
- Keep pages readable when prerequisite records are missing; block only the affected action.
- Replace editable UUID fields with searchable, human-readable selections.
- Make curriculum planning show the real relationship between curriculum versions, study programs, grade levels, and subject or activity versions.
- Rename `learning_offering` in Thai UI copy to `รายการเปิดสอน` or the concrete phrase `รายวิชาและกิจกรรมที่เปิดสอน`.
- Keep `กลุ่มเรียน` for the actual teacher/learner cohort and show human-readable teachers, rooms, homerooms, and learners.
- Avoid N+1 requests by using typed, page-specific batch workspace contracts.
- Preserve the normalized Academic Core and Learning Delivery data model without a new compatibility layer.

## Non-Goals

- Do not build a central academic readiness center or a mandatory setup wizard.
- Do not require every academic task to be completed before the term can begin.
- Do not implement Gradebook, score entry, locked results, term closure, annual closure, or promotion in this change.
- Do not build automatic learning-group generation or an automatic timetable solver.
- Do not merge homerooms, offerings, learning groups, or rosters into one table or UI concept.
- Do not duplicate one service route in multiple menu sections merely to shape navigation.
- Do not infer authorization from menu placement or a staff member's department name.
- Do not automatically overwrite an existing school's menu placement during deployment or route synchronization.
- Do not expose national IDs, contact details, or other unnecessary student data in roster lookup responses.
- Do not edit the applied academic or menu migrations.

## Approaches Considered

### Fixed nationwide academic departments

Hard-code one department tree for every school and migrate all academic menu items into it automatically. This is simple to explain, but the reviewed schools group the same work differently. It would also violate SchoolOrbit's existing rule that section placement and labels are school-owned. It was rejected.

### Central academic preparation center

Put every prerequisite on one ordered checklist and send users through it before other academic pages. This gives one visible sequence, but it implies that legitimate in-term work is late or invalid and introduces a second place that must understand every module's readiness. It was rejected.

### Configurable recommended sections with page-local guidance — selected

Provide a Thai-school-oriented recommended section template, let a menu administrator preview and apply it explicitly, and keep each service page responsible for its own prerequisites. Curriculum and delivery pages receive targeted UX and typed workspace contracts so internal IDs never become user input.

This approach aligns navigation with real responsibility while keeping schools free to combine, rename, or reorder sections. It also keeps dependency logic next to the feature that owns it.

## Architectural Principles

1. **Responsibility, not sequence:** a menu section answers who normally owns the work, not what must be completed first.
2. **Page-local dependencies:** a page or action checks only its direct inputs. There is no global readiness score.
3. **Read first:** missing management prerequisites never turn an otherwise readable page into an error.
4. **Human labels at the UI boundary:** UUIDs remain wire and persistence identifiers and are never editable academic content.
5. **One domain term per concept:** catalog, curriculum requirement, offering, learning group, and roster remain distinct.
6. **Batch reads, lazy management data:** initial pages load bounded summary data; larger management lookups load only after the exact permission and user action require them.
7. **School-owned navigation:** route synchronization updates system-owned route identity, access metadata, and recommended placement metadata but never silently resets actual placement.
8. **Explicit template application:** applying the recommended menu is an authorized school choice, transactional, previewed, and idempotent.
9. **Generated contracts:** all JSON changes originate in typed Rust DTOs and OpenAPI and are consumed through generated TypeScript types.

## Recommended Academic Navigation

The existing top-level workspace remains:

```text
กลุ่มบริหารวิชาการ (workspace code: academic)
```

The recommended sections and service placement are:

| Order | Section code | Default Thai name | Service route and label |
|---:|---|---|---|
| 10 | `academic_curriculum` | งานหลักสูตรและกลุ่มสาระ | `/staff/academic/catalog/subject-groups` — กลุ่มสาระการเรียนรู้ |
|  |  |  | `/staff/academic/catalog/subjects` — ทะเบียนรายวิชา |
|  |  |  | `/staff/academic/curricula` — หลักสูตรและแผนการเรียน |
| 20 | `academic_delivery` | งานจัดการเรียนการสอน | `/staff/academic/core` — ปีการศึกษา ภาคเรียน และเวลาเรียน |
|  |  |  | `/staff/academic/delivery` — รายวิชาและกิจกรรมที่เปิดสอน |
|  |  |  | `/staff/academic/timetable/today` — ตารางสอนวันนี้ |
|  |  |  | `/staff/academic/timetable` — จัดตารางสอน |
| 30 | `academic_registry` | งานทะเบียนนักเรียน | `/staff/students` — รายชื่อนักเรียน |
|  |  |  | `/staff/academic/homerooms` — ห้องประจำชั้น |
|  |  |  | `/staff/academic/student-years` — นักเรียนประจำปี |
| 40 | `academic_assessment` | งานวัดผลและประเมินผล | `/staff/academic/assessments` — โครงสร้างคะแนน |
|  |  |  | `/staff/academic/question-bank` — คลังข้อสอบ |
|  |  |  | `/staff/academic/exam-schedules` — ตารางสอบ |
| 50 | `academic_activities` | งานกิจกรรมพัฒนาผู้เรียน | `/staff/academic/catalog/activities` — ทะเบียนกิจกรรมพัฒนาผู้เรียน |
| 60 | `academic_supervision` | งานนิเทศและพัฒนาการสอน | `/staff/academic/supervision` — นิเทศการสอน |
| 70 | `academic_admission` | งานรับนักเรียน | `/staff/academic/admission` — รับสมัครนักเรียน |

The personal routes `/staff/timetable` and `/staff/exams` remain under `หน้าหลักของฉัน`; they are individual views, not academic-administration services.

The separate `/staff/academic/periods` menu is removed because `/staff/academic/core` already owns bell schedules and their periods. Existing deep links redirect to the bell-schedule section of `/staff/academic/core`; the system does not maintain two editable period workspaces.

Learner-development activity catalog management belongs to `งานกิจกรรมพัฒนาผู้เรียน`. Creating an activity offering, group, timetable, and roster remains in the shared Learning Delivery page. The activity catalog provides a contextual action that opens the delivery page filtered to activities; it does not add a duplicate sidebar route.

## Menu Ownership and Recommended Template

### Persisted section definitions

A new forward-only migration adds nullable `recommended_workspace_code`, `recommended_group_code`, and `recommended_display_order` fields to `menu_items`. These fields are system-owned suggestions and are separate from the actual school-owned `group_id` and `display_order`.

The same migration adds any missing recommended `menu_groups` using the codes above, `ON CONFLICT DO NOTHING`. It does not rename, delete, reorder, activate, deactivate, or move existing school-owned records.

Legacy default groups such as `academic_foundation`, `academic_students`, `academic_timetable`, and `academic_quality` may remain if a school still uses them. Empty groups are naturally omitted from the permission-filtered sidebar. The change does not delete those groups because a school may have renamed them or placed custom links inside them.

### Route metadata

Each system route declares its recommended `group`, `workspace`, and default order in `_meta.menu`. Route synchronization records those values in the three system-owned recommendation fields on every successful sync. They determine actual placement only when the route is first created; existing routes retain their persisted group and order. Synchronization also continues to preserve display label, icon, and active state.

The recommendation columns are nullable so deployment remains safe before the first post-migration route sync. Preview reports that route recommendations are not ready until a complete sync has populated them; it never guesses a target from current placement.

### Preview and apply

Menu administration adds an action named `ใช้โครงสร้างงานวิชาการแนะนำ` for users with `menu.update.all`.

Preview returns:

- recommended sections that will be created;
- system academic routes that will move;
- current and target section names;
- route ordering changes;
- custom links and non-academic routes that will remain untouched.

Apply reads the previewed system-owned recommendation fields and executes them in one transaction. It:

- creates missing recommended sections;
- moves only `frontend`-managed routes whose recommended workspace is `academic`;
- applies the recommended order inside target sections;
- preserves each route's current display label, icon, active state, path, permission, and user type;
- preserves custom and integration-owned menu items;
- does not delete old or empty sections;
- returns the applied counts and current menu revision.

The operation is idempotent. Reapplying an unchanged template produces no additional moves. A stale preview revision returns `409` and asks the administrator to preview again.

After application, every placement remains school-owned. The administrator may immediately rename sections, combine them, move routes, or reorder them. Later deployments and route synchronization preserve those choices.

## User-Facing Domain Language

| Internal domain | Thai UI term | Meaning shown to users |
|---|---|---|
| catalog subject/activity | ทะเบียนรายวิชา / ทะเบียนกิจกรรม | Stable school catalog identity with version history |
| curriculum version | รุ่นหลักสูตร | Published or draft curriculum valid for academic-year ranges |
| study program | แผนการเรียน | Program or track inside one curriculum version |
| curriculum requirement | รายการในแผนการเรียน | Subject or activity expected for a grade and optional term position |
| learning offering | รายการเปิดสอน | A subject or activity made available in one selected term |
| learning group | กลุ่มเรียน | The actual class cohort with teacher, rooms, and roster |
| roster | รายชื่อนักเรียนในกลุ่ม | Authoritative learners in one learning group |

The Thai phrase `ชุดการเรียน` is removed from page titles, labels, empty states, errors, and toasts in this workflow. Rust type and table names remain unchanged because `learning_offering` is the correct domain boundary.

## Page-Local Dependency Model

### Page behavior

The existing route metadata continues to declare whether a page requires no academic context, a year, or a term. Domain prerequisites are evaluated inside the page that owns the action.

Each page distinguishes four independent states:

1. **Missing context:** select a year or term in the topbar.
2. **Empty data:** the page is valid and has no records yet.
3. **Missing action prerequisite:** existing data remains readable, but one create, publish, or schedule action shows what is missing and where to create it.
4. **Request or permission failure:** errors and access restrictions use their existing explicit page states and are never presented as missing setup data.

The frontend uses a small typed page-local view model with a stable key, severity (`missing` or `warning`), concise Thai explanation, and an optional action label and route. It is a presentation pattern, not a central dependency registry or backend workflow engine. A missing seeded master-data record may therefore explain that a system administrator must correct the setup without linking to a nonexistent editor.

Pages use `PageState` for whole-page missing context or empty results. An action-specific missing prerequisite appears beside the affected action in a compact shadcn-svelte `Alert` or empty-state card. The signature interaction is a restrained `ทางไปต่อ` action that names the missing record and links directly to its owner.

### Dependency matrix

| Page or action | Direct dependency | Behavior when missing |
|---|---|---|
| Curriculum overview | none for reading | Show an ordinary empty curriculum state |
| Create curriculum | active grade-level masters | Keep overview readable; explain the missing school master data when creation is attempted |
| Add curriculum requirement | selected draft curriculum version, study program, grade level, and published subject/activity version | Disable only add/save requirement actions and identify the missing catalog or program data |
| Academic core | none | Own creation of years, terms, bell schedules, and periods |
| Delivery overview | selected academic term | Ask for term selection; do not fetch term data without it |
| Create offering manually | selected term plus a published subject/activity version and target data | Keep existing offerings readable; guide the create action |
| Apply offerings from curriculum | selected term plus a published curriculum version and study program requirements | Preview available items; an empty curriculum produces guidance, not an API error |
| Create learning group | an existing offering | Show the action inside the selected offering only |
| Build or publish roster | learning group plus eligible student-year and homeroom data | Show named missing sources and preserve any existing roster |
| Homerooms | selected academic year and grade-level masters for creation | Existing homerooms remain readable |
| Student-year records | selected academic year; student, grade, and study program for creation; homeroom only for placement | Filters and existing records remain usable when an optional placement source is empty |
| Timetable | selected term; learning groups, teachers, periods, and rooms for scheduling | Show separate missing-source guidance; do not treat an empty timetable as an error |
| Assessment structure | selected term and course offerings for creation | Existing plans remain readable; link to delivery when no course offering exists |
| Question bank | none for reading | Subject association is optional until the relevant create action |
| Exam schedule | selected term and eligible course offerings/groups | Existing rounds remain readable; guide round scheduling actions |
| Supervision | term remains optional for overview; teacher/group/schedule data is required only for term-scoped booking or observation actions | Keep templates and non-term views usable |
| Admission | selected academic year | Admission retains its own round-specific prerequisites |

Closing a term or year is explicitly outside this change. Future lifecycle work may aggregate module readiness for a close operation, but it must not turn the ordinary academic menu into a mandatory setup sequence.

## Curriculum Workspace Redesign

### Routes and layout

`/staff/academic/curricula` becomes a scannable curriculum overview. A row shows stable code and name, grade coverage, display version, effective academic-year range, version status, number of study programs, and draft state.

Long-lived editing moves to `/staff/academic/curricula/[id]`. The detail route shows:

- curriculum identity and version history;
- selected version status and effective academic-year range;
- study programs in that version;
- requirements grouped by grade level and recommended term;
- readable subject/activity code, name, kind, credit/hours, and requirement kind;
- create, publish, and replace actions allowed by the exact generated permission.

The overview remains independent of the topbar year because curriculum versions span academic years. The detail workspace may use selected year as a visual comparison default, never as hidden query authority.

### Human-readable inputs

- Grade coverage uses a searchable multi-select of active grade-level masters.
- Effective start and optional end use academic-year selectors with Thai display names.
- Study programs are selected by code and name.
- Requirements select `รายวิชา` or `กิจกรรม`, then a published version by code, name, version, and effective range.
- Grade level, requirement kind, and recommended term position use labeled shadcn-svelte controls.
- UUID values never appear in labels, placeholders, validation messages, exports, or editable text fields.

Published curriculum versions remain immutable. Editing published content creates a new draft version; the UI does not simulate in-place edits.

### Read contracts

Add a typed curriculum overview endpoint that returns curriculum summaries and resolved academic-year and grade-level labels in bounded queries. Extend the existing curriculum program workspace response with resolved labels for existing requirements without per-row requests.

Add a lazy, version-scoped curriculum management-options endpoint containing academic-year, grade-level, and published subject/activity-version selections. The frontend requests it only after the user has the corresponding curriculum management capability and opens a create or edit flow. Read users receive the overview and existing resolved requirement labels without action-only data requests.

## Learning Delivery Workspace Redesign

### Routes and layout

`/staff/academic/delivery` is titled `รายวิชาและกิจกรรมที่เปิดสอน`. It shows one term-scoped overview with filters for course/activity kind, status, grade level, study program, and text search.

Each offering row shows:

- subject/activity code and name snapshot;
- kind;
- target grade levels and study programs;
- draft, published, or closed status;
- learning-group count;
- teacher assignment coverage;
- roster publication coverage;
- an action to open its groups.

`/staff/academic/delivery/[offeringId]` becomes the focused offering workspace. It owns offering details, groups, teachers, homeroom coverage, preferred rooms, roster preview, and roster publication. The deep route supports reload, browser history, and direct links from assessment, timetable, activities, and exam pages.

The create flow offers two deliberate choices:

- `นำมาจากหลักสูตร` — preview and apply eligible curriculum requirements;
- `เพิ่มรายการเปิดสอนเอง` — select one published subject or activity version and explicit targets.

Neither flow creates scores, timetable entries, or copied term history.

### Human-readable inputs and rosters

- Catalog versions display code, name, version, and effective range.
- Grade levels, study programs, homerooms, organization units, teachers, and physical rooms use readable searchable selectors.
- Selected teachers show name and assignment role.
- Selected homerooms show year, grade, and room name.
- Roster preview shows student code, display name, grade level, current homeroom, proposed state, and a conflict explanation.
- Roster responses never include national ID, blind index, contact data, guardian data, or unrelated student profile fields.

### Read contracts

Add a typed, term-scoped delivery overview endpoint that returns offering summaries and resolved group coverage in bounded batch queries. Add a lazy management-options endpoint for the selected term; the frontend requests it only after the user has a management permission and opens a create or edit flow.

Expand the roster preview student DTO with the minimal display fields above. Existing offering, group, teacher, homeroom, roster preview/apply, and roster publish mutation endpoints remain authoritative.

The frontend API layer consumes generated operation query types and DTOs. No `unknown`, response cast, ad-hoc wire interface, or manual snake/camel conversion is introduced.

## Downstream Page Integration

Dependent modules continue to reference `learningOfferingId` or `learningGroupId` internally. Their user interfaces consume resolved display labels and deep-link back to the correct delivery workspace.

- Assessment structure links to the selected course offering and uses `รายการเปิดสอน` in copy.
- Timetable links missing groups, teachers, periods, or rooms to their owning pages.
- Exam schedules list eligible offerings/groups with code, name, grade, and group labels.
- Supervision shows learning-group and timetable context without exposing identifiers.
- Activity catalog opens delivery with `kind=activity` when the user chooses to open the activity for a term.

Each integration loads only its own page data. It does not preload curriculum, delivery, assessment, exam, timetable, and supervision workspaces together.

## Permissions and Authorization

- Navigation placement never grants access. `/api/menu/user` remains permission-filtered.
- Menu template preview requires `menu.read.all`; apply requires `menu.update.all`.
- Existing generated academic module permissions remain authoritative for page discovery and reads.
- Create, edit, publish, roster, timetable, assessment, exam, and supervision controls load their management data only after the exact capability passes.
- Backend resource policies continue to enforce school, organization-unit, organization-tree, assigned, or own scope as defined by each module.
- No role-named or department-named permission is added.
- A read-only page must not fail because it attempted an action-only lookup.

## Error, Empty, and Concurrency Behavior

- Missing year or term context produces a local selection prompt, not a malformed API request.
- A legitimate empty list produces an empty state with the next permitted action.
- A failed initial workspace request replaces only that workspace with a retry action.
- A failed lazy management lookup leaves already-loaded read data visible.
- A failed detail request stays scoped to its detail route or Sheet.
- Save/publish conflicts continue to use row versions, source hashes, and `409`; the UI asks the user to reload or preview again.
- A stale menu-template preview returns `409` without moving any menu item.
- Busy state disables only the mutation that could be duplicated.
- Unknown historical classification values remain readable and are never silently rewritten.

## Visual and Interaction Direction

The audience is Thai teachers and academic staff who recognize school work areas more readily than software-domain names. The visual direction is an orderly academic work register using the existing SchoolOrbit shell and semantic tokens.

The distinctive element is not decoration: section names, table columns, contextual labels, and the `ทางไปต่อ` action encode who owns a record and what directly unlocks the current action. Pages avoid dashboard-style completion percentages, global progress rings, and decorative readiness scores.

Desktop overview pages use readable tables; mobile uses stacked cards in the same information order. Forms use local shadcn-svelte Select, Popover, Command, Checkbox, DatePicker, Alert, Sheet, Dialog, and Table primitives. Existing focus, keyboard, reduced-motion, dark-mode, full-width PageShell, loading skeleton, and compact-filter conventions remain in force.

## Data Migration and Compatibility

This change does not alter the Academic Core, Learning Delivery, assessment, timetable, exam, or supervision table relationships and does not migrate curriculum or delivery records.

One new sequential tenant migration adds the nullable route-recommendation columns and missing recommended menu-section records. It does not move actual menu items. Applied migrations remain untouched.

The UI switches directly from `ชุดการเรียน` copy and UUID inputs to the new terminology and selectors. There is no parallel legacy curriculum or delivery UI. Existing APIs stay in place where still authoritative; additive overview/options contracts and the expanded roster preview are versioned through the tracked OpenAPI artifact.

Existing school menu placement is not migrated automatically. The current school applies the recommended layout through the reviewed menu-template action after deployment. This explicit action is the data-changing cutover for navigation placement.

## Release Boundaries

Implementation is split into serial, deployable releases. Each release receives its own implementation plan and verification checkpoint.

### Release 1 — Academic navigation and local guidance foundation

- add recommended menu sections through a forward migration;
- update route metadata defaults;
- implement menu-template preview/apply;
- consolidate period editing under Academic Core;
- introduce the page-local prerequisite presentation pattern;
- apply terminology changes that do not depend on new academic read contracts.

### Release 2 — Curriculum workspace

- add curriculum overview and resolved program-workspace contracts;
- add curriculum detail route;
- replace curriculum UUID fields with human-readable controls;
- add curriculum-specific dependency, permission, empty, and error states.

### Release 3 — Learning Delivery workspace

- add delivery overview and lazy management-option contracts;
- add offering detail route;
- replace delivery UUID fields with human-readable controls;
- expand roster preview with minimal learner display information;
- apply contextual links to activities, timetable, assessment, exams, and supervision.

Release 3 completes this design. Gradebook and lifecycle work remain separate backlog items and require their own approved design and plans.

## Testing and Verification

Implementation follows test-driven development and runs commands serially.

Backend coverage proves:

- recommended section creation is forward-only and idempotent;
- route synchronization updates recommendation metadata while preserving actual placement and order;
- menu preview/apply requires the correct existing generated permissions;
- apply moves only mapped system academic routes and preserves labels, icons, active state, custom links, and integration links;
- stale preview or any failed move rolls back the whole template transaction;
- curriculum and delivery workspaces respect resource scope and use bounded batch queries;
- read users do not need management-option access;
- roster display data is minimal and excludes sensitive profile fields;
- OpenAPI registers every added or changed response.

Frontend coverage proves:

- route metadata maps to the recommended sections without duplicating routes;
- personal timetable and exam routes remain in the personal workspace;
- the period route no longer creates a second editable period workspace;
- each audited academic page distinguishes missing context, empty data, missing action prerequisite, permission denial, and request failure;
- curriculum and delivery forms never render an editable UUID field;
- offering and group terminology is consistent;
- read-only pages never request action-only management options;
- overview pages and details do not fan out per row;
- roster preview renders names and conflict states without sensitive fields;
- responsive, keyboard, focus, and deep-link behavior works for the new overview/detail flows.

The applicable `.rules` verification matrix includes:

- focused Rust service, policy, handler, and migration tests;
- `cargo fmt --all -- --check`, `cargo test --test static_architecture`, and `cargo check` for backend-school;
- API contract generation, checks, and contract tests;
- Svelte analyzer/autofixer for every edited Svelte component;
- frontend lint, Svelte check, and static tests;
- focused browser coverage for menu-template preview/apply and curriculum/delivery workflows when the test environment is available;
- `git diff --check`, final diff review, and `git status --short` after every release.

## Deployment and Rollback

Each release deploys through the normal `main` workflow after its own checks pass.

- Additive backend contracts and migrations deploy before the frontend that consumes them.
- Route synchronization runs only after the frontend deployment and does not alter existing placement.
- The recommended menu template is previewed and explicitly applied after backend and frontend readiness.
- If a frontend rollback is required, additive endpoints and section records remain harmless.
- Menu placement can be manually rearranged through menu administration; applying the template never deletes custom groups or links.
- No production database restore is required to roll back terminology or UI routing because academic records are unchanged.

## Success Criteria

- Academic services appear under recognizable work sections after an administrator explicitly applies the recommended structure.
- A later route synchronization preserves the applied or subsequently customized layout.
- Teachers can enter curriculum and delivery workflows without knowing or copying a UUID.
- Users can explain that a curriculum requirement becomes a term offering, and that an offering contains one or more actual learning groups and rosters.
- Every affected page remains readable when an unrelated academic area has not been prepared.
- Missing data blocks only the action that directly depends on it and provides one clear route forward.
- Curriculum and delivery overview loading is bounded and does not issue one request per row.
- Existing academic data and normalized domain relationships remain intact.
- The release does not claim Gradebook, term closure, annual closure, or promotion readiness.
