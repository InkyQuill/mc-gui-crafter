# Minecraft Visual Fidelity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Minecraft-like light/dark themes, generated default GUI textures, and vanilla-aligned template slot metrics.

**Architecture:** Rust owns deterministic generated PNG assets so new template projects save and export correctly. The renderer adds a reserved-path fallback only for old or partially migrated projects. Template coordinate fixes are tested at the Rust template layer, and theme changes are centralized through CSS tokens used by Svelte components.

**Tech Stack:** Tauri 2, Rust, `image` crate, Svelte 5, TypeScript, PixiJS, Vite 8.

---

## File Structure

- Modify `src-tauri/src/templates/mod.rs`: add slot metric constants, fix grid spacing, attach generated assets when applying templates, add template tests.
- Modify `src-tauri/src/texture/mod.rs`: add deterministic generated texture functions and tests.
- Modify `src-tauri/src/commands.rs`: ensure `project_new` gets generated assets through `apply_template`; no direct command logic should be needed after template changes.
- Modify `src/lib/stores/preferences.svelte.ts`: add `light` to theme union and normalization.
- Modify `src/lib/components/PreferencesDialog.svelte`: add Light theme option and route styles through tokens where this file currently owns global theme CSS.
- Modify `src/lib/engine/renderer.ts`: add reserved generated texture fallback and theme-aware canvas colors.
- Modify common Svelte component styles as needed: replace hardcoded core palette values with CSS variables without restructuring component layout.
- Modify `docs/roadmap.md`: mark this item complete only after verification.

## Task 1: Template Metrics Tests

**Files:**
- Modify: `src-tauri/src/templates/mod.rs`

- [ ] **Step 1: Add failing tests for slot bounds and 18px cadence**

Add this test module at the end of `src-tauri/src/templates/mod.rs`, below `apply_template`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ElementType, ModTarget, Project};

    fn slot_right(element: &crate::project::Element) -> i32 {
        element.x + element.size.unwrap_or(18) as i32
    }

    fn slot_bottom(element: &crate::project::Element) -> i32 {
        element.y + element.size.unwrap_or(18) as i32
    }

    #[test]
    fn starter_template_slots_stay_inside_gui_bounds() {
        for template in list_templates() {
            for element in &template.elements {
                if element.element_type != ElementType::Slot {
                    continue;
                }

                assert!(
                    element.x >= 0,
                    "{} slot {} has negative x {}",
                    template.name,
                    element.id,
                    element.x
                );
                assert!(
                    element.y >= 0,
                    "{} slot {} has negative y {}",
                    template.name,
                    element.id,
                    element.y
                );
                assert!(
                    slot_right(element) <= template.default_width as i32,
                    "{} slot {} right edge {} exceeds width {}",
                    template.name,
                    element.id,
                    slot_right(element),
                    template.default_width
                );
                assert!(
                    slot_bottom(element) <= template.default_height as i32,
                    "{} slot {} bottom edge {} exceeds height {}",
                    template.name,
                    element.id,
                    slot_bottom(element),
                    template.default_height
                );
            }
        }
    }

    #[test]
    fn nine_column_inventory_templates_use_eighteen_pixel_cadence() {
        for name in ["chest_9x3", "chest_9x6"] {
            let template = get_template(name).expect("template exists");
            let first_row: Vec<_> = template
                .elements
                .iter()
                .filter(|element| element.element_type == ElementType::Slot && element.y == 18)
                .collect();

            assert_eq!(first_row.len(), 9, "{name} first row should have 9 slots");
            for pair in first_row.windows(2) {
                assert_eq!(
                    pair[1].x - pair[0].x,
                    18,
                    "{name} slot cadence should be 18px"
                );
            }
            assert_eq!(first_row[8].x + 18 - first_row[0].x, 162);
        }
    }

    #[test]
    fn crafting_grid_uses_eighteen_pixel_cadence() {
        let template = get_template("crafting_3x3").expect("template exists");
        let first_row: Vec<_> = template
            .elements
            .iter()
            .filter(|element| element.element_type == ElementType::Slot && element.id.starts_with("craft_grid_0_"))
            .collect();

        assert_eq!(first_row.len(), 3);
        for pair in first_row.windows(2) {
            assert_eq!(pair[1].x - pair[0].x, 18);
        }
    }

    #[test]
    fn applying_template_inserts_generated_background_asset() {
        let mut project = Project::new("Generated", 1, 1, ModTarget::Forge);

        apply_template(&mut project, "furnace").expect("template applies");

        assert!(project.assets.iter().any(|asset| asset == GENERATED_GUI_PANEL));
        assert!(project.texture_data.contains_key(GENERATED_GUI_PANEL));
    }
}
```

- [ ] **Step 2: Run the focused Rust tests and confirm failure**

Run: `cargo test templates::tests --all-features --locked`

Expected before implementation: at least one failure for 20px cadence or missing `GENERATED_GUI_PANEL`.

## Task 2: Generated Texture Backend

**Files:**
- Modify: `src-tauri/src/texture/mod.rs`
- Modify: `src-tauri/src/templates/mod.rs`

- [ ] **Step 1: Add generated asset constants and helper in `templates`**

Near the top of `src-tauri/src/templates/mod.rs`, add:

```rust
pub const GENERATED_GUI_PANEL: &str = "textures/generated/gui_panel.png";
pub const GENERATED_SLOT: &str = "textures/generated/slot.png";
pub const GENERATED_PROGRESS_ARROW: &str = "textures/generated/progress_arrow.png";
pub const GENERATED_FLUID_TANK: &str = "textures/generated/fluid_tank.png";
pub const GENERATED_ENERGY_BAR: &str = "textures/generated/energy_bar.png";

