# Multi-Selection Property Editing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement explicit multi-object selection and batch property editing, with mixed-value UI, mirrored canvas/list selection, and no resize handles for multi-selection frames.

**Architecture:** Add one Tauri batch element update command so one property-panel edit produces one backend action/history entry. Extend `EditorStore` to own range/toggle selection semantics, update `LayerPanel.svelte` to use those semantics, update `GuiRenderer` selection frames to mirror all selected rows without multi-selection resize handles, and refactor `PropertyPanel.svelte` around selection-derived editable field sets.

**Tech Stack:** Svelte 5 runes, TypeScript, Tauri 2, Rust command tests, PixiJS renderer, existing session logging and project store APIs.

---

## File Map

- Modify `src-tauri/src/commands.rs`
  - Add `ElementPatch` and `element_update_many`.
  - Reuse existing `apply_element_changes`.
  - Add Rust tests for one-history-entry batch updates and atomic failure.
- Modify `src-tauri/src/lib.rs`
  - Register the new `element_update_many` Tauri command.
- Modify `src/lib/api.ts`
  - Add `ElementPatchRequest`, mock `element_update_many`, and exported `elementUpdateMany`.
- Modify `src/lib/stores/editor.svelte.ts`
  - Add primary-selection-preserving set selection helpers and object-list range anchor.
- Modify `src/lib/stores/project.svelte.ts`
  - Add `updateElements()` batch helper that calls `api.elementUpdateMany`, hydrates, refreshes sessions, and logs through existing invoke logging.
- Modify `src/lib/components/LayerPanel.svelte`
  - Use `Ctrl`/`Cmd+click` and `Shift+click`.
  - Use `editor.selectedIds.has(id)` for selected row state.
  - Keep attached-region row selection separate.
- Modify `src/lib/engine/renderer.ts`
  - Use `Ctrl`/`Cmd` toggling on canvas.
  - Draw resize handles only when exactly one element is selected.
  - Keep selection frames mirrored to `editor.selectedIds`.
- Modify `src/lib/components/PropertyPanel.svelte`
  - Replace implicit slot-wide update behavior with explicit selected-object batch editing.
  - Add mixed-value helpers.
  - Show same-type vs mixed-type field sets from the approved spec.
  - Remove `Apply to all slots`.
- Test with:
  - `cargo test --manifest-path src-tauri/Cargo.toml element_update_many`
  - `npx @sveltejs/mcp svelte-autofixer ./src/lib/components/LayerPanel.svelte --svelte-version 5`
  - `npx @sveltejs/mcp svelte-autofixer ./src/lib/components/PropertyPanel.svelte --svelte-version 5`
  - `npx svelte-check --tsconfig ./tsconfig.json`
  - `npm run build`

---

### Task 1: Add Tauri Batch Element Updates

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Test: `src-tauri/src/commands.rs`

- [ ] **Step 1: Add failing Rust tests for batch updates**

Add these tests inside the existing `#[cfg(test)] mod tests` in `src-tauri/src/commands.rs`:

```rust
#[test]
fn element_update_many_changes_multiple_elements_in_one_revision() {
    let mut sessions = ProjectSessionManager::default();
    let project_id =
        sessions.create_session(Project::new("Batch Update", 176, 166, ModTarget::Forge));
    let session = sessions.resolve_mut(Some(&project_id)).unwrap();
    session.project.add_element(sample_element("slot_1", 8, 18));
    session.project.add_element(sample_element("slot_2", 26, 18));

    let result = element_update_many_in_session(
        &mut sessions,
        Some(&project_id),
        vec![
            ElementPatch {
                id: "slot_1".to_string(),
                changes: serde_json::json!({ "asset": "textures/slot.png", "visible": false }),
            },
            ElementPatch {
                id: "slot_2".to_string(),
                changes: serde_json::json!({ "asset": "textures/slot.png", "visible": false }),
            },
        ],
    )
    .unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].asset.as_deref(), Some("textures/slot.png"));
    assert!(!result[0].visible);
    assert_eq!(result[1].asset.as_deref(), Some("textures/slot.png"));
    assert!(!result[1].visible);

    let summary = session_summary(&sessions, &project_id).unwrap();
    assert_eq!(summary.revision, 1);
    assert!(summary.can_undo);
}

#[test]
fn element_update_many_failure_is_atomic() {
    let mut sessions = ProjectSessionManager::default();
    let project_id =
        sessions.create_session(Project::new("Batch Update Failure", 176, 166, ModTarget::Forge));
    let session = sessions.resolve_mut(Some(&project_id)).unwrap();
    session.project.add_element(sample_element("slot_1", 8, 18));

    let error = element_update_many_in_session(
        &mut sessions,
        Some(&project_id),
        vec![
            ElementPatch {
                id: "slot_1".to_string(),
                changes: serde_json::json!({ "visible": false }),
            },
            ElementPatch {
                id: "missing".to_string(),
                changes: serde_json::json!({ "visible": false }),
            },
        ],
    )
    .unwrap_err();

    assert_eq!(error, "Element not found: missing");
    let unchanged = sessions
        .resolve(Some(&project_id))
        .unwrap()
        .project
        .find_element("slot_1")
        .unwrap();
    assert!(unchanged.visible);
    let summary = session_summary(&sessions, &project_id).unwrap();
    assert_eq!(summary.revision, 0);
    assert!(!summary.can_undo);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml element_update_many
```

Expected: FAIL because `ElementPatch` and `element_update_many_in_session` are not defined in `commands.rs`.

- [ ] **Step 3: Add batch update types and helper**

In `src-tauri/src/commands.rs`, near `ElementMove`, add:

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ElementPatch {
    pub id: String,
    pub changes: serde_json::Value,
}
```

Near `update_element_in_session`, add:

```rust
fn element_update_many_in_session(
    sessions: &mut ProjectSessionManager,
    project_id: Option<&str>,
    patches: Vec<ElementPatch>,
) -> Result<Vec<Element>, String> {
    if patches.is_empty() {
        return Ok(Vec::new());
    }

    let updated = {
        let session = sessions.resolve(project_id)?;
        let mut seen = std::collections::HashSet::new();
        let mut updated = Vec::with_capacity(patches.len());

        for patch in &patches {
            if !seen.insert(patch.id.clone()) {
                return Err(format!("Duplicate element update: {}", patch.id));
            }
            let current = session
                .project
                .find_element(&patch.id)
                .ok_or_else(|| format!("Element not found: {}", patch.id))?;
            updated.push(apply_element_changes(current, patch.changes.clone())?);
        }

        updated
    };

    let changed_count = {
        let session = sessions.resolve(project_id)?;
        updated
            .iter()
            .filter(|element| {
                session
                    .project
                    .find_element(&element.id)
                    .is_some_and(|current| current != *element)
            })
            .count()
    };

    if changed_count == 0 {
        return Ok(updated);
    }

    sessions.record_history(project_id)?;
    let session = sessions.resolve_mut(project_id)?;
    for element in &updated {
        *session
            .project
            .find_element_mut(&element.id)
            .ok_or_else(|| format!("Element not found: {}", element.id))? = element.clone();
    }
    sessions.mark_changed(project_id)?;

    Ok(updated)
}
```

- [ ] **Step 4: Add the Tauri command**

In `src-tauri/src/commands.rs`, below `element_update`, add:

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn element_update_many(
    state: State<AppState>,
    patches: Vec<ElementPatch>,
    project_id: Option<String>,
) -> Result<Vec<Element>, String> {
    let mut sessions = state.sessions.lock().unwrap();
    element_update_many_in_session(&mut sessions, project_id.as_deref(), patches)
}
```

- [ ] **Step 5: Register the command**

