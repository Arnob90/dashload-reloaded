# Plan: Make the frontend look sleek and modern

## Task

PROMPT.md: "I think the frontend is poorly styled. Make it look sleek and modern."

This is a pure styling task. No changes to backend logic, data flow, hooks, or component
props. Every step must be verifiable with `bun run build` (runs `tsc && vite build`).

## Approach

Work from the top of the styling stack down: global theme first, then per-surface CSS.
This keeps every change small, reviewable, and independently verifiable. Each step below
can be committed and tested on its own with `bun run build`.

### Design direction
- **Depth & elevation** — soft multi-layer box-shadows, subtle `backdrop-filter` glassmorphism on key surfaces (header, details pane, modals).
- **Color** — shift the flat cyan primary to a refined indigo accent; keep `teal` for success/completed, `amber` for paused, `red` for failed. Introduce a true dark palette (slightly bluish grays) so the dark scheme feels intentional, not just "Mantine dark".
- **Typography** — keep Inter; add a robust system-font fallback stack so the app never renders with a missing font. Improve heading hierarchy via font-weight and letter-spacing.
- **Motion** — a single 150–220ms ease transition on interactive elements (rows, icons, buttons).
- **Shape** — raise default radius to `lg`; pill-shaped progress bars (already present) are kept.
- **Spacing & layout** — tighten the header, give the table proper sticky headers, modern thin scrollbars.

---

## Files to modify

| File | What changes |
|------|-------------|
| `src/App.tsx` | Expand `createTheme` with full design tokens. Remove redundant `<Notifications />` (rendered twice). Keep `forceColorScheme="dark"`. |
| `src/App.css` | Replace commented-out boilerplate with real global CSS: reset, dark background, thin scrollbars, selection color, focus rings, base transitions. |
| `src/App.module.css` | Delete — unused (no `.flex`/`.centered` references in `App.tsx`; layout is done via Mantine `<Flex>`). Verify no imports before deleting. |
| `src/components/header/downloaditemheader.tsx` | Replace raw `TextInput` + `ActionIcon` with a proper pill-shaped search bar (search icon inside the input) and a branded primary "Add" button. |
| `src/components/header/header.module.css` | Replace the flat gray separator with a subtle gradient hairline; add glass background to the header bar. |
| `src/components/downloadtable/downloadtable.tsx` | Add a sticky table header, refine row density, and swap the `bg="dark.8"` thead for the theme surface color. |
| `src/components/downloadtable/downloadtable.module.css` | Modern selected-row (accent-tinted left border + tinted background), smoother hover, sticky header background. |
| `src/components/downloadtable/downloadDetails.tsx` | Glass/elevation for the details pane; refine card surfaces. Keep all logic untouched. |
| `src/components/selectabletable/selectableItemsTable.tsx` | (Optional) pass through the modern styling; ensure `Table` uses the theme surface. |
| `src/components/adddownload/addDownloadUrl.tsx` | Make the primary "Download" button stand out; refine input width. |
| `src/components/adddownload/runDownload.tsx` | Refine the "Start Download" primary button and section spacing. |
| `src/components/adddownload/collapseSection.tsx` / `advancedOptions.tsx` / `moreInfo.tsx` | Harmonize collapsed-section surfaces with the new theme. |

Files **not** to touch (no style surface, or logic only):
- `src/code/*` (hooks, API, types, defaults, utils)
- `src-tauri/*` (Rust backend)
- `index.html` (title only — optionally update title to "DashLoad")

---

## Step-by-step

### Step 1 — Global theme tokens in `App.tsx`
**Why first:** Every component reads from the Mantine theme. Establishing tokens here means
later steps only reference variables.

Change `src/App.tsx`:
- Expand `createTheme({...})` to include:
  - `primaryColor: "indigo"` (sleeker than cyan) — or keep a custom hex `#6366f1` via `colors`.
  - `defaultRadius: "md"` (leave; individual components will use `lg`/`xl` where needed).
  - `shadows`:
    - `sm: "0 1px 2px rgba(0,0,0,0.3)"`
    - `md: "0 4px 12px rgba(0,0,0,0.4)"`
    - `lg: "0 12px 40px rgba(0,0,0,0.5)"`
  - `fonts: { default: "Inter, ui-sans-serif, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif" }` and set `fontFamily: "Inter"`.
  - `other: { ... }` with custom CSS vars for glass: `glassBg: "rgba(24, 26, 32, 0.7)"`, `glassBorder: "rgba(255,255,255,0.08)"`.
- Remove the duplicate `<Notifications />` (currently rendered twice — once before the `<Flex>` and once inside it).
- Keep `forceColorScheme="dark"`.

**Verify:** `bun run build`. Visually the primary color shifts to indigo.