const SLOT_SIZE: i32 = 18;
const SLOT_STEP: i32 = 18;

fn add_asset_once(project: &mut Project, path: &str, bytes: Vec<u8>) {
    if !project.assets.iter().any(|asset| asset == path) {
        project.assets.push(path.to_string());
    }
    project.texture_data.entry(path.to_string()).or_insert(bytes);
}

fn add_generated_template_assets(project: &mut Project) -> Result<(), String> {
    add_asset_once(
        project,
        GENERATED_GUI_PANEL,
        crate::texture::generated_gui_panel(project.gui_size.width, project.gui_size.height)?,
    );
    add_asset_once(project, GENERATED_SLOT, crate::texture::generated_slot()?);
    add_asset_once(
        project,
        GENERATED_PROGRESS_ARROW,
        crate::texture::generated_progress_arrow()?,
    );
    add_asset_once(project, GENERATED_FLUID_TANK, crate::texture::generated_fluid_tank()?);
    add_asset_once(project, GENERATED_ENERGY_BAR, crate::texture::generated_energy_bar()?);
    Ok(())
}
```

- [ ] **Step 2: Add deterministic texture functions in `texture/mod.rs`**

Add these functions above the test module in `src-tauri/src/texture/mod.rs`:

```rust
fn encode_png(img: RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .map_err(|e| format!("Failed to encode generated PNG: {e}"))?;
    Ok(bytes)
}

fn noise(x: u32, y: u32) -> u8 {
    let n = x.wrapping_mul(37).wrapping_add(y.wrapping_mul(17)).wrapping_add(13);
    (n % 9) as u8
}

pub fn generated_gui_panel(width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::new(width.max(1), height.max(1));
    let w = img.width();
    let h = img.height();

    for y in 0..h {
        for x in 0..w {
            let shade = 0xb8u8.saturating_add(noise(x, y));
            img.put_pixel(x, y, Rgba([shade, shade, shade, 0xff]));
        }
    }

    for x in 0..w {
        img.put_pixel(x, 0, Rgba([0xff, 0xff, 0xff, 0xff]));
        img.put_pixel(x, h - 1, Rgba([0x55, 0x55, 0x55, 0xff]));
    }
    for y in 0..h {
        img.put_pixel(0, y, Rgba([0xff, 0xff, 0xff, 0xff]));
        img.put_pixel(w - 1, y, Rgba([0x55, 0x55, 0x55, 0xff]));
    }

    encode_png(img)
}

pub fn generated_slot() -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::from_pixel(18, 18, Rgba([0x8b, 0x8b, 0x8b, 0xff]));
    for i in 0..18 {
        img.put_pixel(i, 0, Rgba([0x37, 0x37, 0x37, 0xff]));
        img.put_pixel(0, i, Rgba([0x37, 0x37, 0x37, 0xff]));
        img.put_pixel(i, 17, Rgba([0xff, 0xff, 0xff, 0xff]));
        img.put_pixel(17, i, Rgba([0xff, 0xff, 0xff, 0xff]));
    }
    for y in 2..16 {
        for x in 2..16 {
            img.put_pixel(x, y, Rgba([0x70, 0x70, 0x70, 0xff]));
        }
    }
    encode_png(img)
}

