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

function isTheme(value: unknown): value is EditorPreferences["theme"] {
  return value === "dark" || value === "high_contrast";
}

function numberOrDefault(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function booleanOrDefault(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function normalizePreferences(value: Partial<EditorPreferences>): EditorPreferences {
  return {
    showGrid: booleanOrDefault(value.showGrid, defaults.showGrid),
    snapToGrid: booleanOrDefault(value.snapToGrid, defaults.snapToGrid),
    majorGridSize: numberOrDefault(value.majorGridSize, defaults.majorGridSize),
    minorGridSize: numberOrDefault(value.minorGridSize, defaults.minorGridSize),
    snapSize: numberOrDefault(value.snapSize, defaults.snapSize),
    defaultPreset: typeof value.defaultPreset === "string" ? value.defaultPreset : defaults.defaultPreset,
    theme: isTheme(value.theme) ? value.theme : defaults.theme,
  };
}

function readStoredPreferences(): Partial<EditorPreferences> {
  if (typeof localStorage === "undefined") return {};

  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

function persistPreferences(values: EditorPreferences) {
  if (typeof localStorage === "undefined") return;

  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(values));
  } catch {
    // Preferences are UI-only; storage failures should not break editing.
  }
}

function loadPreferences(): EditorPreferences {
  return normalizePreferences(readStoredPreferences());
}

class PreferencesStore {
  values = $state<EditorPreferences>(loadPreferences());

  update(changes: Partial<EditorPreferences>) {
    this.values = normalizePreferences({ ...this.values, ...changes });
    persistPreferences(this.values);
  }

  reset() {
    this.values = { ...defaults };
    persistPreferences(this.values);
  }
}

export const preferences = new PreferencesStore();
