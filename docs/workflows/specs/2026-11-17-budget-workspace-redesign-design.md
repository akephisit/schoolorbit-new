# Budget workspace redesign

Senior approved this design on 2026-11-17 within the user's request to redesign `/staff/budget` and every subpage for clarity and usability. No additional approval checkpoint is required.

## Scope and owners

Redesign the nine existing budget screens and their forms using the existing PageShell, app-state and shadcn-svelte owners. Keep global navigation, theme, primitives, API contracts, permissions, database and financial rules unchanged.

The frontend workspace writer owns navigation, overview/reports, three list screens and loaders, annual settings, presentation helpers and existing budget browser fixtures. The detail/form writer owns the three details and related forms/editors. Senior owns Git, generators, broad checks and integration. Independent Reviewer and QA assess the frozen candidate.

## Interaction and presentation

- Add compact permission-aware budget navigation with actual route links, active state, horizontal mobile scrolling and explicit fiscal-year context. Settings remains available to managers and payment operators.
- Make overview metrics explain their actual scope and provide useful task destinations. Keep reports focused on comparison, metric selection and export.
- Give all three lists consistent year/status filters. Only projects expose search because requests and adjustments do not implement search in the backend. Keep URL-owned filters, server totals, paging and return context.
- Stream existing read-authorized catalogs independently into list filters. A catalog failure must not block the list or request action-only settings.
- Present details in the order summary/status, contextual next action, grouped related regions. Preserve independent errors/retries and lazy history and action reads.
- Order settings around year/catalog setup, authorized funding, unit allocations and routing. Clearly distinguish actual receipts from authorized funding, saved state from drafts, and unknown money from explicit zero.
- Use semantic theme colors, restrained tinted context surfaces, neutral financial tables, readable mobile content and existing keyboard/focus behavior.

## Invariants

Preserve annual funding/quota meaning, exact integer-satang calculations, year-specific drafts, date and closed-year locks, scoped draft deletion, nested activity dates, mutation intent, late-result fencing and Back/Forward behavior. No summary may imply page-only rows represent a complete scope. No primary read moves into a component lifecycle.

`BudgetNavigation.svelte` accepts `fiscalYearId?: string`, performs no reads and marks the active route branch with `aria-current="page"`. Request details have no fiscal-year field; preserve already-known URL/return context without adding a project read.

## Acceptance

Verify all nine screens, forms, permission states, loading/errors/empty/pending states, mobile/desktop light/dark geometry and keyboard/touch interactions. Retain existing financial and async lifecycle tests. Replace the previous no-local-navigation assertion with active/year/access navigation coverage because the approved design intentionally adds local navigation.