In `src-tauri/src/lib.rs`, add `commands::element_update_many` to the `tauri::generate_handler!` list next to `commands::element_update`.

The relevant block should include:

```rust
commands::element_update,
commands::element_update_many,
commands::element_resize,
```

- [ ] **Step 6: Add frontend API types and mock support**

In `src/lib/api.ts`, below `ElementMoveRequest`, add:

```ts
export interface ElementPatchRequest {
  id: string;
  changes: ElementChanges;
}
```

In `mockInvoke`, add a case next to `"element_update"`:

```ts
case "element_update_many": {
  const session = mockSession(args?.project_id);
  const patches = ((args?.patches as ElementPatchRequest[] | undefined) ?? []).map(patch => ({
    id: patch.id,
    changes: clone(patch.changes),
  }));
  if (patches.length === 0) return [];

  const seen = new Set<string>();
  const nextElements = patches.map(patch => {
    if (seen.has(patch.id)) throw `Duplicate element update: ${patch.id}`;
    seen.add(patch.id);
    const current = session.project.elements.find(element => element.id === patch.id);
    if (!current) throw `Element not found: ${patch.id}`;
    return { ...current, ...patch.changes };
  });

  if (nextElements.some(next => {
    const current = session.project.elements.find(element => element.id === next.id);
    return JSON.stringify(current) !== JSON.stringify(next);
  })) {
    const previous = clone(session.project);
    for (const next of nextElements) {
      const index = session.project.elements.findIndex(element => element.id === next.id);
      session.project.elements[index] = clone(next);
    }
    markMockChanged(session, previous);
  }

  return nextElements.map(element => clone(element));
}
```

Then add the exported API function near `elementUpdate`:

```ts
export async function elementUpdateMany(patches: ElementPatchRequest[], projectId?: string): Promise<Element[]> {
  const invoke = await getInvoke();
  return invoke("element_update_many", { patches, project_id: projectId }) as Promise<Element[]>;
}
```

- [ ] **Step 7: Run backend and frontend verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml element_update_many
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: Rust tests PASS, Svelte check reports `0 errors and 0 warnings`.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src/lib/api.ts
git commit -m "Add batch element update command"
```

---

### Task 2: Add Project and Editor Selection Helpers

**Files:**
- Modify: `src/lib/stores/project.svelte.ts`
- Modify: `src/lib/stores/editor.svelte.ts`

- [ ] **Step 1: Add `ProjectStore.updateElements`**

In `src/lib/stores/project.svelte.ts`, add this method next to `updateElement`:

```ts
async updateElements(patches: api.ElementPatchRequest[]): Promise<Element[]> {
  const filtered = patches.filter(patch => this.elementById(patch.id));
  if (filtered.length === 0) return [];

  const updated = await api.elementUpdateMany(filtered, this.activeProjectId ?? undefined);
  for (const element of updated) {
    const current = this.elements.find(candidate => candidate.id === element.id);
    if (current) Object.assign(current, element);
  }

  await this.refreshSessions();
  await this.hydrateActiveProject();
  return updated;
}
```

- [ ] **Step 2: Add primary-preserving editor selection helpers**

In `src/lib/stores/editor.svelte.ts`, add a field near `selectedIds`:

```ts
selectionAnchorId = $state<string | null>(null);
```

Replace `selectElement` with:

```ts
selectElement(id: string | null, additive = false) {
  if (this.selectedAttachedRegionId !== null) {
    this.selectedAttachedRegionId = null;
    this.regionSelectionRevision += 1;
  }
  if (additive && id) {
    const next = new Set(this.selectedIds);
    if (next.has(id)) {
      next.delete(id);
      this.selectedElementId = next.size > 0 ? [...next][0] : null;
    } else {
      next.add(id);
      this.selectedElementId = id;
    }
    this.selectedIds = next;
    this.selectionAnchorId = this.selectedElementId;
  } else {
    this.selectedElementId = id;
    this.selectedIds = id ? new Set([id]) : new Set();
    this.selectionAnchorId = id;
  }
  if (this.selectedIds.size === 0) {
    this.selectedElementId = null;
    this.selectionAnchorId = null;
  }
  this.selectionRevision += 1;
}
```

Add these methods below `selectElement`:

```ts
setSelectedElements(ids: Iterable<string>, primaryId: string | null, anchorId = primaryId) {
  if (this.selectedAttachedRegionId !== null) {
    this.selectedAttachedRegionId = null;
    this.regionSelectionRevision += 1;
  }
  const next = new Set(ids);
  this.selectedIds = next;
  this.selectedElementId = primaryId && next.has(primaryId) ? primaryId : next.values().next().value ?? null;
  this.selectionAnchorId = anchorId && next.has(anchorId) ? anchorId : this.selectedElementId;
  this.selectionRevision += 1;
}

