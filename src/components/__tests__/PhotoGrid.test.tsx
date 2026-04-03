import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PhotoGrid } from "../PhotoGrid";
import { makePhoto, makeSearchResult } from "../../test/factories";

describe("PhotoGrid", () => {
  const onSelect = vi.fn();

  it("renders empty state when no items", () => {
    render(<PhotoGrid items={[]} onSelect={onSelect} />);
    expect(screen.getByText("No photos here yet")).toBeInTheDocument();
  });

  it("renders a photo grid item", () => {
    const photo = makePhoto({ filename: "beach.jpg" });
    render(<PhotoGrid items={[photo]} onSelect={onSelect} />);
    const img = screen.getByRole("img");
    expect(img).toHaveAttribute("alt", "beach.jpg");
  });

  it("calls onSelect when a photo is clicked", async () => {
    const user = userEvent.setup();
    const photo = makePhoto({ id: "abc123" });
    render(<PhotoGrid items={[photo]} onSelect={onSelect} />);

    await user.click(screen.getByRole("img").closest("div")!);
    expect(onSelect).toHaveBeenCalledWith(photo);
  });

  it("shows video badge for video items", () => {
    const video = makePhoto({ is_video: true, filename: "clip.mp4" });
    render(<PhotoGrid items={[video]} onSelect={onSelect} />);
    expect(screen.getByText("Video")).toBeInTheDocument();
  });

  it("does not show video badge for photos", () => {
    const photo = makePhoto({ is_video: false });
    render(<PhotoGrid items={[photo]} onSelect={onSelect} />);
    expect(screen.queryByText("Video")).not.toBeInTheDocument();
  });

  it("renders search results with score available on hover", () => {
    const result = makeSearchResult({ score: 0.923 });
    const { container } = render(
      <PhotoGrid items={[result]} onSelect={onSelect} isSearchResults={true} />,
    );
    // Score overlay is in the DOM (hidden via opacity, but rendered)
    expect(container.textContent).toContain("92.3% match");
  });

  it("renders multiple photos", () => {
    const photos = [makePhoto({ id: "1" }), makePhoto({ id: "2" }), makePhoto({ id: "3" })];
    render(<PhotoGrid items={photos} onSelect={onSelect} />);
    expect(screen.getAllByRole("img")).toHaveLength(3);
  });
});
