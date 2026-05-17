# Phase 6 Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish MCGUI Crafter into a daily-usable editor with a better start flow, preferences, shortcut discoverability, export preflight, pixel zoom, and consistent status UX.

**Architecture:** Rust remains authoritative for project sessions, export planning, filesystem checks, and MCP status. Svelte owns local editor preferences and renders compact workflow surfaces. Every durable project mutation still goes through backend APIs; UI-only preferences persist in localStorage.

**Tech Stack:** Tauri 2, Rust 2021, Svelte 5 runes, TypeScript, PixiJS 8, localStorage preferences, existing Rust export/session modules.

---

## File Structure

- `src/lib/stores/preferences.svelte.ts`: new UI-only persisted editor preferences.
- `src/lib/stores/status.svelte.ts`: new compact app status message queue/current-message store.
- `src/lib/components/StartPanel.svelte`: new empty-state project launcher and recent projects surface.
- `src/lib/components/PreferencesDialog.svelte`: grid/snap/preset/theme controls.
- `src/lib/components/ShortcutsDialog.svelte`: shortcut/help reference.
- `src/lib/components/StatusMessages.svelte`: app-level success/warning/error messages.
- `src/lib/components/ExportDialog.svelte`: export preview/preflight UI.
- `src/lib/components/PixelEditor.svelte`: zoom controls and viewport sizing.
- `src/lib/components/Toolbar.svelte`: entry points for preferences/help and status-producing actions.
- `src/lib/components/Canvas.svelte`, `src/lib/engine/renderer.ts`, `src/lib/stores/editor.svelte.ts`: grid/snap preference integration.
- `src/lib/api.ts`: typed wrapper for export preview command and browser mock.
- `src-tauri/src/commands.rs`: `project_export_preview` Tauri command.
- `src-tauri/src/export/mod.rs`: shared export planning/preflight function.
- `docs/roadmap.md`, `README.md`: Phase 6 status and feature docs after implementation.

---

## Task 1: Preferences Store And Grid/Snap Integration

**Files:**

- Create: `src/lib/stores/preferences.svelte.ts`
- Modify: `src/lib/stores/editor.svelte.ts`
- Modify: `src/lib/engine/renderer.ts`
- Modify: `src/lib/components/Canvas.svelte`
- Modify: `src/lib/components/StatusBar.svelte`
- Test: `pnpm verify`

- [ ] **Step 1: Create persisted preferences store**

Create `src/lib/stores/preferences.svelte.ts`:

```ts
export interface EditorPreferences {
  showGrid: boolean;
  snapToGrid: boolean;
  majorGridSize: number;
  minorGridSize: number;
  snapSize: number;
  defaultPreset: string;
  theme: "dark" | "high_contrast";
}

const STORAGE_KEY = "mcgui_preferences";

const defaults: EditorPreferences = {
  showGrid: true,
  snapToGrid: true,
  majorGridSize: 18,
  minorGridSize: 2,
  snapSize: 1,
  defaultPreset: "furnace",
  theme: "dark",
};

function loadPreferences(): EditorPreferences {
  if (typeof localStorage === "undefined") return { ...defaults };
  try {
    const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}") as Partial<EditorPreferences>;
    return { ...defaults, ...parsed };
  } catch {
    return { ...defaults };
  }
}

class PreferencesStore {
  values = $state<EditorPreferences>(loadPreferences());

  update(changes: Partial<EditorPreferences>) {
    this.values = { ...this.values, ...changes };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(this.values));
  }

  reset() {
    this.values = { ...defaults };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(this.values));
  }
}

export const preferences = new PreferencesStore();
```

- [ ] **Step 2: Route editor snap/grid defaults through preferences**

Modify `src/lib/stores/editor.svelte.ts` so `showGrid` and `snapToGrid` remain editor state only if they are actively used elsewhere; otherwise read from `preferences.values`. Add a helper:

```ts
snap(value: number, snapSize: number): number {
  if (snapSize <= 1) return Math.round(value);
  return Math.round(value / snapSize) * snapSize;
}
```

Expected behavior: drag/resize coordinates use `snapSize` when snap is enabled.

- [ ] **Step 3: Update renderer grid sizes**

In `src/lib/engine/renderer.ts`, import `preferences` and replace hardcoded grid constants with `preferences.values.majorGridSize` and `preferences.values.minorGridSize` inside render-time code. Keep fallback minimums:

```ts
const major = Math.max(1, preferences.values.majorGridSize);
const minor = Math.max(1, preferences.values.minorGridSize);
```

