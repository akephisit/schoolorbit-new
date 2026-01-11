# Frontend School - Project Structure & Routing

## Directory Structure

*   `src/routes/`: SvelteKit File-system based routing. Each folder represents a URL path.
*   `src/lib/`:
    *   `components/`: Reusable UI components.
    *   `stores/`: Global state management.
    *   `utils/`: Helper functions.
    *   `types/`: TypeScript type definitions.
    *   `server/`: Server-only code (not leaked to client).

## Routing Guide

### Generic Layout
The app uses a layout hierarchy:
*   `src/routes/+layout.svelte`: Root layout (Global providers, standard imports).
*   `src/routes/(app)/+layout.svelte`: Main application layout (Sidebar, Navbar) for authenticated users.
*   `src/routes/(auth)/+layout.svelte`: Layout for login/register pages.

### Creating a New Page
1.  Create a folder in `src/routes/` matching the URL, e.g., `src/routes/staff/new`.
2.  Add `+page.svelte` for the UI.
3.  Add `+page.ts` (or `+page.server.ts`) for data loading.

### Data Loading (Load Functions)
*   **Prefer `+page.ts` / `+page.server.ts`** over `onMount` for initial data fetching.
*   This ensures data is available before the component renders, preventing layout shift.

```typescript
// Example +page.ts
export const load = async ({ fetch }) => {
    const res = await fetch('/api/data');
    const data = await res.json();
    return { data };
}
```