**Edge cases:**
- Mantine v9 uses `theme.shadows` as an object; confirm key names (`xs`/`sm`/`md`/`lg`/`xl`/`2xl`).
- Do not remove `Notifications` entirely — keep exactly one instance.

---

### Step 2 — Global CSS in `App.css`
Replace the entire commented-out block with real, minimal global CSS:
- CSS reset: `* { box-sizing: border-box; }`, `html, body, #root { height: 100%; margin: 0; }`.
- Body background: a deep, slightly bluish dark `#0b0d12` with an optional subtle radial gradient for depth.
- Custom thin scrollbars (WebKit + Firefox) with low-contrast thumb.
- `::selection` tinted with the primary color at low alpha.
- A base transition utility for interactive Mantine elements is *not* needed globally (Mantine handles it); instead define it per component in step 5/6.
- Ensure `color-scheme: dark` is set on `:root` so native form controls render dark.

**Verify:** `bun run build`. Background and scrollbars update globally.

**Edge cases:**
- `color-scheme: dark` must be on `:root`, not a child, to affect the whole document.
- Keep the file small — do not reintroduce the old commented boilerplate.

---

### Step 3 — Header (`downloaditemheader.tsx` + `header.module.css`)
Make the top bar a sleek glass strip:
- In `header.module.css`:
  - Add a `.container` class: `backdrop-filter: blur(12px)`, `background: var(--app-glass-bg)`, subtle `border-bottom: 1px solid var(--app-glass-border)`.
  - Replace `.separator` (flat `#707070`) with a gradient hairline: `background: linear-gradient(90deg, transparent, rgba(255,255,255,0.12), transparent)`, height `1px`.
