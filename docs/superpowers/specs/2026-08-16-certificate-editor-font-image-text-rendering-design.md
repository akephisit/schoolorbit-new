# Certificate Editor Font, Image, and Thai Text Rendering Design

## Goal

Improve the certificate template editor in four related areas:

- Thai vowels and tone marks must not be clipped in the editor, exact PDF preview, or exported PDF.
- Uploaded images preserve their aspect ratio by default, can be unlocked, and can be reset to the source image ratio.
- Administrators can upload multiple static font files as one reviewed batch. The system groups real font variants by family, weight, and normal or italic style.
- Exact preview opens immediately with a visible loading state while fonts and other render assets are prepared.

The change extends the existing certificate editor and browser renderer. It does not turn the editor into a general-purpose design application.

## Chosen Approach

Model every static font file as one certificate template asset and derive its family, weight, and style from the font's internal metadata. The UI groups those assets into font families without adding a separate font-family table. Text elements select an exact asset variant, and the renderer never synthesizes a missing bold or italic face.

Font uploads use the existing private File Platform for each temporary file, followed by a certificate-specific inspection step and one atomic batch attach. This preserves the current file ownership, malware scanning, lifecycle, and cleanup boundaries while allowing the user to review the detected variants before they become template assets.

Image aspect locking is stored on each image element. Source image dimensions continue to come from File Platform inspection metadata rather than duplicated database columns. Thai text is fixed at the renderer's measurement and baseline layer so the same correction applies to exact preview and export.

## Non-goals

- Variable fonts and arbitrary variation axes are not supported in this change. A variable font is rejected with guidance to upload static font files.
- The browser does not synthesize bold or italic faces.
- Font editing, conversion, licensing, and font-family renaming are not provided.
- Image cropping, masks, and focal-point editing are not added.
- Certificate PDFs remain rendered on demand and are not stored as one completed file per recipient.

## Font Variant Model

### Inspection metadata

Extend the File Platform font inspection result with:

- detected typographic family name, falling back to the legacy family name;
- normalized weight from 100 through 900 in increments of 100;
- style `normal` or `italic`, with a static oblique face normalized to `italic`;
- whether the file contains variable-font axes.

The server reads these values with the existing trusted font parser. A malformed font is rejected by File Platform inspection. A readable file with a missing family, unsupported weight, or variable axes remains available long enough for the certificate inspection response to explain why it cannot be attached. A mismatched file purpose is rejected. The client does not infer metadata from filenames.

### Certificate assets

Add `font_style` to `certificate_template_assets` through a new migration. Existing font rows are backfilled to `normal`; image rows retain null font fields. Update the kind-fields check so font rows require family, weight, style, and rights confirmation while image rows require all font fields to be null.

A partial unique index on trimmed lowercase family, weight, and style prevents two font assets on one template from using the same normalized variant. Batch attach also checks duplicates before writing so it can return a useful row-level explanation instead of exposing a raw database error.

`CertificateTemplateAsset` gains:

- `fontStyle` for font assets;
- `imageWidthPixels` and `imageHeightPixels` for image assets.

The image dimensions are read by joining the asset's file inspection metadata; they are not copied into `certificate_template_assets`.

### Text layout

Add `fontStyle` to text elements with a backward-compatible default of `normal`. An uploaded text face continues to store its exact `assetId`, `fontFamily`, and `fontWeight`, and now also stores `fontStyle`. Built-in font manifests and uploaded-font grants include style as well.

Backend layout validation requires an uploaded font reference to match the asset's family, weight, and style exactly. Built-in font validation requires an exact built-in manifest entry. A missing variant is an actionable validation or render error; it never silently falls back to another file.

The built-in Sarabun catalog remains Regular 400 and Bold 700 in normal style. Italic controls are unavailable for this family until real italic assets are added.

## Multi-file Font Upload Flow

The font input accepts up to 40 `.ttf` and `.otf` files per reviewed selection. The client uploads them one at a time through the existing `certificate_template_font` purpose so every row has deterministic progress and retry state. It retains each temporary file ID in durable component state until it is attached or explicitly cleaned up. Both inspection and batch-attach endpoints enforce the same 40-file limit.

After uploads complete, the client calls a certificate-specific inspection endpoint with the selected file IDs. The endpoint authorizes template update access, verifies every file is ready, privately owned for that template and purpose, and returns safe font metadata plus an attachability result for each row. It does not promote or attach files.

The review table groups rows by detected family and shows original filename, weight, style, and status. It identifies:

