import * as api from "../api";
import type { BrowserTab, EditorLayoutConfig } from "../types";

export const DEFAULT_EDITOR_LAYOUT: EditorLayoutConfig = {
  version: 1,
  right_dock_width: 520,
  properties_width: 300,
  browser_tab: "layers",
};

function clampLayout(layout: Partial<EditorLayoutConfig> | null | undefined): EditorLayoutConfig {
  const right = Math.min(900, Math.max(360, Math.round(layout?.right_dock_width ?? DEFAULT_EDITOR_LAYOUT.right_dock_width)));
  const maxProperties = Math.max(240, right - 160);
  const properties = Math.min(maxProperties, Math.max(240, Math.round(layout?.properties_width ?? DEFAULT_EDITOR_LAYOUT.properties_width)));
  const tab: BrowserTab = layout?.browser_tab === "assets" ? "assets" : "layers";
  return {
    version: 1,
    right_dock_width: right,
    properties_width: properties,
    browser_tab: tab,
  };
}

class LayoutStore {
  values = $state<EditorLayoutConfig>({ ...DEFAULT_EDITOR_LAYOUT });
  loaded = $state(false);
  private saveTimer: number | null = null;

  async load() {
    const config = await api.appConfigGet();
    this.values = clampLayout(config.editor_layout);
    this.loaded = true;
  }

  update(changes: Partial<EditorLayoutConfig>) {
    this.values = clampLayout({ ...this.values, ...changes });
    this.scheduleSave();
  }

  async reset() {
    const config = await api.uiLayoutReset();
    this.values = clampLayout(config.editor_layout);
  }

  private scheduleSave() {
    if (this.saveTimer !== null) window.clearTimeout(this.saveTimer);
    this.saveTimer = window.setTimeout(() => {
      this.saveTimer = null;
      void api.editorLayoutSave(this.values);
    }, 250);
  }
}

export const layout = new LayoutStore();
