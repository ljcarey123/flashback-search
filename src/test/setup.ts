import "@testing-library/jest-dom";
import { vi } from "vitest";

// ── Mock @tauri-apps/api/core (invoke + convertFileSrc) ───────────────────────
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  // Return path unchanged so img src attributes are testable
  convertFileSrc: vi.fn((path: string) => `asset://localhost${path}`),
}));

// ── Mock @tauri-apps/api/event (listen / emit) ────────────────────────────────
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn().mockResolvedValue(undefined),
}));

// ── Mock @tauri-apps/plugin-dialog (open folder picker) ──────────────────────
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue(null),
}));
