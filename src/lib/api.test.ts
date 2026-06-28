import { describe, expect, it } from "vitest";
import type { Element } from "./types";
import {
  assetImport,
  elementAdd,
  elementUpdateMany,
  projectExportPreview,
  projectGetActive,
  projectNew,
  projectSummary,
  projectUndo,
} from "./api";

function slot(id: string, x: number): Element {
  return {
    id,
    type: "slot",
    x,
    y: 18,
    size: 18,
    visible: true,
  };
}

describe("mock elementUpdateMany", () => {
  it("updates multiple elements in one undoable revision", async () => {
    const project = await projectNew("Mock Batch Update", 176, 166, "forge");
    await elementAdd(slot("a", 8), project.project_id);
    await elementAdd(slot("b", 26), project.project_id);

    const before = await projectSummary(project.project_id);
    const updated = await elementUpdateMany([
      { id: "a", changes: { x: 10, y: 20 } },
      { id: "b", changes: { x: 30, y: 40, slot_index: 7 } },
    ], project.project_id);

    expect(updated.map(element => [element.id, element.x, element.y, element.slot_index])).toEqual([
      ["a", 10, 20, undefined],
      ["b", 30, 40, 7],
    ]);
    const after = await projectSummary(project.project_id);
    expect(after.revision).toBe(before.revision + 1);

    await projectUndo(project.project_id);
    const active = await projectGetActive();
    expect(active.project.elements.find(element => element.id === "a")?.x).toBe(8);
    expect(active.project.elements.find(element => element.id === "b")?.slot_index).toBeUndefined();
  });

  it("rejects invalid batches atomically", async () => {
    const project = await projectNew("Mock Batch Atomic", 176, 166, "forge");
    await elementAdd(slot("a", 8), project.project_id);

    const before = await projectSummary(project.project_id);
    await expect(elementUpdateMany([
      { id: "a", changes: { x: 10 } },
      { id: "missing", changes: { x: 30 } },
    ], project.project_id)).rejects.toBe("Element not found: missing");

    const active = await projectGetActive();
    expect(active.project.elements.find(element => element.id === "a")?.x).toBe(8);
    expect((await projectSummary(project.project_id)).revision).toBe(before.revision);
  });

  it("clears nullable fields with null and treats undefined as absent", async () => {
    const project = await projectNew("Mock Batch Validation", 176, 166, "forge");
    await elementAdd(slot("a", 8), project.project_id);

    await elementUpdateMany([
      { id: "a", changes: { width: 32, content: "Label", slot_index: 3 } },
    ], project.project_id);
    await elementUpdateMany([
      { id: "a", changes: { width: null, content: null, slot_index: null } },
    ], project.project_id);
    let active = await projectGetActive();
    let element = active.project.elements.find(item => item.id === "a");
    expect(element?.width).toBeUndefined();
    expect(element?.content).toBeUndefined();
    expect(element?.slot_index).toBeUndefined();

    await elementUpdateMany([
      { id: "a", changes: { width: 48, content: "Keep" } },
    ], project.project_id);
    const before = await projectSummary(project.project_id);
    await elementUpdateMany([
      { id: "a", changes: { width: undefined, content: undefined } },
    ], project.project_id);
    active = await projectGetActive();
    element = active.project.elements.find(item => item.id === "a");
    expect(element?.width).toBe(48);
    expect(element?.content).toBe("Keep");
    expect((await projectSummary(project.project_id)).revision).toBe(before.revision);
  });

  it("rejects identity fields and preserves no-op history", async () => {
    const project = await projectNew("Mock Batch Identity Validation", 176, 166, "forge");
    await elementAdd(slot("a", 8), project.project_id);

    const before = await projectSummary(project.project_id);
    await expect(elementUpdateMany([
      { id: "a", changes: { id: "b" } as never },
    ], project.project_id)).rejects.toBe("Invalid element update: id is not a mutable field");

    expect((await projectSummary(project.project_id)).revision).toBe(before.revision);
    await expect(elementUpdateMany([
      { id: "a", changes: { x: 8, y: 18, visible: true } },
    ], project.project_id)).resolves.toHaveLength(1);
    expect((await projectSummary(project.project_id)).revision).toBe(before.revision);
  });
});

describe("mock export preview", () => {
  it("includes visible element assets in textures-only previews", async () => {
    const project = await projectNew("Mock Textures Only Assets", 176, 166, "forge");
    const slotAsset = await assetImport(
      "/tmp/custom_slot.png",
      project.project_id,
      "data:image/png;base64,iVBORw0KGgo=",
    );
    const backgroundAsset = await assetImport(
      "/tmp/custom_background.png",
      project.project_id,
      "data:image/png;base64,iVBORw0KGgo=",
    );
    await elementAdd({
      ...slot("slot_with_asset", 8),
      asset: slotAsset.name,
    }, project.project_id);
    await elementAdd({
      id: "background_with_asset",
      type: "texture",
      x: 0,
      y: 0,
      width: 176,
      height: 166,
      visible: true,
      asset: backgroundAsset.name,
    }, project.project_id);

    const preview = await projectExportPreview(
      "forge",
      "testmod",
      "com.example",
      "TextureOnlyGui",
      "/tmp/mcgui-textures-only",
      project.project_id,
      {
        codegen_mode: "simple",
        generate_runtime_helpers: true,
        generate_semantic_registry: false,
        export_scope: "textures_only",
      },
    );

    expect(preview.files).toContain("/tmp/mcgui-textures-only/src/main/resources/assets/testmod/textures/custom_slot.png");
    expect(preview.files).toContain("/tmp/mcgui-textures-only/src/main/resources/assets/testmod/textures/custom_background.png");
  });
});
