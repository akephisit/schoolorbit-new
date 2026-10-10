# Private File Image Loading Design

## Problem

`PrivateFileImage` renders an `<img>` before its private file blob URL is ready. On
the first paint the browser therefore shows a broken-image icon and the image alt
text. Once the download completes, the component replaces that state with the
real image.

## Scope

- Change only the shared `PrivateFileImage` presentation behavior.
- Keep the existing private file API and download flow unchanged.
- Preserve the current element classes and surrounding layout for every consumer.

## Design

The image element will be hidden from the initial server/client render. Its
attachment will keep it hidden while downloading the private file and decoding
the resulting blob URL. The component will reveal the image only after the
browser fires its successful `load` event.

While hidden, the existing background supplied by each avatar or image container
will remain visible as a neutral placeholder. If downloading or decoding fails,
the image remains hidden and the existing error logging remains in place. A new
wrapper or animated skeleton is intentionally excluded so current layouts do not
change.

Cleanup will remove event listeners, abort an in-flight request, revoke any object
URL, remove the image source, and return the image to its hidden state. This keeps
file changes and component destruction safe.

## Verification

1. Add a regression test that requires the shared image to be hidden initially
   and revealed only by a successful image load.
2. Run the focused file-platform test and confirm it fails before implementation.
3. Apply the minimal component change and confirm the focused test passes.
4. Run Svelte validation, type checks, lint, and the frontend static test suite.
5. Deploy the frontend for `snwsb`.
6. In production, delay the private download request and verify the neutral
   placeholder is shown during loading, then verify the real image appears after
   the request is released.
