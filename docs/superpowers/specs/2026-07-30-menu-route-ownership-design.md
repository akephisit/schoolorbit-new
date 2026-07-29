# Menu Route Ownership Design

## Goal

Make deployment route synchronization authoritative only for frontend-owned route identity while preserving every school-owned menu label, placement, icon, active state, ordering, and custom menu record.

## Ownership model

Add `menu_items.managed_by` with the allowed values `frontend`, `school`, and `integration`. New menu items created through school menu administration default to `school`. A successful route synchronization claims only the route codes present in that desired-state payload as `frontend`.

Existing rows initially default to `school` instead of being bulk-classified. This avoids guessing that an existing custom record belongs to the frontend. The first successful synchronization safely claims active frontend routes by their system-owned route code.

The alternatives rejected are:

- Marking every existing row as `frontend`, because it could misclassify and later delete school-created records.
- Adding a separate ownership table, because ownership is a single invariant of each menu item and does not need another lifecycle.

## Synchronization transaction

The backend validates that the desired route list is non-empty and contains unique route codes. It then performs group/workspace creation, route upserts, and stale-route cleanup in one database transaction.

Frontend synchronization owns and updates only route code, path, user type, required permission, and `managed_by`. It preserves the school-owned name, icon, group, display order, and active state on conflict. Cleanup deletes only rows where `managed_by = 'frontend'` and the code is absent from the complete desired-state list.

Any validation or database error rolls back all inserts, updates, default groups, and cleanup. Per-route errors are returned instead of logged and ignored.

## Explicit deployment step

Route scanning and registration move out of the Vite build hook into `npm run sync:menu-routes`. Production deployment workflows run it after a successful Cloudflare deployment. A missing environment value, empty scan, malformed menu metadata, duplicate route code, HTTP failure, or invalid response fails the workflow visibly.

The deployment key is supplied only to this explicit command. It is removed from Vite-prefixed build variables and Cloudflare Worker runtime variables.

## Testing

Backend database tests prove:

- school-owned custom items survive synchronization;
- stale frontend-owned items are removed;
- school-customized labels, placement, icon, order, and active state survive a frontend upsert;
- a later route failure rolls back earlier writes and cleanup;
- empty or duplicate desired-state payloads are rejected without mutation.

Frontend runtime tests prove malformed metadata and incomplete scans fail, and the explicit command sends the complete route payload while surfacing backend failure. Workflow/static checks ensure menu synchronization is a separate post-deployment step and is no longer a Vite build side effect.

## Rollout

Migration `033` is forward-only and safe before the new backend runs. Existing menu items remain school-owned until a complete successful route synchronization claims current frontend routes. Deploy backend-school and apply the migration before tenant frontend workflows begin using the explicit synchronization command.