- [ ] **Step 4: Track preferences as canvas render dependencies**

In `src/lib/components/Canvas.svelte`, add dependency reads for:

```ts
void preferences.values.showGrid;
void preferences.values.majorGridSize;
void preferences.values.minorGridSize;
void preferences.values.snapToGrid;
void preferences.values.snapSize;
```

- [ ] **Step 5: Verify**

Run:

```bash
pnpm verify
```

Expected: `svelte-check found 0 errors and 0 warnings`, Vite build passes.

---

## Task 2: Start Panel And Recent Projects

**Files:**

- Create: `src/lib/components/StartPanel.svelte`
- Modify: `src/App.svelte`
- Modify: `src/lib/stores/project.svelte.ts`
- Modify: `src/lib/components/NewProjectDialog.svelte`
- Test: `pnpm verify`

- [ ] **Step 1: Add recent-project helpers**

In `src/lib/stores/project.svelte.ts`, expose:

```ts
removeRecentProject(path: string) {
  const recent = ProjectStore.getRecentProjects().filter(p => p !== path);
  localStorage.setItem("mcgui_recent", JSON.stringify(recent));
}
```

- [ ] **Step 2: Create start panel**

Create `src/lib/components/StartPanel.svelte` with props:

```ts
let { onnew, onopen }: { onnew: () => void; onopen: () => void } = $props();
```

Render:

- Recent projects list from `ProjectStore.getRecentProjects()`.
- Buttons for New Project and Open Project.
- Compact MCP endpoint from `api.mcpStatus()` if available.
- Empty recent state text when no entries exist.

- [ ] **Step 3: Wire start panel into app shell**

In `src/App.svelte`, render `StartPanel` in the main editor area when `!project.isOpen`. Keep toolbar visible.

- [ ] **Step 4: Improve failed recent open behavior**

When opening a recent project fails, show a status error and offer a remove button in the row. Do not silently delete it.

- [ ] **Step 5: Verify**

Run:

```bash
pnpm verify
```

Expected: frontend check/build passes with no warnings.

---

## Task 3: Preferences Dialog And GUI Presets

**Files:**

- Create: `src/lib/components/PreferencesDialog.svelte`
- Modify: `src/lib/components/Toolbar.svelte`
- Modify: `src/lib/components/NewProjectDialog.svelte`
- Create or Modify: `src/lib/guiPresets.ts`
- Test: `pnpm verify`

- [ ] **Step 1: Add shared presets**

Create `src/lib/guiPresets.ts`:

```ts
export interface GuiPreset {
  id: string;
  label: string;
  width: number;
  height: number;
}

export const guiPresets: GuiPreset[] = [
  { id: "furnace", label: "Furnace / Inventory", width: 176, height: 166 },
  { id: "chest_9x3", label: "Chest 9x3", width: 176, height: 166 },
  { id: "chest_9x6", label: "Chest 9x6", width: 176, height: 222 },
  { id: "hopper", label: "Hopper", width: 176, height: 133 },
  { id: "custom", label: "Custom", width: 176, height: 166 },
];
```

- [ ] **Step 2: Add preferences dialog**

Create controls for grid visible, snap enabled, major grid, minor grid, snap size, default preset, and theme. All controls call `preferences.update(...)`.

- [ ] **Step 3: Add toolbar entry point**

Add a compact settings button to `Toolbar.svelte`, with a tooltip and dialog state.

- [ ] **Step 4: Use presets in New Project**

In `NewProjectDialog.svelte`, add a preset select above width/height. Selecting a preset updates dimensions. Manual edits set preset to `custom`.

- [ ] **Step 5: Verify**

Run:

```bash
pnpm verify
```

Expected: frontend check/build passes with no warnings.

---

## Task 4: Shortcut Reference

**Files:**

- Create: `src/lib/components/ShortcutsDialog.svelte`
- Modify: `src/lib/components/Toolbar.svelte`
- Modify: `src/lib/components/ElementPalette.svelte`
- Modify: `src/lib/engine/renderer.ts` or `src/lib/stores/editor.svelte.ts` if adding shortcuts
- Test: `pnpm verify`

- [ ] **Step 1: Define shortcut groups**

Create `ShortcutsDialog.svelte` with a local array:

