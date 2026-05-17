export interface GuiPreset {
  id: "furnace" | "chest_9x3" | "chest_9x6" | "hopper" | "custom";
  label: string;
  width: number;
  height: number;
}

export type GuiPresetId = GuiPreset["id"];

export const guiPresets: GuiPreset[] = [
  { id: "furnace", label: "Furnace / Inventory", width: 176, height: 166 },
  { id: "chest_9x3", label: "Chest 9x3", width: 176, height: 166 },
  { id: "chest_9x6", label: "Chest 9x6", width: 176, height: 222 },
  { id: "hopper", label: "Hopper", width: 176, height: 133 },
  { id: "custom", label: "Custom", width: 176, height: 166 },
];

export function getGuiPreset(id: string): GuiPreset | undefined {
  return guiPresets.find((preset) => preset.id === id);
}
