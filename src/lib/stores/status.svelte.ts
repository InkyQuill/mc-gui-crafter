export type StatusType = "success" | "warning" | "error" | "info";

import { appendSessionLog } from "../api";

export interface StatusMessage {
  id: number;
  type: StatusType;
  text: string;
}

class StatusStore {
  current = $state<StatusMessage | null>(null);
  private timeout: ReturnType<typeof setTimeout> | null = null;
  private nextId = 1;

  show(type: StatusType, text: string, timeoutMs = 4000) {
    this.clearTimer();
    this.current = {
      id: this.nextId++,
      type,
      text,
    };
    void appendSessionLog({
      level: type === "error" ? "error" : type === "warning" ? "warning" : "info",
      source: "ui",
      category: "status",
      message: text,
      details: { status_type: type },
    });

    if (timeoutMs > 0) {
      this.timeout = setTimeout(() => {
        this.clear();
      }, timeoutMs);
    }
  }

  success(text: string, timeoutMs?: number) {
    this.show("success", text, timeoutMs);
  }

  warning(text: string, timeoutMs?: number) {
    this.show("warning", text, timeoutMs);
  }

  error(text: string, timeoutMs = 7000) {
    this.show("error", text, timeoutMs);
  }

  info(text: string, timeoutMs?: number) {
    this.show("info", text, timeoutMs);
  }

  clear() {
    this.clearTimer();
    this.current = null;
  }

  private clearTimer() {
    if (this.timeout) {
      clearTimeout(this.timeout);
      this.timeout = null;
    }
  }
}

export function readableError(error: unknown): string {
  return error instanceof Error ? error.message : String(error || "Unknown error");
}

export const status = new StatusStore();
