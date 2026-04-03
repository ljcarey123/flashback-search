import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SearchBar } from "../SearchBar";

describe("SearchBar", () => {
  const onSearch = vi.fn();

  beforeEach(() => {
    onSearch.mockClear();
  });

  it("renders the search input", () => {
    render(<SearchBar onSearch={onSearch} isSearching={false} />);
    expect(screen.getByRole("textbox")).toBeInTheDocument();
  });

  it("calls onSearch with trimmed query on submit", async () => {
    const user = userEvent.setup();
    render(<SearchBar onSearch={onSearch} isSearching={false} />);

    await user.type(screen.getByRole("textbox"), "  me at the beach  ");
    await user.keyboard("{Enter}");

    expect(onSearch).toHaveBeenCalledOnce();
    expect(onSearch).toHaveBeenCalledWith("me at the beach");
  });

  it("does not call onSearch for an empty query", async () => {
    const user = userEvent.setup();
    render(<SearchBar onSearch={onSearch} isSearching={false} />);

    await user.keyboard("{Enter}");
    expect(onSearch).not.toHaveBeenCalled();
  });

  it("does not call onSearch for a whitespace-only query", async () => {
    const user = userEvent.setup();
    render(<SearchBar onSearch={onSearch} isSearching={false} />);

    await user.type(screen.getByRole("textbox"), "   ");
    await user.keyboard("{Enter}");
    expect(onSearch).not.toHaveBeenCalled();
  });

  it("shows a spinner when isSearching is true", () => {
    const { container } = render(<SearchBar onSearch={onSearch} isSearching={true} />);
    // Spinner div uses animate-spin
    expect(container.querySelector(".animate-spin")).toBeInTheDocument();
  });

  it("hides spinner when isSearching is false", () => {
    const { container } = render(<SearchBar onSearch={onSearch} isSearching={false} />);
    expect(container.querySelector(".animate-spin")).not.toBeInTheDocument();
  });
});