- duplicates within the selected files;
- variants already attached to the template;
- variable fonts;
- unreadable or incomplete metadata;
- files that are no longer ready or no longer belong to the template.

Multiple families may be reviewed in one selection. The user removes or cleans failed rows, confirms font usage rights once for the selected attachable rows, and submits the batch.

The batch attach endpoint re-authorizes and revalidates all rows while holding the existing campaign-owner and template locks. It inserts all selected variants, records the rights confirmation, promotes all corresponding files, updates the template timestamp, and records an audit entry in one database transaction. Any validation or database failure attaches none of the files. Temporary uploads remain visible for retry or cleanup after a failure.

The existing single-asset endpoint remains available for image assets. A single font attach, where still needed by an internal caller, uses the same metadata-derived service as a one-item font batch and never accepts a user-supplied weight or style.

## Font Controls in the Editor

The selected text panel exposes:

- a family selector;
- a weight selector containing only weights backed by real files for the current style;
- a Bold shortcut;
- an Italic toggle.

Changing family selects normal 400 when available, otherwise the available normal weight closest to 400, otherwise the first deterministic variant sorted by style and weight. The Bold shortcut selects exact weight 700 for the current style. Pressing it again returns to the available normal weight closest to 400. Italic switches between `normal` and `italic` at the current weight. A shortcut is disabled when its exact target variant does not exist and provides a tooltip explaining which file is missing.

Changing any font control updates all four text font fields together so the element can never temporarily refer to one asset while claiming another variant.

## Image Aspect Ratio Behavior

Add `lockAspectRatio` and `aspectRatio` to image layout elements:

- New images use the inspected source width divided by source height, are fitted within the editor's initial image bounds, and start with `lockAspectRatio=true`.
- Existing layouts that lack these fields are normalized before validation and before returning the detail response: locking is enabled and `aspectRatio` is derived from the current frame. This preserves the current visual result instead of reshaping an existing image on first load.
- When locked, every resize handle preserves the stored ratio and keeps the opposite handle or edge anchored, including rotated images.
- When unlocked, width and height resize independently using the existing minimum-frame rules.
- Enabling the lock after free resizing restores the source image ratio while retaining current width.
- “Reset original ratio” also retains current width, computes height from inspected source dimensions, updates the stored ratio, constrains the result to the page, and leaves locking enabled.

Backend layout validation requires a finite positive stored ratio and, when locking is enabled, verifies the frame ratio against it within a small numeric tolerance. The source ratio is checked by the editor action, while the stored ratio allows legacy layouts to preserve their current appearance safely.

Page-geometry scaling continues to scale image frames uniformly, so a locked image remains valid after background replacement.

## Thai Text Measurement and Rendering

The current renderer sets a top text baseline at the frame boundary and clips to the exact frame. Some Thai glyph ink, including stacked vowels and tone marks, extends above that assumed top and is therefore clipped. Export uses the same overlay renderer, so the defect is present in the final PDF as well as preview.

The renderer will load the exact `FontFace` before measuring or drawing. Font registration and cache keys include family asset, file, weight, and style. Canvas font declarations and `FontFace` descriptors include both weight and style.

For each candidate font size, the text layout routine will:

1. wrap text using the existing Thai word and grapheme segmentation;
2. use an alphabetic baseline and measure each rendered line's actual ink ascent, descent, left overhang, and right overhang;
3. compute the first baseline from the maximum required ascent rather than drawing at y=0;
4. compute the final required height from ink ascent, baseline spacing, ink descent, a small anti-aliasing safety inset, and shadow extents;
5. compute fit width from actual ink bounds and shadow extents rather than advance width alone;
6. feed those exact dimensions into auto-shrink;
7. draw only after the measured ink and shadow fit the element frame.

Browsers that lack one of the optional detailed text metrics use conservative font-size-based ascent and descent fallbacks. They must reserve additional space rather than risk clipping. The interactive editor text layer receives the same vertical safety treatment and must not hide glyph ink at the top of its frame.

This keeps wrapping and frame clipping intentional while ensuring marks that belong to a fitted glyph are inside the clip. It also covers italic side overhang and text shadows instead of fixing Thai marks with an arbitrary one-off top margin.

Regression samples include stacked Thai text such as `ปั้น น้ำ ผู้เข้าร่วม กิจกรรม` and the user-visible certificate lines from the reported case. Pixel assertions verify non-background ink is present below the safety inset and absent on the clipped canvas boundary. Preview and exported PDF rasterizations remain pixel-equivalent within the existing tolerance.

## Preview Loading and Errors