selectElementRange(orderedIds: string[], id: string) {
  const anchor = this.selectionAnchorId && orderedIds.includes(this.selectionAnchorId)
    ? this.selectionAnchorId
    : this.selectedElementId && orderedIds.includes(this.selectedElementId)
      ? this.selectedElementId
      : id;
  const start = orderedIds.indexOf(anchor);
  const end = orderedIds.indexOf(id);
  if (start === -1 || end === -1) {
    this.selectElement(id);
    return;
  }
  const [from, to] = start < end ? [start, end] : [end, start];
  this.setSelectedElements(orderedIds.slice(from, to + 1), id, anchor);
}
```

In `selectAttachedRegion` and `clearSelection`, set `this.selectionAnchorId = null`.

- [ ] **Step 3: Run Svelte check**

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: `0 errors and 0 warnings`.

- [ ] **Step 4: Commit**

```bash
git add src/lib/stores/project.svelte.ts src/lib/stores/editor.svelte.ts
git commit -m "Add multi-selection store helpers"
```

---

### Task 3: Implement Object List Multi-Selection

**Files:**
- Modify: `src/lib/components/LayerPanel.svelte`

- [ ] **Step 1: Add visible element order and row click handler**

In `src/lib/components/LayerPanel.svelte`, add these helpers above derived values:

```ts
function visibleElementIds(rows: LayerRow[]): string[] {
  const ids: string[] = [];
  for (const row of rows) {
    if (row.kind === "element") ids.push(row.element.id);
    if ((row.kind === "group" || row.kind === "attached_region") && !collapsedGroups.has(row.kind === "group" ? row.id : `attached:${row.region.id}`)) {
      for (const element of row.elements) ids.push(element.id);
    }
  }
  return ids;
}

