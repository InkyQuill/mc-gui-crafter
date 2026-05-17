# ADR 005: Export System

**Date:** 2026-05-17  
**Status:** Accepted

## Context

The `.mcgui` project must export to formats that Minecraft mod developers can directly include in their projects:

1. A composited GUI texture atlas (PNG)
2. A `Screen` class for the chosen mod loader
3. A runtime layout JSON (optional, for dynamic GUIs)

Users choose their mod loader at export time.

## Decision

**Multi-target export pipeline with load-time layout JSON as the primary integration pattern.**

### Export Structure

```
export/
├── textures/gui/
│   └── furnace_gui.png          # Composited texture atlas
├── screen/
│   └── FurnaceScreen.java        # Generated Screen class (Forge/Fabric/NeoForge)
├── data/
│   └── gui/
│       └── furnace_layout.json    # Runtime layout (optional)
└── README.txt                     # Integration instructions
```

### Texture Atlas Compositing

All referenced PNG assets are composited into a single texture sheet. The compositor:

1. Reads each referenced texture from the `.mcgui` archive
2. Places them into the atlas using a bin-packing algorithm (max 256×256 texture for Minecraft compatibility)
3. Records UV coordinates for each element
4. Exports the atlas as a single PNG

If the GUI fits entirely within one texture (which most do), the background IS the atlas and smaller textures (arrow, flame) are composited on top at their correct positions.

### Code Generation Strategy

Rather than generating monolithic Screen classes (which break when users modify them), the default export generates a **layout JSON** and a **lightweight renderer** that reads it:

```java
// Generated: FurnaceScreenRenderer.java
public class FurnaceScreenRenderer {
    private final GuiLayout layout;

    public FurnaceScreenRenderer() {
        this.layout = GuiLayout.load("data/gui/furnace_layout.json");
    }

    public void render(PoseStack poseStack, int guiLeft, int guiTop) {
        layout.render(poseStack, guiLeft, guiTop);
    }

    public void renderProgress(PoseStack poseStack, int guiLeft, int guiTop, float progress) {
        layout.renderAnimation("progress_arrow", poseStack, guiLeft, guiTop, progress);
    }
}
```

The `GuiLayout` runtime class (provided as a small library/dependency) parses the layout JSON and handles all rendering. This means:
- Users can tweak the layout JSON without recompiling
- The library handles all element types (slot, texture, text, progress, etc.)
- Animation bindings are data-driven

### Per-Loader Differences

| Aspect | Forge | Fabric | NeoForge |
|--------|-------|--------|----------|
| PoseStack | `com.mojang.blaze3d.vertex.PoseStack` | Same (Mojang mapping) | Same |
| Screen base | `AbstractContainerScreen<M>` | `HandledScreen<M>` | `AbstractContainerScreen<M>` |
| Texture binding | `RenderSystem.setShaderTexture(0, loc)` | `RenderSystem.setShaderTexture(0, loc)` | Same |
| Resource location | `new ResourceLocation(modid, path)` | `new Identifier(modid, path)` | `ResourceLocation.fromNamespaceAndPath(modid, path)` |

The generated code adapts to the user's choice. The runtime library abstracts these differences with a thin adapter layer.

### Export Options

| Option | Description |
|--------|-------------|
| Loader | forge / fabric / neoforge |
| Mappings | official / yarn / mojang (default: official) |
| Mod ID | User-provided (defaults to project name slug) |
| Package | Java package for generated classes |
| Style | `renderer_only` (lightweight, recommended) or `full_screen` (standalone Screen class) |
| Texture path | Where textures go in the resource structure |

## Consequences

- The runtime `GuiLayout` library must be maintained alongside the editor (available as a standalone dependency)
- Export produces working code immediately usable in a mod project
- The data-driven approach means layout tweaks don't require Java recompilation
- Forge, Fabric, and NeoForge are generated as first-class targets. Loader-specific runtime/screen code is used where APIs differ.
- Resource pack export (Bedrock JSON UI) is a future extension point
