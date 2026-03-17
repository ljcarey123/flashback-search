import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { SettingsPage } from "./SettingsPage";
import { makeAuthStatus, makeStats } from "../test/factories";

const mockInvoke = vi.mocked(invoke);

function defaultInvokes() {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "load_settings") return Promise.resolve({ has_gemini_key: false, client_id: null });
    if (cmd === "get_stats")
      return Promise.resolve(makeStats({ total: 0, photos: 0, indexed: 0, videos: 0 }));
    if (cmd === "get_db_path") return Promise.resolve("C:\\AppData\\flashback.db");
    return Promise.resolve(null);
  });
}

describe("SettingsPage", () => {
  const onAuthChange = vi.fn();

  beforeEach(() => {
    mockInvoke.mockReset();
    onAuthChange.mockClear();
    defaultInvokes();
  });

  it("renders page heading", async () => {
    render(<SettingsPage authStatus={makeAuthStatus()} onAuthChange={onAuthChange} />);
    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
  });

  it("shows DB path in index health panel when stats load", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "load_settings")
        return Promise.resolve({ has_gemini_key: false, client_id: null });
      if (cmd === "get_stats") return Promise.resolve(makeStats());
      if (cmd === "get_db_path") return Promise.resolve("C:\\AppData\\flashback.db");
      return Promise.resolve(null);
    });
    render(<SettingsPage authStatus={makeAuthStatus()} onAuthChange={onAuthChange} />);
    await waitFor(() => {
      expect(screen.getByText(/C:\\AppData\\flashback.db/)).toBeInTheDocument();
    });
  });

  it("shows 'signed in' UI when authenticated", async () => {
    render(
      <SettingsPage
        authStatus={makeAuthStatus({ authenticated: true, user_name: "Ada Lovelace" })}
        onAuthChange={onAuthChange}
      />,
    );
    await waitFor(() => {
      expect(screen.getByText(/Ada Lovelace/)).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: "Sync Library" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeInTheDocument();
  });

  it("shows OAuth form when not authenticated", async () => {
    render(<SettingsPage authStatus={makeAuthStatus()} onAuthChange={onAuthChange} />);
    await waitFor(() => {
      expect(screen.getByPlaceholderText("Client ID")).toBeInTheDocument();
    });
    expect(screen.getByPlaceholderText("Client Secret")).toBeInTheDocument();
  });

  it("disables 'Open Google Sign-In' when credentials are empty", async () => {
    render(<SettingsPage authStatus={makeAuthStatus()} onAuthChange={onAuthChange} />);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Open Google Sign-In" })).toBeDisabled();
    });
  });

  it("enables 'Open Google Sign-In' once both fields are filled", async () => {
    const user = userEvent.setup();
    render(<SettingsPage authStatus={makeAuthStatus()} onAuthChange={onAuthChange} />);

    await user.type(await screen.findByPlaceholderText("Client ID"), "my-client-id");
    await user.type(screen.getByPlaceholderText("Client Secret"), "my-secret");

    expect(screen.getByRole("button", { name: "Open Google Sign-In" })).toBeEnabled();
  });

  it("shows key-saved indicator when Gemini key is present in keychain", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "load_settings")
        return Promise.resolve({ has_gemini_key: true, client_id: null });
      if (cmd === "get_stats")
        return Promise.resolve(makeStats({ total: 0, photos: 0, indexed: 0, videos: 0 }));
      if (cmd === "get_db_path") return Promise.resolve("C:\\AppData\\flashback.db");
      return Promise.resolve(null);
    });
    render(<SettingsPage authStatus={makeAuthStatus()} onAuthChange={onAuthChange} />);
    await waitFor(() => {
      expect(screen.getByText("API key saved in OS keychain")).toBeInTheDocument();
    });
  });

  it("shows success message after saving Gemini key", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "load_settings")
        return Promise.resolve({ has_gemini_key: false, client_id: null });
      if (cmd === "get_stats")
        return Promise.resolve(makeStats({ total: 0, photos: 0, indexed: 0, videos: 0 }));
      if (cmd === "get_db_path") return Promise.resolve("C:\\AppData\\flashback.db");
      if (cmd === "save_settings") return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<SettingsPage authStatus={makeAuthStatus()} onAuthChange={onAuthChange} />);

    await user.type(await screen.findByPlaceholderText("Gemini API key"), "AIza-test-key");
    await user.click(screen.getByRole("button", { name: "Save Key" }));

    await waitFor(() => {
      expect(screen.getByText(/Gemini key saved securely/)).toBeInTheDocument();
    });
  });

  it("shows sync error when sync_library fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "load_settings")
        return Promise.resolve({ has_gemini_key: false, client_id: null });
      if (cmd === "get_stats")
        return Promise.resolve(makeStats({ total: 0, photos: 0, indexed: 0, videos: 0 }));
      if (cmd === "get_db_path") return Promise.resolve("C:\\AppData\\flashback.db");
      if (cmd === "sync_library") return Promise.reject("Network error");
      return Promise.resolve(null);
    });
    const user = userEvent.setup();
    render(
      <SettingsPage
        authStatus={makeAuthStatus({ authenticated: true, user_name: "Ada" })}
        onAuthChange={onAuthChange}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "Sync Library" }));

    await waitFor(() => {
      expect(screen.getByText(/Network error/)).toBeInTheDocument();
    });
  });
});