pub fn generated_progress_arrow() -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::from_pixel(22, 15, Rgba([0x00, 0x00, 0x00, 0x00]));
    for y in 4..11 {
        for x in 0..14 {
            img.put_pixel(x, y, Rgba([0x8a, 0x8a, 0x8a, 0xff]));
        }
    }
    for offset in 0..7 {
        for y in (4 + offset)..=(10 - offset) {
            img.put_pixel(14 + offset, y, Rgba([0x8a, 0x8a, 0x8a, 0xff]));
        }
    }
    encode_png(img)
}

pub fn generated_fluid_tank() -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::from_pixel(18, 54, Rgba([0x22, 0x2a, 0x33, 0xff]));
    for x in 0..18 {
        img.put_pixel(x, 0, Rgba([0xd8, 0xe8, 0xff, 0xff]));
        img.put_pixel(x, 53, Rgba([0x28, 0x32, 0x3c, 0xff]));
    }
    for y in 0..54 {
        img.put_pixel(0, y, Rgba([0xd8, 0xe8, 0xff, 0xff]));
        img.put_pixel(17, y, Rgba([0x28, 0x32, 0x3c, 0xff]));
    }
    encode_png(img)
}

pub fn generated_energy_bar() -> Result<Vec<u8>, String> {
    let mut img = RgbaImage::from_pixel(12, 54, Rgba([0x28, 0x18, 0x18, 0xff]));
    for x in 0..12 {
        img.put_pixel(x, 0, Rgba([0xa8, 0x54, 0x54, 0xff]));
        img.put_pixel(x, 53, Rgba([0x38, 0x10, 0x10, 0xff]));
    }
    for y in 0..54 {
        img.put_pixel(0, y, Rgba([0xa8, 0x54, 0x54, 0xff]));
        img.put_pixel(11, y, Rgba([0x38, 0x10, 0x10, 0xff]));
    }
    encode_png(img)
}
```

- [ ] **Step 3: Add texture generation tests**

Inside the existing `texture::tests` module, add:

```rust
    #[test]
    fn generated_gui_panel_is_deterministic_and_requested_size() {
        let first = generated_gui_panel(176, 166).unwrap();
        let second = generated_gui_panel(176, 166).unwrap();

        assert_eq!(first, second);

        let decoded = image::load_from_memory(&first).unwrap().to_rgba8();
        assert_eq!(decoded.width(), 176);
        assert_eq!(decoded.height(), 166);
    }

    #[test]
    fn generated_slot_is_eighteen_pixels_square() {
        let decoded = image::load_from_memory(&generated_slot().unwrap())
            .unwrap()
            .to_rgba8();

        assert_eq!(decoded.width(), 18);
        assert_eq!(decoded.height(), 18);
        assert_eq!(decoded.get_pixel(0, 0).0, [0x37, 0x37, 0x37, 0xff]);
    }
```

- [ ] **Step 4: Use generated assets when applying templates**

In `src-tauri/src/templates/mod.rs`, update all background texture elements from:

```rust
asset: Some("textures/background.png".into()),
```

to:

```rust
asset: Some(GENERATED_GUI_PANEL.into()),
```

Then update `apply_template`:

```rust
pub fn apply_template(project: &mut Project, template_name: &str) -> Result<(), String> {
    let template =
        get_template(template_name).ok_or_else(|| format!("Unknown template: {template_name}"))?;

    project.gui_size.width = template.default_width;
    project.gui_size.height = template.default_height;
    project.elements = template.elements;
    project.groups.clear();
    project.animations.clear();
    add_generated_template_assets(project)?;
    project.is_dirty = true;

    Ok(())
}
```

- [ ] **Step 5: Run focused Rust tests**

Run: `cargo test templates::tests texture::tests --all-features --locked`

Expected: all focused tests pass.

## Task 3: Fix Template Slot Coordinates

**Files:**
- Modify: `src-tauri/src/templates/mod.rs`

- [ ] **Step 1: Replace 20px grid steps with constants**

Change crafting and chest grid expressions:

```rust
x: 30 + col * SLOT_STEP,
y: 17 + row * SLOT_STEP,
```

```rust
x: 8 + col * SLOT_STEP,
y: 18 + row * SLOT_STEP,
```

For the custom grid section near the bottom of the file, change both the crafting grid and player inventory grid expressions to use `SLOT_STEP`:

```rust
x: 30 + col * SLOT_STEP,
y: 17 + row * SLOT_STEP,
```

```rust
x: 8 + col * SLOT_STEP,
y: 86 + row * SLOT_STEP,
```

- [ ] **Step 2: Keep slot size centralized**

Where a template creates standard slot elements, prefer:

```rust
size: Some(SLOT_SIZE as u32),
```

Do this for loop-generated slots in the templates touched by this task. Manual one-off slots can stay as `Some(18)` in this pass unless the surrounding code is already being edited.

- [ ] **Step 3: Run focused template tests**

Run: `cargo test templates::tests --all-features --locked`

Expected: all template metric tests pass.

## Task 4: Renderer Generated-Asset Fallback

**Files:**
- Modify: `src/lib/engine/renderer.ts`

- [ ] **Step 1: Add generated path constants and path guard**

Near the top of `src/lib/engine/renderer.ts`, add:

```ts
const GENERATED_TEXTURES = new Set([
  "textures/generated/gui_panel.png",
  "textures/generated/slot.png",
  "textures/generated/progress_arrow.png",
  "textures/generated/fluid_tank.png",
  "textures/generated/energy_bar.png",
]);