function selectLayerElement(event: MouseEvent, id: string) {
  const orderedIds = visibleElementIds(rows);
  if (event.shiftKey) {
    editor.selectElementRange(orderedIds, id);
    return;
  }
  editor.selectElement(id, event.ctrlKey || event.metaKey);
}
```

- [ ] **Step 2: Use selected set for row styling**

Add a derived selected id set:

```ts
let selectedIds = $derived.by(() => {
  void editor.selectionRevision;
  return editor.selectedIds;
});
```

In the `elementRow` snippet, replace:

```svelte
class:selected={selectedElementId === el.id}
onclick={() => editor.selectElement(el.id)}
```

with:

```svelte
class:selected={selectedIds.has(el.id)}
onclick={(event) => selectLayerElement(event, el.id)}
```

- [ ] **Step 3: Preserve attached-region selection behavior**

Keep attached region rows as single-select rows:

```svelte
onclick={() => editor.selectAttachedRegion(row.region.id)}
```

Do not add `Ctrl`/`Shift` behavior to attached region rows in this task.

- [ ] **Step 4: Run Svelte autofixer and check**

```bash
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/LayerPanel.svelte --svelte-version 5
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: autofixer reports no issues, Svelte check reports `0 errors and 0 warnings`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/LayerPanel.svelte
git commit -m "Add layer list multi-selection"
```

---

### Task 4: Align Canvas Selection Frames and Resize Handles

**Files:**
- Modify: `src/lib/engine/renderer.ts`

- [ ] **Step 1: Support Ctrl/Cmd canvas toggling**

In `onPointerDown`, replace:

```ts
const shiftHeld = e.shiftKey;
```

with:

```ts
const additiveSelection = e.shiftKey || e.ctrlKey || e.metaKey;
```

Replace uses of `shiftHeld` in selection logic:

```ts
if (editor.tool === "select" && editor.selectedElementId && !additiveSelection) {
```

```ts
const keepMultiSelection = !additiveSelection && editor.selectedIds.size > 1 && editor.selectedIds.has(clicked.id);
```

```ts
editor.selectElement(clicked.id, additiveSelection);
```

```ts
} else if (editor.tool === "select" && !clicked && !additiveSelection) {
```

- [ ] **Step 2: Hide resize handles when multiple elements are selected**

In `drawSelection`, replace:

```ts
if (isPrimary) {
```

with:

```ts
if (isPrimary && editor.selectedIds.size === 1) {
```

- [ ] **Step 3: Prevent resize hit testing during multi-selection**

In `onPointerDown`, change the resize-handle guard to:

```ts
if (editor.tool === "select" && editor.selectedElementId && editor.selectedIds.size === 1 && !additiveSelection) {
```

- [ ] **Step 4: Run TypeScript/Svelte check**

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: `0 errors and 0 warnings`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/engine/renderer.ts
git commit -m "Mirror multi-selection frames on canvas"
```

---

### Task 5: Refactor Property Panel Selection Model

**Files:**
- Modify: `src/lib/components/PropertyPanel.svelte`

- [ ] **Step 1: Add multi-selection derived state**

At the top of `PropertyPanel.svelte`, after `selectedEl`, add:

```ts
let selectedElements = $derived.by(() => {
  void editor.selectionRevision;
  return [...editor.selectedIds]
    .map(id => project.effectiveElementById(id))
    .filter((element): element is Element => element !== undefined);
});

let hasMultiSelection = $derived(selectedElements.length > 1);

let selectedTypes = $derived.by(() => {
  const types = new Set(selectedElements.map(element => element.type));
  return types;
});

let isSameTypeMultiSelection = $derived(hasMultiSelection && selectedTypes.size === 1);
let isMixedTypeMultiSelection = $derived(hasMultiSelection && selectedTypes.size > 1);

let selectedSlots = $derived(selectedElements.filter(
  element => element.type === "slot" || element.type === "virtual_slot_cell",
));
```

Delete the existing `selectedSlotIds` derived block.

- [ ] **Step 2: Add mixed-value helpers**

Add these helpers below `numberValue`:

```ts
type MixedValue<T> = { mixed: true; value: null } | { mixed: false; value: T };

function mixedValue<T>(elements: Element[], read: (element: Element) => T): MixedValue<T> {
  if (elements.length === 0) return { mixed: false, value: null as T };
  const first = read(elements[0]);
  const mixed = elements.some(element => JSON.stringify(read(element)) !== JSON.stringify(first));
  return mixed ? { mixed: true, value: null } : { mixed: false, value: first };
}

function mixedSelectValue<T extends string | null | undefined>(field: MixedValue<T>): string {
  if (field.mixed) return "__mixed__";
  return field.value ?? "";
}

function updateSelectedElements(changes: Partial<Element>) {
  const patches = selectedElements.map(element => ({ id: element.id, changes }));
  void project.updateElements(patches);
}

function updateSelectedElementsWhere(predicate: (element: Element) => boolean, changes: Partial<Element>) {
  const patches = selectedElements
    .filter(predicate)
    .map(element => ({ id: element.id, changes }));
  void project.updateElements(patches);
}
```

- [ ] **Step 3: Add compositing derived fields**

Add these derived values:

```ts
let multiLayer = $derived(mixedValue(selectedElements, element => element.layer ?? "background"));
let multiVisible = $derived(mixedValue(selectedElements, element => element.visible ?? true));
let multiAttachedRegion = $derived(mixedValue(selectedElements, element => element.attached_region ?? ""));
let multiSlotAsset = $derived(mixedValue(selectedSlots, element => element.asset ?? ""));
let multiSlotUv = $derived(mixedValue(selectedSlots, element => element.uv ?? null));
let multiSlotRole = $derived(mixedValue(selectedSlots, element => element.slot_role ?? ""));
let multiInventoryGroup = $derived(mixedValue(selectedSlots, element => element.inventory_group ?? ""));
let multiScrollBinding = $derived(mixedValue(selectedSlots, element => element.scroll_binding ?? ""));
```

- [ ] **Step 4: Run Svelte check**

```bash
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/PropertyPanel.svelte --svelte-version 5
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: no Svelte correctness issues and `0 errors and 0 warnings`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/PropertyPanel.svelte
git commit -m "Derive property panel multi-selection state"
```

---

### Task 6: Add Multi-Selection Property UI

**Files:**
- Modify: `src/lib/components/PropertyPanel.svelte`

- [ ] **Step 1: Add a multi-selection branch before the single-element branch**

In the markup, before `{#if selectedEl}`, add:

```svelte
  {#if hasMultiSelection}
    <div class="props-form">
      <div class="prop-row">
        <span class="prop-label">Selection</span>
        <span class="prop-value">{selectedElements.length} objects</span>
      </div>

      <div class="prop-row">
        <label for="multi-layer">Layer</label>
        <select
          id="multi-layer"
          value={mixedSelectValue(multiLayer)}
          onchange={(event) => updateSelectedElements({ layer: event.currentTarget.value as Element["layer"] })}
        >
          {#if multiLayer.mixed}<option value="__mixed__" disabled>Mixed</option>{/if}
          <option value="background">Background</option>
          <option value="overlay">Overlay</option>
          <option value="animatable">Animatable</option>
        </select>
      </div>

      <div class="prop-row">
        <label for="multi-attached-region">Attached region</label>
        <select
          id="multi-attached-region"
          value={mixedSelectValue(multiAttachedRegion)}
          onchange={(event) => updateSelectedElements({ attached_region: event.currentTarget.value || null })}
        >
          {#if multiAttachedRegion.mixed}<option value="__mixed__" disabled>Mixed</option>{/if}
          <option value="">(none)</option>
          {#each project.effectiveAttachedRegions as region (region.id)}
            <option value={region.id}>{region.id}</option>
          {/each}
        </select>
      </div>

      <div class="prop-row">
        <label for="multi-visible">Visible</label>
        <input
          id="multi-visible"
          type="checkbox"
          checked={!multiVisible.mixed && multiVisible.value}
          indeterminate={multiVisible.mixed}
          onchange={(event) => updateSelectedElements({ visible: event.currentTarget.checked })}
        />
      </div>
```

Keep the existing single-object branch as `{:else if selectedEl}`.

- [ ] **Step 2: Add same-type slot controls**

Inside the `hasMultiSelection` branch, after compositing fields, add:

```svelte
      {#if isSameTypeMultiSelection && selectedSlots.length === selectedElements.length}
        <div class="prop-section">
          <div class="section-title">Slot</div>
          <div class="prop-row">
            <label for="multi-slot-asset">Background</label>
            <select
              id="multi-slot-asset"
              value={mixedSelectValue(multiSlotAsset)}
              onchange={(event) => updateSelectedElements({ asset: event.currentTarget.value || undefined })}
            >
              {#if multiSlotAsset.mixed}<option value="__mixed__" disabled>Mixed</option>{/if}
              <option value="">(none)</option>
              {#each project.assets as asset (asset)}
                <option value={asset}>{asset.replace("textures/", "").replace(".png", "")}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <label for="multi-slot-role">Role</label>
            <select
              id="multi-slot-role"
              value={mixedSelectValue(multiSlotRole)}
              onchange={(event) => updateSelectedElements({ slot_role: (event.currentTarget.value || null) as SlotRole | null })}
            >
              {#if multiSlotRole.mixed}<option value="__mixed__" disabled>Mixed</option>{/if}
              <option value="">(none)</option>
              {#each slotRoleOptions as role (role)}
                <option value={role}>{role}</option>
              {/each}
            </select>
          </div>
          <div class="prop-row">
            <label for="multi-inventory-group">Group</label>
            <input
              id="multi-inventory-group"
              type="text"
              placeholder={multiInventoryGroup.mixed ? "Mixed" : ""}
              value={multiInventoryGroup.mixed ? "" : multiInventoryGroup.value ?? ""}
              oninput={(event) => updateSelectedElements({ inventory_group: optionalText(event.currentTarget.value) })}
            />
          </div>
          <div class="prop-row">
            <label for="multi-scroll-binding">Scroll</label>
            <input
              id="multi-scroll-binding"
              type="text"
              placeholder={multiScrollBinding.mixed ? "Mixed" : ""}
              value={multiScrollBinding.mixed ? "" : multiScrollBinding.value ?? ""}
              oninput={(event) => updateSelectedElements({ scroll_binding: optionalText(event.currentTarget.value) })}
            />
          </div>
          <button class="secondary-btn" onclick={() => updateSelectedElements({ uv: null })}>
            Clear UV
          </button>
        </div>
      {/if}
```

Do not include X, Y, Size, Width, Height, slot index, or UV numeric controls in the multi-selection UI.

After the same-type slot block, close the multi-selection form and switch the
existing single-object branch to `{:else if selectedEl}`:

```svelte
    </div>
  {:else if selectedEl}
```

- [ ] **Step 3: Remove implicit slot-wide action**

Delete:

```ts
function applySlotBackgroundToAllSlots() {
  if (!selectedEl || (selectedEl.type !== "slot" && selectedEl.type !== "virtual_slot_cell")) return;
  const changes: Partial<Element> = {
    asset: selectedEl.asset,
    uv: selectedEl.uv ?? null,
  };
  updateElementsSequentially(
    project.elements
      .filter(element => element.type === "slot" || element.type === "virtual_slot_cell")
      .map(element => element.id),
    changes,
  );
}
```

Delete the button:

```svelte
<button class="secondary-btn" onclick={applySlotBackgroundToAllSlots}>
  Apply to all slots
</button>
```

- [ ] **Step 4: Keep single-object behavior intact**

In the single-object branch, keep X/Y/size/width/height controls exactly as single-object controls. Replace slot texture multi-update paths so they update only the selected explicit selection:

```ts
function updateTextureProp(key: "asset" | "uv", value: unknown) {
  if (hasMultiSelection) {
    updateSelectedElementsWhere(
      element => element.type === "slot" || element.type === "virtual_slot_cell" || element.type === "texture" || element.type === "progress",
      { [key]: value },
    );
    return;
  }
  updateProp(key, value);
}
```

For single selected slot, this still updates only that slot.

- [ ] **Step 5: Replace UV picker batch paths**

In `applyUvSelection`, replace the selected-slot branch:

```ts
if (selectedEl?.type === "slot" || selectedEl?.type === "virtual_slot_cell") {
  updateElementsSequentially(selectedSlotIds, { asset, uv });
} else {
  updateSelectedElement({ asset, uv });
}
```

with:

```ts
if (hasMultiSelection) {
  updateSelectedElementsWhere(
    element => element.type === "slot" || element.type === "virtual_slot_cell" || element.type === "texture" || element.type === "progress",
    { asset, uv },
  );
} else {
  updateSelectedElement({ asset, uv });
}
```

Then delete the `updateElementsSequentially` helper entirely:

```ts
function updateElementsSequentially(ids: string[], changes: Partial<Element>) {
  void (async () => {
    for (const id of ids) {
      await project.updateElement(id, changes);
    }
  })();
}
```

- [ ] **Step 6: Run Svelte autofixer/check**

```bash
npx @sveltejs/mcp svelte-autofixer ./src/lib/components/PropertyPanel.svelte --svelte-version 5
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: no Svelte correctness issues and `0 errors and 0 warnings`.

- [ ] **Step 7: Commit**

```bash
git add src/lib/components/PropertyPanel.svelte
git commit -m "Add multi-selection property editing UI"
```

---

### Task 7: Manual Verification on Food Pouch Project

**Files:**
- No source edits expected.

- [ ] **Step 1: Start dev server and app**

Run:

```bash
npm run dev -- --host 127.0.0.1 --port 49320
./src-tauri/target/release/mc-gui-crafter /home/inky/Development/minecraft/food-pouch/gui-crafter/large.mcgui
```

Expected: app opens and no `127.0.0.1 connection refused` page appears.

- [ ] **Step 2: Verify object-list selection**

Manual checks:

- Click `food_8`; only `food_8` row and canvas frame are selected.
- `Ctrl+click` `food_17`; both rows and both canvas frames are selected.
- `Shift+click` a later visible slot row; every visible row between the anchor and target is selected.
- Click `background`; previous slot selection clears.

- [ ] **Step 3: Verify canvas selection frames**

Manual checks:

- Multi-selected objects all draw frames.
- Multi-selected objects draw no resize handles.
- Single selected object still draws resize handles.
- Dragging one selected slot moves the selected set according to existing group/attached-region movement rules.

- [ ] **Step 4: Verify property filtering**

Manual checks:

- Select `food_8` and `food_17`: slot background, role, group, scroll, layer, visible, and attached region are available; X/Y/size are not editable.
- Select `background`, `food_8`, and `food_17`: only layer, visible, and attached region are available.
- Mixed fields display `Mixed` or indeterminate state.
- Changing slot background changes only selected slots.
- There is no `Apply to all slots` button.

- [ ] **Step 5: Verify session logs**

Run:

```bash
tail -n 80 "$(ls -t ~/.config/mc-gui-crafter/logs/session-*.jsonl | head -n 1)"
```

Expected: property edits are logged as `element_update_many completed` or equivalent action entries, and no renderer/property errors appear.

- [ ] **Step 6: Final verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml element_update_many
npx svelte-check --tsconfig ./tsconfig.json
npm run build
```

Expected: all commands pass.

- [ ] **Step 7: Commit any verification-only fixes**

If manual verification required small fixes:

```bash
git add <changed-files>
git commit -m "Polish multi-selection property editing"
```

If no fixes were needed, do not create an empty commit.

---

## Self-Review

- Spec coverage:
  - Object-list `Ctrl`/`Cmd+click` and `Shift+click`: Task 3.
  - Property panel edits all explicitly selected supported objects: Tasks 1, 2, 5, 6.
  - Mixed-value fields: Tasks 5 and 6.
  - Remove `Apply to all slots`: Task 6.
  - No multi-selection geometry editing: Task 6.
  - Mixed-type compositing-only fields: Task 6.
  - Canvas/list selection mirror: Tasks 3 and 4.
  - No resize nodes for multi-selection frames: Task 4.
  - Attached region meaning and field label: Task 6 uses `Attached region` and existing `attached_region` model.
- Placeholder scan: no `TBD`, `TODO`, or "implement later" placeholders are present.
- Type consistency:
  - `ElementPatchRequest` is defined in `src/lib/api.ts` before `ProjectStore.updateElements` uses it.
  - `ElementPatch` is defined in `src-tauri/src/commands.rs` before the Tauri command uses it.
  - `selectedElements` and `selectedSlots` are defined before property-panel handlers use them.
