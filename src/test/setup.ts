import "@testing-library/jest-dom";
import { vi } from "vitest";

// ── Mock @tauri-apps/api/core (invoke) ────────────────────────────────────────
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// ── Mock @tauri-apps/api/event (listen / emit) ────────────────────────────────
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn().mockResolvedValue(undefined),
}));
