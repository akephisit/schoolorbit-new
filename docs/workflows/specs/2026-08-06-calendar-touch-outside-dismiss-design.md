# Calendar Touch Outside Dismiss Design

## Problem

The public calendar opens `CalendarDayTimelineDialog` when a visitor selects a day on a mobile-width page, including `/calendar/embed` inside WordPress. Tapping the dimmed area outside the dialog does not close it on touch devices, although clicking the same area with a mouse and using the close button both work.

The issue is reproducible on the deployed embed page with an iPhone browser profile. The installed Bits UI dismissible layer handles mouse interactions immediately, but its touch path waits to register a `click` listener after the outside `pointerdown`. Modern touch browsers can dispatch the synthesized click before that delayed listener is registered, so the close transition is missed.

## Outcome

The mobile day-detail dialog closes when the visitor taps its dimmed overlay. Taps within the dialog continue to interact with its content and do not dismiss it. The existing close button and Escape-key behavior remain unchanged.

## Considered Approaches

### Calendar-specific overlay click handler — selected

Allow the shared `Dialog.Content` wrapper to accept overlay properties, then give only `CalendarDayTimelineDialog` an overlay `onclick` handler that sets its bound `open` state to `false`.

This uses the overlay itself as the outside-interaction boundary, avoids touch event timing in the library dismissible layer, and limits the behavior change to the affected calendar dialog.

### Upgrade Bits UI

Upgrading from the installed 2.14 series would change a shared interaction dependency across the application. The latest inspected implementation still uses the same delayed touch-click path, so an upgrade would add broad regression risk without demonstrating that it fixes this issue.

### Add overlay dismissal to every shared dialog

Handling overlay clicks globally would make all dialogs bypass the affected touch path. It would also change interaction behavior throughout the application, including dialogs that may intentionally customize outside interactions, which exceeds this bug's scope.

## Component Changes

`src/lib/components/ui/dialog/dialog-content.svelte` gains an optional, typed `overlayProps` property. The wrapper forwards those properties to its existing `Dialog.Overlay`. Existing callers that omit the property retain their current behavior.

`src/lib/components/calendar/CalendarDayTimelineDialog.svelte` passes an overlay click callback that closes its bindable `open` state. No calendar data, iframe code, public API, route policy, or WordPress snippet changes are required.

The overlay and content remain siblings in the existing portal. Consequently, a click within `Dialog.Content` does not bubble through the overlay, while a click on any visible dimmed area invokes the calendar-specific close callback.

## Testing

A focused Playwright regression test will:

1. emulate a touch-capable iPhone viewport;
2. intercept the public-calendar API with a valid empty response;
3. open `/calendar/embed` and tap a calendar day;
4. assert that the day dialog is visible;
5. tap the dimmed overlay;
6. assert that the dialog closes.

The test must fail against the current implementation before the fix and pass afterward. The affected Svelte files will also be checked with the Svelte autofixer, followed by the frontend verification matrix from `.rules`: lint, Svelte check, static tests, `git diff --check`, diff review, and clean status inspection.

## Scope Boundaries

This is a frontend interaction fix. It does not change backend behavior, database migrations, permissions, generated API contracts, authentication, CSP, iframe sandboxing, calendar visibility rules, or dialog behavior outside the mobile calendar day-detail component.
