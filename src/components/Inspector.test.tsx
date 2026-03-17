import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { Inspector } from "./Inspector";
import { makePhoto } from "../test/factories";

const mockInvoke = vi.mocked(invoke);

describe("Inspector", () => {
  const onClose = vi.fn();

  beforeEach(() => {
    mockInvoke.mockReset();
    onClose.mockClear();
  });

  it("renders the filename", () => {
    render(<Inspector photo={makePhoto({ filename: "holiday.jpg" })} onClose={onClose} />);
    expect(screen.getByText("holiday.jpg")).toBeInTheDocument();
  });

  it("renders formatted creation date", () => {
    render(
      <Inspector photo={makePhoto({ created_at: "2024-06-15T12:00:00Z" })} onClose={onClose} />,
    );
    expect(screen.getByText(/June 15, 2024/)).toBeInTheDocument();
  });

  it("shows 'Unknown date' when created_at is null", () => {
    render(<Inspector photo={makePhoto({ created_at: null })} onClose={onClose} />);
    expect(screen.getByText("Unknown date")).toBeInTheDocument();
  });

  it("renders dimensions when present", () => {
    render(<Inspector photo={makePhoto({ width: 1920, height: 1080 })} onClose={onClose} />);
    expect(screen.getByText("1920 × 1080")).toBeInTheDocument();
  });

  it("calls onClose when the close button is clicked", async () => {
    const user = userEvent.setup();
    render(<Inspector photo={makePhoto()} onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "" })); // SVG close button
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("shows Indexed status for indexed photos", () => {
    render(<Inspector photo={makePhoto({ indexed: true })} onClose={onClose} />);
    expect(screen.getByText("Indexed")).toBeInTheDocument();
  });

  it("shows Not indexed status for unindexed photos", () => {
    render(<Inspector photo={makePhoto({ indexed: false })} onClose={onClose} />);
    expect(screen.getByText("Not indexed")).toBeInTheDocument();
  });

  it("disables download button for videos", () => {
    render(<Inspector photo={makePhoto({ is_video: true })} onClose={onClose} />);
    expect(screen.getByRole("button", { name: /Save to/i })).toBeDisabled();
  });

  it("shows saved path after successful download", async () => {
    mockInvoke.mockResolvedValueOnce("C:\\Users\\linus\\Pictures\\Flashback\\photo.jpg");
    const user = userEvent.setup();
    render(<Inspector photo={makePhoto({ is_video: false })} onClose={onClose} />);

    await user.click(screen.getByRole("button", { name: /Save to/i }));

    await waitFor(() => {
      expect(
        screen.getByText(/C:\\Users\\linus\\Pictures\\Flashback\\photo.jpg/),
      ).toBeInTheDocument();
    });
  });

  it("shows error message when download fails", async () => {
    mockInvoke.mockRejectedValueOnce("Permission denied");
    const user = userEvent.setup();
    render(<Inspector photo={makePhoto({ is_video: false })} onClose={onClose} />);

    await user.click(screen.getByRole("button", { name: /Save to/i }));

    await waitFor(() => {
      expect(screen.getByText(/Permission denied/)).toBeInTheDocument();
    });
  });
});