function isGeneratedTexturePath(path: string | undefined): boolean {
  return path !== undefined && GENERATED_TEXTURES.has(path);
}
```

- [ ] **Step 2: Add procedural drawing fallback**

Add these private methods inside the renderer class near `drawTexture`:

```ts
  private drawGeneratedTextureFallback(el: Element): Container {
    const container = new Container();
    const g = new Graphics();
    const w = el.width ?? 16;
    const h = el.height ?? 16;

    if (el.asset === "textures/generated/gui_panel.png") {
      g.rect(el.x, el.y, w, h);
      g.fill({ color: 0xb8b8b8 });
      g.rect(el.x, el.y, w, 1);
      g.fill({ color: 0xffffff });
      g.rect(el.x, el.y, 1, h);
      g.fill({ color: 0xffffff });
      g.rect(el.x, el.y + h - 1, w, 1);
      g.fill({ color: 0x555555 });
      g.rect(el.x + w - 1, el.y, 1, h);
      g.fill({ color: 0x555555 });
    } else if (el.asset === "textures/generated/slot.png") {
      g.rect(el.x, el.y, w, h);
      g.fill({ color: 0x8b8b8b });
      g.rect(el.x, el.y, w, 1);
      g.fill({ color: 0x373737 });
      g.rect(el.x, el.y, 1, h);
      g.fill({ color: 0x373737 });
      g.rect(el.x, el.y + h - 1, w, 1);
      g.fill({ color: 0xffffff, alpha: 0.8 });
      g.rect(el.x + w - 1, el.y, 1, h);
      g.fill({ color: 0xffffff, alpha: 0.8 });
    } else {
      g.rect(el.x, el.y, w, h);
      g.fill({ color: 0x6f6f6f });
      g.rect(el.x, el.y, w, h);
      g.stroke({ width: 1, color: 0x373737 });
    }

    container.addChild(g);
    return container;
  }
```

- [ ] **Step 3: Use fallback before checkerboard**

In `drawTexture`, after the asset-data lookup block and before the checkerboard placeholder, add:

```ts
    if (isGeneratedTexturePath(el.asset)) {
      return this.drawGeneratedTextureFallback(el);
    }
```

- [ ] **Step 4: Run frontend type check**

Run: `pnpm check`

Expected: no Svelte or TypeScript errors.

## Task 5: Theme Preference and Tokens

**Files:**
- Modify: `src/lib/stores/preferences.svelte.ts`
- Modify: `src/lib/components/PreferencesDialog.svelte`
- Modify: Svelte component styles that use the old core palette values

- [ ] **Step 1: Extend the theme type**

In `src/lib/stores/preferences.svelte.ts`, change:

```ts
theme: "dark" | "high_contrast";
```

to:

```ts
theme: "light" | "dark" | "high_contrast";
```

Change `isTheme` to:

```ts
function isTheme(value: unknown): value is EditorPreferences["theme"] {
  return value === "light" || value === "dark" || value === "high_contrast";
}
```

- [ ] **Step 2: Add the Light option**

In `src/lib/components/PreferencesDialog.svelte`, update the theme selector:

```svelte
<select id="{dialogId}-theme" value={preferences.values.theme} onchange={updateTheme}>
  <option value="dark">Dark</option>
  <option value="light">Light</option>
  <option value="high_contrast">High contrast</option>
