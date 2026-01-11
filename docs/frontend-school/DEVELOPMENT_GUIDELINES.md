# Frontend School - Development Guidelines

## UI Components & Design System
We prioritze a premium, "Wow" aesthetic using **Shadcn Svelte** and **TailwindCSS**.

### Using Components
*   **Import:** Import components from `$lib/components/ui/...`.
*   **Do not** create raw HTML elements (like `<input>`, `<button>`) if a Shadcn component exists.
*   **Icons:** Use `lucide-svelte` for icons.

### Styling Rules
*   Use standard Tailwind utility classes.
*   Avoid arbitrary values (e.g., `w-[123px]`) unless absolutely necessary.
*   Maintain consistency with the color palette defined in `app.css` / `tailwind.config.ts`.

## State Management
*   **Page State:** Use Svelte 5 "Runes" (`$state`, `$derived`, `$effect`) for local component state.
*   **Global State:** Use Runes in a shared `.svelte.ts` file or stores in `src/lib/stores` for data shared across multiple pages (e.g., User Profile, Notifications).

## Best Practices
1.  **Type Safety:** Always define Interfaces for props and API responses. Avoid `any`.
2.  **No Direct API Calls in Components:** Ideally, abstraction layers (helper functions in `src/lib/api`) should handle raw `fetch` calls, or they should happen in `load` functions.
3.  **Clean Code:** Keep `+page.svelte` focused on View logic. Move complex business logic to separate `.ts` files.
