import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import App from "../App";
import { createTauriMock } from "./test_utils";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("Application Shell & Navigation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as any).mockImplementation(createTauriMock());
  });

  it("renders shell with default Dashboard view on startup", async () => {
    render(<App />);

    expect(screen.getByText("TELEVAULT")).toBeDefined();
    expect(screen.getByText("System Dashboard")).toBeDefined();

    await waitFor(() => {
      expect(screen.getByText("Backup Profiles")).toBeDefined();
      expect(screen.getByText("Active Schedules")).toBeDefined();
      expect(screen.getByText("Transfer Queue")).toBeDefined();
      expect(screen.getByText("Total Snapshots")).toBeDefined();
    });
  });

  it("navigates across all primary views via Sidebar links", async () => {
    render(<App />);

    const nav = screen.getByRole("navigation");

    // 1. Backups
    fireEvent.click(within(nav).getByRole("button", { name: /Backups/i }));
    await waitFor(() => {
      expect(screen.getByText("Backup Management")).toBeDefined();
    });

    // 2. Restore
    fireEvent.click(within(nav).getByRole("button", { name: /Restore/i }));
    await waitFor(() => {
      expect(screen.getByText("Disaster Recovery & Restore")).toBeDefined();
    });

    // 3. Schedules
    fireEvent.click(within(nav).getByRole("button", { name: /Schedules/i }));
    await waitFor(() => {
      expect(screen.getByText("Automated Schedules")).toBeDefined();
    });

    // 4. Verification
    fireEvent.click(within(nav).getByRole("button", { name: /Verification/i }));
    await waitFor(() => {
      expect(screen.getByText("Remote Storage Verification")).toBeDefined();
    });

    // 5. Repair
    fireEvent.click(within(nav).getByRole("button", { name: /Repair/i }));
    await waitFor(() => {
      expect(screen.getByText("Remote Repair & Chunk Recovery")).toBeDefined();
    });

    // 6. Activity
    fireEvent.click(within(nav).getByRole("button", { name: /Activity/i }));
    await waitFor(() => {
      expect(screen.getByText("Transfer Activity & Queue")).toBeDefined();
    });

    // 7. Profiles
    fireEvent.click(within(nav).getByRole("button", { name: /Profiles/i }));
    await waitFor(() => {
      expect(screen.getByText("Backup Profiles")).toBeDefined();
    });

    // 8. Retention
    fireEvent.click(within(nav).getByRole("button", { name: /Retention/i }));
    await waitFor(() => {
      expect(screen.getByText("Snapshot Retention Policies")).toBeDefined();
    });

    // 9. Settings
    fireEvent.click(within(nav).getByRole("button", { name: /Settings/i }));
    await waitFor(() => {
      expect(screen.getByText("Application Settings")).toBeDefined();
    });

    // 10. Back to Dashboard
    fireEvent.click(within(nav).getByRole("button", { name: /Dashboard/i }));
    await waitFor(() => {
      expect(screen.getByText("System Dashboard")).toBeDefined();
    });
  });
});
