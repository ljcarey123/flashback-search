import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { makePhoto, makeSearchResult, makeAuthStatus, makeStats } from "./test/factories";

const mockInvoke = vi.mocked(invoke);

function defaultInvokes() {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "get_auth_status") return Promise.resolve(makeAuthStatus());
    if (cmd === "get_library") return Promise.resolve([]);
    if (cmd === "get_stats")
      return Promise.resolve(makeStats({ total: 0, photos: 0, indexed: 0, videos: 0 }));
    if (cmd === "load_settings") return Promise.resolve({ has_gemini_key: false, client_id: null });
    if (cmd === "get_db_path") return Promise.resolve("C:\\AppData\\flashback.db");
    return Promise.resolve(null);
  });
}

describe("App", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    defaultInvokes();
  });

  it("renders the app logo and title", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText("Flashback")).toBeInTheDocument();
    });
  });

  it("renders the search bar", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByRole("textbox")).toBeInTheDocument();
    });
  });

  it("shows library nav item as active by default", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "library" })).toBeInTheDocument();
    });
  });

  it("shows 'Connect Google Photos' prompt when unauthenticated", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText(/Connect Google Photos/)).toBeInTheDocument();
    });
  });

  it("does not show 'Connect Google Photos' when authenticated", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_auth_status")
        return Promise.resolve(makeAuthStatus({ authenticated: true, user_name: "Ada" }));
      if (cmd === "get_library") return Promise.resolve([]);
      if (cmd === "get_stats") return Promise.resolve(makeStats());
      return Promise.resolve(null);
    });
    render(<App />);
    await waitFor(() => {
      expect(screen.queryByText(/Connect Google Photos/)).not.toBeInTheDocument();
    });
  });

  it("renders photos from the library", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_auth_status") return Promise.resolve(makeAuthStatus());
      if (cmd === "get_library")
        return Promise.resolve([makePhoto({ id: "p1", filename: "beach.jpg" })]);
      if (cmd === "get_stats") return Promise.resolve(makeStats());
      return Promise.resolve(null);
    });
    render(<App />);
    await waitFor(() => {
      expect(screen.getByAltText("beach.jpg")).toBeInTheDocument();
    });
  });

  it("navigates to settings view", async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => screen.getByRole("button", { name: "settings" }));
    await user.click(screen.getByRole("button", { name: "settings" }));
    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    });
  });

  it("shows search results after a search", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_auth_status") return Promise.resolve(makeAuthStatus());
      if (cmd === "get_library") return Promise.resolve([]);
      if (cmd === "get_stats")
        return Promise.resolve(makeStats({ total: 0, photos: 0, indexed: 0, videos: 0 }));
      if (cmd === "search")
        return Promise.resolve([
          makeSearchResult({ photo: makePhoto({ filename: "sunset.jpg" }) }),
        ]);
      return Promise.resolve(null);
    });
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => screen.getByRole("textbox"));

    await user.type(screen.getByRole("textbox"), "sunset");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(screen.getByAltText("sunset.jpg")).toBeInTheDocument();
    });
  });

  it("shows inspector when a photo is clicked", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_auth_status") return Promise.resolve(makeAuthStatus());
      if (cmd === "get_library") return Promise.resolve([makePhoto({ filename: "wedding.jpg" })]);
      if (cmd === "get_stats") return Promise.resolve(makeStats());
      return Promise.resolve(null);
    });
    const user = userEvent.setup();
    render(<App />);

    const img = await screen.findByAltText("wedding.jpg");
    await user.click(img.closest("div")!);

    await waitFor(() => {
      expect(screen.getByText("Inspector")).toBeInTheDocument();
    });
  });

  it("closes inspector when close button is clicked", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_auth_status") return Promise.resolve(makeAuthStatus());
      if (cmd === "get_library") return Promise.resolve([makePhoto({ filename: "picnic.jpg" })]);
      if (cmd === "get_stats") return Promise.resolve(makeStats());
      return Promise.resolve(null);
    });
    const user = userEvent.setup();
    render(<App />);

    const img = await screen.findByAltText("picnic.jpg");
    await user.click(img.closest("div")!);
    await waitFor(() => screen.getByText("Inspector"));

    // Click the SVG close button (first button in the inspector header)
    const closeBtn = screen.getAllByRole("button").find((b) => b.closest("aside"));
    await user.click(closeBtn!);

    await waitFor(() => {
      expect(screen.queryByText("Inspector")).not.toBeInTheDocument();
    });
  });
});