Clicking an exact-preview action opens the dialog immediately. Until the operation completes, the dialog shows a spinner, `aria-busy=true`, and the status text “กำลังโหลดฟอนต์และสร้างพรีวิว…” over the preview area. Preview and download actions are disabled while the current operation is pending.

The loading state covers manifest refresh, private grant refresh, background retrieval, font loading, image loading, overlay rendering, PDF construction, and PDF rasterization. It does not disappear merely because the manifest request completed.

Closing the dialog aborts the active preview. A newer preview aborts and supersedes an older one. Aborted work must not poison the shared loaded-font cache. A real failure replaces the spinner with an actionable message and Retry and Close controls. The renderer never returns a partial PDF with a missing font, image, or text layer.

## Authorization, Privacy, and Audit

The new inspection and batch-attach operations use the existing exact template update permission and certificate resource policy. They verify current campaign ownership again under lock, following the existing template mutation pattern.

Font and image files remain private File Platform objects. Inspection responses expose only safe technical metadata needed for the editor and never return object keys, raw font tables, signed URLs, credentials, or user data. Render manifests continue to use short-lived authorized grants.

Batch attachment records the actor, template, attached asset IDs, file IDs, and changed domain fields without logging file bytes, request bodies, recipient data, or grant URLs.

No permission definition changes are required. API DTO and route changes require OpenAPI regeneration and generated TypeScript consumption.

## Migration and Compatibility

Create the next sequential tenant migration; do not edit migrations `035`, `036`, or any other applied migration. The migration:

1. adds nullable `font_style`;
2. backfills existing font rows to `normal`;
3. replaces the existing kind-fields check with a style-aware check;
4. adds the allowed-style check;
5. adds the partial unique variant index.

Before adding the unique index, the migration performs a deterministic duplicate precondition check and raises a descriptive exception if conflicting historical variants exist. It does not delete or merge user files automatically.

Serde defaults keep old text and image JSON readable. The layout normalization layer derives a missing legacy image ratio from its frame, and the editor writes explicit new fields on the next save. Rendering behavior for an existing image does not change until the user resizes it or resets its ratio. Existing text defaults to normal style and continues to match the backfilled font asset.

## Testing Strategy

Implementation follows test-driven development with commands run sequentially.

Backend focused coverage:

- font inspector detects regular, bold, italic, and variable-font metadata;
- migration and schema constraints accept valid image/font rows and reject invalid or duplicate variants;
- inspection rejects wrong-purpose, unavailable, cross-template, and unauthorized files;
- batch attach is atomic, revalidates under lock, records rights confirmation, promotes all files, and reports duplicates safely;
- layout validation matches exact family, weight, and style;
- locked image ratios and backward-compatible layout defaults validate correctly;
- render manifests contain exact style-aware built-in and uploaded font grants.

Frontend pure and static coverage:

- font families group and sort deterministically;
- family, weight, Bold, and Italic controls resolve only real variants;
- image creation uses intrinsic ratio;
- locked resize preserves ratio and the opposite anchor for all handle directions and rotations;
- unlocked resize remains independent;
- lock and reset actions restore source ratio and remain page constrained;
- contract and UI guards cover the new batch workflow and explicit cleanup state.

Browser coverage with one Playwright worker:

- multi-file upload review, row errors, rights confirmation, atomic success, retry, and cleanup;
- preview dialog spinner remains visible until a deliberately delayed font finishes loading;
- preview failure exposes Retry and a retry succeeds;
- Thai stacked marks and shadows do not touch the clipping boundary;
- exact preview and exported PDF stay pixel-equivalent;
- existing font-abort and signed-grant cache races remain covered.

After focused checks, run the applicable `.rules` verification matrix one command at a time: API contract generation/check/tests, backend formatting/static architecture/check and focused database tests, frontend lint/check/static tests, `git diff --check`, final diff review, and `git status --short`. This change alters the render manifest and issued-PDF path, so the documented live certificate lifecycle is required once after lower-level checks pass. It uses the dedicated accounts and `--workers=1`; the user must be reminded immediately before the run that successful issuance creates immutable sandbox audit records.

## Success Criteria

- Thai upper marks remain complete in the editor, exact preview, and exported certificate PDF.
- New images do not distort during normal dragging, can be intentionally unlocked, and can be reset to source ratio.
- A user can select multiple static font files, review correctly detected variants grouped by family, confirm rights once, and attach the selected valid batch atomically.
- The editor offers only real weight/style variants and never synthesizes or silently substitutes a face.
- Exact preview communicates all font and render waiting time with an accessible spinner and deterministic retry behavior.
- Existing templates remain readable and visually stable before an intentional edit.