</select>
```

- [ ] **Step 3: Add global theme tokens**

Add this global CSS block to the component that currently owns global theme CSS. If `App.svelte` already has global app-shell styles, put the tokens there; otherwise use `PreferencesDialog.svelte` with the existing high-contrast global block:

```css
:global(:root[data-theme="dark"]) {
  --app-bg: #101214;
  --surface: #1f2326;
  --surface-raised: #2d3033;
  --border: #08090a;
  --text: #f2f2f2;
  --muted-text: #b8b8b8;
  --accent: #3aa655;
  --accent-2: #3f76b5;
  --danger: #b83a32;
  --warning: #d7a339;
}

:global(:root[data-theme="light"]) {
  --app-bg: #9f9f9f;
  --surface: #c6c6c6;
  --surface-raised: #d8d8d8;
  --border: #4a4a4a;
  --text: #202020;
  --muted-text: #505050;
  --accent: #2f8f46;
  --accent-2: #3f76b5;
  --danger: #9f3028;
  --warning: #b98525;
}

:global(:root[data-theme="high_contrast"]) {
  --app-bg: #000000;
  --surface: #000000;
  --surface-raised: #111111;
  --border: #ffffff;
  --text: #ffffff;
  --muted-text: #ffffff;
  --accent: #00ffff;
  --accent-2: #ffff00;
  --danger: #ff5555;
  --warning: #ffff00;
}
```

- [ ] **Step 4: Replace core hardcoded colors with tokens**

For each component touched by this task, replace only obvious core palette values:

```css
background: #12121f;
```

with:

```css
background: var(--app-bg);
```

```css
background: #1a1a2e;
```

with:

```css
background: var(--surface);
```

```css
background: #0f3460;
```

with:

```css
background: var(--surface-raised);
```

```css
color: #e94560;
```

with:

```css
color: var(--accent);
```

Use judgement for red error states: map those to `var(--danger)`, not `var(--accent)`.

- [ ] **Step 5: Run frontend verification**

Run: `pnpm check`

Expected: no Svelte or TypeScript errors.

Run: `pnpm build`

Expected: Vite build succeeds. A chunk-size warning is acceptable if it matches the existing Pixi bundle warning.

## Task 6: Documentation and Roadmap Completion

**Files:**
- Modify: `docs/roadmap.md`
- Optionally modify: `README.md` if the theme/default texture behavior is documented there already

- [ ] **Step 1: Mark the roadmap item complete**

In `docs/roadmap.md`, change:

```markdown
- [ ] Minecraft visual fidelity pass: light/dark Minecraft-like themes, generated default GUI textures, and vanilla-aligned template slot metrics
```

to:

```markdown
- [x] Minecraft visual fidelity pass: light/dark Minecraft-like themes, generated default GUI textures, and vanilla-aligned template slot metrics
```

- [ ] **Step 2: Add README note only if there is an existing feature list section**

If `README.md` has a current feature list for templates or editing, add this bullet in that existing list:

```markdown
- Generated Minecraft-like default GUI textures for new templates, with user-imported textures taking precedence.
```

Do not create a new README section just for this change.

## Task 7: Full Verification

**Files:**
- No source edits unless verification reveals a defect.

- [ ] **Step 1: Format Rust**

Run: `cargo fmt --all`

Expected: command exits successfully.

- [ ] **Step 2: Run Rust tests**

Run: `cargo test --all-features --locked`

Expected: all tests pass.

- [ ] **Step 3: Run frontend checks**

Run: `pnpm check`

Expected: no Svelte or TypeScript errors.

- [ ] **Step 4: Build frontend**

Run: `pnpm build`

Expected: Vite build succeeds. Existing chunk-size warning is acceptable.

- [ ] **Step 5: Build Tauri app**

Run: `pnpm tauri build`

Expected: Tauri build succeeds.

- [ ] **Step 6: Inspect final diff**

Run: `git diff -- src-tauri/src/templates/mod.rs src-tauri/src/texture/mod.rs src/lib/stores/preferences.svelte.ts src/lib/components/PreferencesDialog.svelte src/lib/engine/renderer.ts docs/roadmap.md README.md`

Expected: diff is limited to template metrics, generated textures, theme wiring, renderer fallback, and documentation.

## Self-Review

- Spec coverage: themes are covered in Task 5; generated Rust assets are covered in Task 2; renderer fallback is covered in Task 4; template metrics are covered in Tasks 1 and 3; roadmap/docs are covered in Task 6; verification is covered in Task 7.
- Tests: Rust tests cover slot bounds, slot cadence, generated asset insertion, and deterministic PNG generation. Frontend verification covers the theme type and Svelte template changes.
- Risk: the broadest implementation area is replacing hardcoded colors with CSS variables. Keep that scoped to core shell/panel colors and leave unrelated component layout unchanged.
