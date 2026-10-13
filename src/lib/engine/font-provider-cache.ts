import type { MinecraftFontProviderRenderData, MinecraftFontRenderData } from "../types";

/** Retain ordered provider matches and misses for each render-data snapshot. */
export class FontProviderCache {
  private fonts = new WeakMap<MinecraftFontRenderData, Map<string, MinecraftFontProviderRenderData | undefined>>();

  get(data: MinecraftFontRenderData, character: string): MinecraftFontProviderRenderData | undefined {
    let characters = this.fonts.get(data);
    if (!characters) {
      characters = new Map();
      this.fonts.set(data, characters);
    }
    if (characters.has(character)) return characters.get(character);

    const provider = data.providers.find(candidate => candidate.chars.some(row => row.includes(character)));
    characters.set(character, provider);
    return provider;
  }
}
