// Integration smoke — the React component layer. Renders the app shell's
// sidebar (no IPC required) and asserts the brand + navigation are present.
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import { Sidebar } from "@/components/Sidebar";

describe("Sidebar", () => {
  it("renders the brand and main navigation", () => {
    render(
      <Sidebar
        current="dashboard"
        onNavigate={vi.fn()}
        onNewDocument={vi.fn()}
      />,
    );
    expect(screen.getByText("SundayPaper")).toBeInTheDocument();
    expect(screen.getByText("Bibliotek")).toBeInTheDocument();
    expect(screen.getByText("Bygger")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Nytt dokument/ }),
    ).toBeInTheDocument();
  });
});
