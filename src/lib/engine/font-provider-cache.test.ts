import { describe, expect, it, vi } from "vitest";
import type { MinecraftFontProviderRenderData, MinecraftFontRenderData } from "../types";
import { FontProviderCache } from "./font-provider-cache";

function provider(file: string, chars: string[]): MinecraftFontProviderRenderData {
  return { file, chars, ascent: 7, height: 8, image_width: 16, image_height: 8, image_data_url: "" };
}

function font(providers: MinecraftFontProviderRenderData[]): MinecraftFontRenderData {
  return { id: "test", source_type: "minecraft", providers, glyph_map: {} };
}

describe("font provider lookup", () => {
  it("preserves first-provider precedence and caches Unicode matches and misses", () => {
    const first = provider("first", ["A😀"]);
    const data = font([first, provider("second", ["AB😀"])]);
    const scan = vi.spyOn(data.providers, "find");
    const cache = new FontProviderCache();

    for (let frame = 0; frame < 3; frame++) {
      expect(cache.get(data, "A")).toBe(first);
      expect(cache.get(data, "😀")).toBe(first);
      expect(cache.get(data, "B")).toBe(data.providers[1]);
      expect(cache.get(data, "missing")).toBeUndefined();
    }
    expect(scan).toHaveBeenCalledTimes(4);
  });

  it("does not reuse hits or misses after replacing render data with the same font id", () => {
    const cache = new FontProviderCache();
    const oldData = font([provider("old", ["A"])]);
    expect(cache.get(oldData, "A")).toBe(oldData.providers[0]);
    expect(cache.get(oldData, "B")).toBeUndefined();

    const newData = font([provider("new", ["AB"])]);
    expect(cache.get(newData, "A")).toBe(newData.providers[0]);
    expect(cache.get(newData, "B")).toBe(newData.providers[0]);
  });
});
