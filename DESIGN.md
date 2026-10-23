# ConfigDeck design system

Calm, dense developer tool. Flat divided rows, no cards, no gradients. One indigo accent. IBM Plex everywhere.
Source decisions: spec DD1 to DD17 (`docs/spec/ConfigDeck_AI_Agent_Build_Spec.md`). Approved reference:
`~/.gstack/projects/altafino-jsonui/designs/workspace-20261023/variant-A-reviewed.png`.
The light theme mirrors the dark structure.

Tokens live in `src/styles/tokens.css` (CSS variables on `:root`, dark values on `:root[data-theme="dark"]`).
`src/styles/tokens.test.ts` asserts the hex values and the contrast rules below.

## Color (DD9, DD10, DD13)

| Token | Dark | Light | Use |
|---|---|---|---|
| `--bg` | `#0B1020` | `#FFFFFF` | app background |
| `--sidebar` | `#10172A` | `#F8FAFC` | sidebar, dialogs, popovers |
| `--row-hover` | `#141C33` | `#F1F5F9` | row hover and focus-within |
| `--border` | `#1E293B` | `#E2E8F0` | row and section dividers |
| `--control-border` | `#64748B` | `#64748B` | inputs, selects, segmented controls, off-state switch (DD13) |
| `--text` | `#E2E8F0` | `#0F172A` | body text |
| `--muted` | `#94A3B8` | `#475569` | meta text, hints |
| `--accent` | `#6366F1` | `#4F46E5` | non-text only: selection tint, active switch track, selected-row bar |
| `--accent-text` | `#818CF8` | `#4F46E5` | focus ring, links, changed dot, accent text |
| `--primary` / hover | `#4F46E5` / `#4338CA` | same | the primary button fill, white text (6.29:1 / 7.90:1) (DD10) |
| `--error` | `#F87171` | `#DC2626` | |
| `--warning` | `#F59E0B` | `#B45309` | amber is reserved for warnings |
| `--success` | `#4ADE80` | `#15803D` | |

Rules: every text pair is at least 4.5:1 (tested). `control-border` is at least 3:1 on `bg` and `sidebar` (tested).
`#6366F1` never carries text (white on it is 4.47:1).

## Status semantics (DD11)

Status is never color alone.

- Changed: accent dot before the label, muted `was <old>` line, the same dot on the sidebar row. Masked values show `was ••••`.
- Warning: amber triangle icon plus text. Error: red circled-exclamation icon plus text. Success: green check plus text.
- Invalid file in the sidebar: error icon plus the word "Invalid".

## Type (DD8)

Bundled via `@fontsource` (offline, OFL): IBM Plex Sans 400/500/600 for UI; IBM Plex Mono 400/500 for keys, paths,
values and the source view. Numbers use tabular numerals (`font-variant-numeric: tabular-nums` on `body`).

| Role | Size / weight |
|---|---|
| File title | 20 / 600 |
| Labels and values, comfortable | 16 / 400 (`--fs-body: 1rem`) |
| Labels and values, compact | 14 (`--fs-body: 0.875rem`) |
| Mono meta (keys, paths, was) | 13 |
| Section labels | 12 / 500, caps, tracking |

All sizes are rem; Settings > font size sets `--font-scale` (0.875 to 1.5) on `html`.

## Space, shape, motion

- 4 px base (4/8/12/16/24/32). Row height 44 comfortable, 32 compact (`--row-h`); control height 36 / 28 (`--ctl-h`).
- Radius 6 for controls, 8 for dialogs. 1 px borders. Shadow only on dialogs and popovers.
- Motion: 120 ms ease-out on expand/collapse only. Disabled under `prefers-reduced-motion`.
- Browser surfaces: thin themed scrollbars, `::selection` accent at 30%, accent caret, 2 px focus ring with 2 px offset
  (`:focus-visible`, `--accent-text`).

## Layout (DD12)

- Sidebar: resizable (220 to 480 px, width remembered in preferences, arrow keys on the separator). Below 1180 px, or when
  collapsed with Ctrl+B, it is a 56 px icon rail with status dots and names in tooltips; on narrow windows the rail opens as an overlay.
- Form rows: label column at least 200 px, control column at least 280 px (and at most 420 px). Labels wrap to 2 lines, then clip.
  Paths middle-truncate and keep the file name visible.
- Dialogs: `min(720px, 90vw)` (wide variant `min(880px, 92vw)`).
- Form column is capped at 960 px above 1600 px. No horizontal scrolling from 1024 px up (Playwright-checked).
- Header holds identity and file-level actions; Review changes and Save changes live only in the footer save bar (DD2).

## Field row anatomy (DD1)

`label` + `control`. The raw key shows under the label only when a schema title says something else, or with Show advanced.
A type hint appears only when it carries information: `null`, `not in file`, mixed array. Hover or focus reveals the JSON path
(`$.logging.level`; RFC 6901 underneath) with "Copy JSON path" in the row's actions menu (also on the context menu key / Shift+F10).
Schema-less labels are humanized (`retryCount` to "Retry count").

Controls: switch (existing boolean), Not set / On / Off radiogroup (optional missing boolean; absent never becomes false),
text (multiline when long), native select (enum), integer/number text input that validates and keeps the exact text,
masked secret input with reveal, `null` pill with "Set value", object section (disclosure button, depth 3 then "Open ›" drill-in),
key/value map editor, list editor (drag handle, Alt+Up/Down), array-of-objects flat rows with the DD17 summary rule.

## Accessibility (DD14)

Landmarks: banner, navigation, main, status (save bar). F6 cycles. Sections are disclosure buttons with `aria-expanded`.
The tri-state control is a radiogroup with arrow keys; changed state goes in the accessible description ("changed, was false").
Save feedback uses `aria-live`: polite for success, assertive for failure. Shortcuts are listed in Settings > Shortcuts.