```ts
const groups = [
  { title: "Project", items: [["Ctrl+N", "New project"], ["Ctrl+O", "Open project"], ["Ctrl+S", "Save"], ["Ctrl+Shift+S", "Save as"]] },
  { title: "Tools", items: [["V", "Select"], ["H", "Pan"], ["S", "Slot"], ["T", "Texture"], ["X", "Text"]] },
  { title: "View", items: [["+", "Zoom in"], ["-", "Zoom out"], ["0", "Reset view"], ["G", "Toggle grid"]] },
  { title: "Edit", items: [["Delete", "Delete selection"], ["Ctrl+D", "Duplicate"], ["Ctrl+G", "Group"], ["Ctrl+Shift+G", "Ungroup"], ["Ctrl+Z", "Undo"], ["Ctrl+Y", "Redo"]] },
  { title: "Timeline", items: [["Space", "Play/pause preview"]] },
];
```

- [ ] **Step 2: Ensure listed shortcuts exist**

If a listed shortcut does not exist, either implement it or remove it from the dialog. The reference must never list non-working shortcuts.

- [ ] **Step 3: Add toolbar help button**

Add a help button to `Toolbar.svelte` that opens the dialog. Add `?` as a keyboard shortcut only if it does not conflict with text inputs.

- [ ] **Step 4: Verify**

Run:

```bash
pnpm verify
```

Expected: frontend check/build passes with no warnings.

---

## Task 5: Status Message System

**Files:**

- Create: `src/lib/stores/status.svelte.ts`
- Create: `src/lib/components/StatusMessages.svelte`
- Modify: `src/App.svelte`
- Modify: `src/lib/components/Toolbar.svelte`
- Modify: `src/lib/components/AssetLibrary.svelte`
- Modify: `src/lib/components/ExportDialog.svelte`
- Test: `pnpm verify`

- [ ] **Step 1: Create status store**

Create `src/lib/stores/status.svelte.ts`:

```ts
export type StatusKind = "success" | "warning" | "error" | "info";

export interface StatusMessage {
  id: number;
  kind: StatusKind;
  text: string;
}

class StatusStore {
  current = $state<StatusMessage | null>(null);
  private nextId = 1;
  private timer: ReturnType<typeof setTimeout> | null = null;

  show(kind: StatusKind, text: string, timeout = 4000) {
    this.current = { id: this.nextId++, kind, text };
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.current = null;
      this.timer = null;
    }, timeout);
  }

  clear() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.current = null;
  }
}

export const status = new StatusStore();
```

- [ ] **Step 2: Render status messages**

Create `StatusMessages.svelte` as a compact region with `role="status"` for success/info and `role="alert"` for warnings/errors.

- [ ] **Step 3: Replace silent failures**

Add `status.show(...)` calls for save, save as, open failure, asset import/update failure, export success/failure.

- [ ] **Step 4: Verify**

Run:

```bash
pnpm verify
```

Expected: frontend check/build passes with no warnings.

---

## Task 6: Backend Export Preview

**Files:**

- Modify: `src-tauri/src/export/mod.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/components/ExportDialog.svelte`
- Test: `src-tauri/src/export/mod.rs`

- [x] **Step 1: Add export preview model**

In `src-tauri/src/export/mod.rs`, add serializable structs:

```rust
#[derive(Debug, serde::Serialize)]
pub struct ExportPreview {
    pub target: String,
    pub mod_id: String,
    pub package: String,
    pub class_name: String,
    pub output_dir: String,
    pub files: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}
```

- [x] **Step 2: Add planning function**

Add:

```rust
pub fn preview_export(project: &Project, config: &ExportConfig, target: &str) -> Result<ExportPreview, String>
```

It must reuse the same sanitization and planned paths as `export_project`, without writing files.

- [x] **Step 3: Detect missing textures and overwrites**

Preview should put missing texture references in `errors`. Existing target files should be listed in `warnings`.

- [x] **Step 4: Add command and frontend API**

