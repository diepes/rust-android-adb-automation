# Sidecar JSON file defines MatchTargets; PNG alone is not a Template

A PNG image in `assets/test_images/` becomes a **Template** only when a sidecar JSON file with the same stem exists alongside it (e.g. `login.png` + `login.json`). The sidecar owns all MatchTarget definitions. A PNG without a sidecar is a saved Screenshot — valid for display and future annotation, but not loaded by the matching pipeline.

## Sidecar schema

```json
{
  "match_targets": [
    { "name": "login_button", "crop": { "x": 120, "y": 540, "width": 200, "height": 60 } }
  ]
}
```

`crop` is in Template-image pixel coordinates. Additional per-MatchTarget fields (SearchRegion, Scene links) can be added later without breaking existing sidecars.

## Why not encode crop coordinates in the filename?

The codebase previously used `screenshot[x,y,w,h].png` to encode a single crop region. This was rejected for three reasons:

1. **One crop per file** — filename encoding supports exactly one MatchTarget per PNG; the sidecar supports many.
2. **Fragile on rename** — renaming the file silently breaks the crop definition; a sidecar is an explicit, independent artefact.
3. **Not human-readable** — `login[120,540,200,60].png` is opaque; a named JSON entry communicates intent.

## Why not embed metadata in the PNG itself (e.g. tEXt chunks)?

Embedded metadata requires image-writing on every edit and makes the file uneditable with standard image tools without losing metadata. A plain JSON sidecar can be edited with any text editor and diffed in version control independently of the image.

## TemplateEditor writes sidecars immediately

The GUI TemplateEditor saves each MatchTarget addition or deletion to the sidecar the moment it occurs — there is no "save" step for metadata. This keeps the sidecar as the single source of truth and avoids silent data loss if the app closes unexpectedly.