- In `downloaditemheader.tsx`:
  - Turn the search into a single pill: use `TextInput` with `leftSection={<Search size={16} />}`, `variant="filled"`, `radius="xl"`, `style={{ background: "rgba(255,255,255,0.05)", border: "none" }}`, fixed width (e.g. `w={320}`).
  - Remove the separate gray `Search` `ActionIcon` (now inside the input).
  - Style the "Add" control as a primary `Button` with `leftSection={<Plus size={16}/>}` and label "New Download" (or keep it icon-only `variant="light" radius="xl"` — pick icon-only for a cleaner bar). Keep `onClick={onClickAddDownload}`.
  - Preserve the `onClickSearch?.(textInputContents)` handler (currently it is only wired to the search icon; move it to the input's `onKeyDown` Enter, or keep a subtle submit).

**Verify:** `bun run build`. Header shows a glassy strip, pill search, refined add button.

**Edge cases:**
- `onClickSearch` prop is currently optional and unused by the caller (`App.tsx` doesn't pass it). Do not add new required props.
- `Plus size="2vh"` is a hack — replace with a fixed `16`px icon.
- Ensure the header still spans full width (`w="100%"`).

---

### Step 4 — Download table (`downloadtable.tsx` + `downloadtable.module.css`)
- **Sticky header:** add a `.sticky_thead th { position: sticky; top: 0; z-index: 2; background: <surface>; }` in the CSS module and attach the class to `Table.Thead`. The surface should match the app background so content scrolling under it is masked.
- **Row hover / selected:** in `downloadtable.module.css`:
  - `.table_row:hover td` → subtle `rgba(255,255,255,0.04)` (softer than the current flat `dark.6`).
  - `.selected_row td` → `background: rgba(99,102,241,0.10)` plus a `box-shadow: inset 3px 0 0 var(--mantine-color-indigo-6)` on the first cell for an accent bar. Add `transition: background-color 150ms ease`.
  - Keep the existing focus-ring removal, but add a `:focus-visible` outline on the row for keyboard users (`outline: 2px solid var(--mantine-color-indigo-5); outline-offset: -2px`).
- **Thead background:** change `bg="dark.8"` in `downloadtable.tsx` to a surface token that matches the app background (use `styles={{ thead: { background: "..." } }}` or a CSS module class). Keep the uppercase dimmed headers.
- Leave column logic, `getFileIcon`, status color map, and the `Menu` context menu untouched.

**Verify:** `bun run build`. Table header stays pinned on scroll; selected row shows accent bar.

**Edge cases:**
- Sticky `th` requires the scroll container to be the `Box` with `overflowY: auto` (it already is). Ensure `z-index` doesn't overlap the details pane (pane is below, not overlapping, so fine).
- The `Menu` wraps each `<Table.Tr>`; sticky `th` is on the header row only, so no conflict.
- Do not change `SelectableItemsTable` props.

---

### Step 5 — Details pane (`downloadDetails.tsx`)
- Give the `Paper` a glass treatment: `backdrop-filter: blur(16px)`, `background: var(--app-glass-bg)`, `borderTop: 1px solid var(--app-glass-border)`, `boxShadow: "0 -8px 24px rgba(0,0,0,0.4)"`.
- Raise the inner card surfaces (`Paper` with `bg="dark.8"`) to a slightly elevated token with a soft `shadow="xs"` for hierarchy.
- Keep `h={260}`, `radius={0}` (full-width bottom pane) — the rounded top corners are already disabled via the border override.
- **Do not change** any tab logic, the `useEffect` that syncs the active tab, `buildErrorReport`, or the retry/copy handlers.

**Verify:** `bun run build`. Details pane looks elevated/glassy.

**Edge cases:**
- `backdrop-filter` over a dark background needs the pane to be semi-transparent to show the effect; keep `rgba(24,26,32,0.7)`.
- Ensure the close `ActionIcon` and tab pills remain legible on the new background.

---

### Step 6 — "Add Download" modal forms
Apply consistent, refined styling across the add-download flow:
- `addDownloadUrl.tsx`:
  - Style the URL `Input` with `variant="filled" radius="md"`.
  - Make the submit `Button` `variant="filled" color="indigo" radius="md"` so it reads as the primary action.
- `runDownload.tsx`:
  - `Start Download` button: `variant="filled" color="indigo" radius="md" fw={600}`.
  - `Save Directory` input: keep the folder `ActionIcon`, add `variant="filled"`.
- `collapseSection.tsx` / `advancedOptions.tsx` / `moreInfo.tsx`:
  - Use `radius="md"` on the inner `Paper`s and a consistent `bg` token; keep the `SlidersHorizontal`/`Info` icons.
- These are cosmetic prop tweaks only. **No logic changes** to form state, the `verifier` `useMemo`, `startDownloadNow`, or the `useRequestHeaders` hook.

**Verify:** `bun run build`. The modal buttons are clearly primary; inputs are filled and rounded.

**Edge cases:**
- `RequestHeadersSection` and `VerifierOptions` are reused in multiple places; keep their props unchanged so both call sites still compile.
- `NumberInput` for segments: only adjust `variant`/`radius`, do not change `min`/`max`.

---

### Step 7 — Cleanup
- Delete `src/App.module.css` if truly unused.
  - **Confirm first:** run a content search for `App.module` / `AppModule` imports. (`App.tsx` only imports `./App.css`; the `.module.css` is not referenced anywhere.) If confirmed unused, remove it.
- Update `index.html` `<title>` to "DashLoad" (cosmetic, optional).
- Keep the favicon (`/vite.svg`) as-is (no brand asset provided).

**Verify:** `bun run build`. No dangling imports.

---

## Types involved
- Mantine `createTheme` options (shadows, colors, fonts, other) — `App.tsx`.
- CSS custom properties defined in `App.css` and consumed in modules:
  - `--app-glass-bg`, `--app-glass-border`, and Mantine's built-in `--mantine-color-*` vars.
- No TypeScript type changes; no new exported symbols. All existing component prop types
  (`DownloadHeaderProps`, `TableEntry`, `RunDownloadDialogProps`, `MoreInfoProps`, etc.)
  remain unchanged.

## Edge cases to handle
1. **Duplicate `<Notifications />`** in `App.tsx` — collapse to one (behavior-neutral).
2. **Sticky thead z-index** vs the `Menu` context-menu dropdown — keep thead `z-index` low (2) so the dropdown (portal) still overlays.
3. **`backdrop-filter` over a non-transparent backdrop** — ensure the element background is `rgba` with alpha so blur is visible; otherwise the effect is invisible.
4. **Keyboard focus** — the current CSS removes all focus rings (`outline: none !important`). Reintroduce a `:focus-visible` ring on rows for accessibility without affecting mouse users.
5. **Font availability** — if `Inter` is not installed, fall back to the system stack so text never renders in a serif default.
6. **Primary color change (cyan → indigo)** — any hardcoded `blue`/`teal` status colors in the table are semantic (status colors) and must **not** change; only the app's accent (buttons, selected-row accent, focus) shifts to indigo.
7. **Delete of `App.module.css`** — only after confirming zero import references.

## Out of scope
- Rust backend / Tauri commands / `src/code/*` data layer.
- New features (search is not currently wired end-to-end; we only restyle the existing input).
- Theming beyond dark (the app forces `forceColorScheme="dark"`; light mode is out of scope).

## Testing / verification
There is **no test suite** in this repo (no `*.test.*` / `*.spec.*` files found).
Each step is verified by:
1. `bun run build` — ensures `tsc && vite build` passes (type safety + bundling).
2. `bun run dev` (Vite on port 1420) — manual visual check of the header, table,
   details pane, and add-download modal.
3. For the Tauri shell specifically, `bun run tauri dev` if a native window is needed.

Because styling has no assertions, "passing" = build succeeds + the targeted surfaces
visually match the design direction above.