Add Tauri command:

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn project_export_preview(...) -> Result<crate::export::ExportPreview, String>
```

Register it in `src-tauri/src/lib.rs` and add `api.projectExportPreview(...)`.

- [x] **Step 5: Render preview in export dialog**

Update `ExportDialog.svelte` to call preview when target/mod/package/class/output changes and show file tree, warnings, and errors. Disable export when preview has errors.

- [x] **Step 6: Add Rust tests**

Add tests that assert preview:

- uses sanitized names,
- includes planned Java/resource files,
- reports missing textures,
- reports existing output files as warnings.

- [x] **Step 7: Verify**

Run:

```bash
cargo test
cargo build
pnpm verify
```

Expected: Rust tests pass, Rust build warning-free, frontend passes.

---

## Task 7: Pixel Editor Zoom

**Files:**

- Modify: `src/lib/components/PixelEditor.svelte`
- Test: `pnpm verify`

- [ ] **Step 1: Add zoom state**

Add:

```ts
let zoom = $state<1 | 2 | 4 | 8 | "fit">(4);
```

- [ ] **Step 2: Render zoom controls**

Add segmented buttons for `1x`, `2x`, `4x`, `8x`, and `Fit`.

- [ ] **Step 3: Apply pixelated sizing**

Set canvas style width/height from image dimensions and zoom. Use CSS:

```css
.pe-canvas {
  image-rendering: pixelated;
  image-rendering: crisp-edges;
}
```

For fit mode, constrain the canvas wrapper with max viewport dimensions and let CSS scale the canvas while preserving aspect ratio.

- [ ] **Step 4: Verify pointer mapping**

Confirm `handleCanvasClick` still uses `getBoundingClientRect()` so pointer mapping works at every zoom.

- [ ] **Step 5: Verify**

Run:

```bash
pnpm verify
```

Expected: frontend check/build passes with no warnings.

---

## Task 8: Visual Consistency Pass

**Files:**

- Modify: `src/lib/components/Toolbar.svelte`
- Modify: `src/lib/components/ProjectTabs.svelte`
- Modify: `src/lib/components/NewProjectDialog.svelte`
- Modify: `src/lib/components/ExportDialog.svelte`
- Modify: `src/lib/components/PreferencesDialog.svelte`
- Modify: `src/App.svelte`
- Test: `pnpm verify`

- [x] **Step 1: Normalize icon button dimensions**

Use stable dimensions for toolbar icon buttons and project tab close buttons. Avoid text overflow in project names.

- [x] **Step 2: Normalize modal spacing**

Use consistent title size, row spacing, button alignment, and border radius across New Project, Export, Preferences, Shortcuts, and Pixel Editor.

- [x] **Step 3: Add missing tooltips**

Every icon-only button gets `title` and accessible text or `aria-label`.

- [x] **Step 4: Verify responsive fit**

Run the app manually or use browser dev tooling if available. Check at least 1280x800 and a narrow 900x700 viewport. No overlapping toolbar/status/modal text.

- [x] **Step 5: Verify**

Run:

```bash
pnpm verify
```

Expected: frontend check/build passes with no warnings.

---

## Task 9: Documentation And Roadmap Update

**Files:**

- Modify: `docs/roadmap.md`
- Modify: `README.md`
- Optional Modify: `docs/architecture.md`

- [x] **Step 1: Update Phase 6 checklist**

Mark implemented polish items complete. Move non-goals to Phase 7 or Phase 6.x.

- [x] **Step 2: Update README feature language**

Document recent projects, preferences, export preview, pixel zoom, and web MCP status only after implemented.

- [x] **Step 3: Verify docs do not overclaim**

Search:

```bash
rg -n "planned|TODO|not implemented|Phase 6|In Progress|stdio|--mcp" README.md docs
```

Expected: any remaining hits are accurate historical plan text or future roadmap items.

---

## Task 10: Final Verification

**Files:**

- No source changes unless verification finds a defect.

- [ ] **Step 1: Run frontend verification**

```bash
pnpm verify
```

Expected: 0 Svelte errors, 0 Svelte warnings, Vite build passes.

- [ ] **Step 2: Run Rust verification**

```bash
cd src-tauri
cargo fmt --all -- --check
cargo test
cargo build
```

Expected: formatting clean, all tests pass, build warning-free.

- [ ] **Step 3: Manual smoke checklist**

Verify manually:

- New project from start panel.
- Open recent project.
- Toggle grid/snap settings and drag an element.
- Open shortcuts dialog.
- Preview export, then export.
- Pixel editor zoom at 1x and 8x.
- Save, undo, redo.
- MCP status appears without starting another app instance.

---

## Plan Self-Review

- Spec coverage: every design-spec scope item maps to a task: start/recent projects (Task 2), preferences/grid/snap/presets/theme (Tasks 1 and 3), shortcut reference (Task 4), export preview (Task 6), pixel zoom (Task 7), status/error UX (Task 5), visual consistency (Task 8), docs (Task 9), verification (Task 10).
- Placeholder scan: no `TBD`, deferred implementation holes, or intentionally vague feature placeholders remain. Any future work is listed as a non-goal in the design spec.
- Type consistency: `EditorPreferences`, `GuiPreset`, `StatusMessage`, and `ExportPreview` are introduced before later tasks reference them.
- Execution split: Tasks 1-5 and 7-8 are frontend-heavy and can be subagent-driven with limited overlap. Task 6 is backend/export-heavy and should run in isolation from ExportDialog frontend edits unless explicitly coordinated.
