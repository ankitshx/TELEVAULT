import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { DashboardView } from "../views/DashboardView";
import { ProfilesView } from "../views/ProfilesView";
import { createTauriMock, mockProfiles } from "./test_utils";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("Dashboard & Profile Management", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as any).mockImplementation(createTauriMock());
  });

  describe("DashboardView", () => {
    it("fetches and displays real-time telemetry metrics", async () => {
      const handleNavigate = vi.fn();
      const handleStartBackup = vi.fn();

      render(
        <DashboardView
          onNavigate={handleNavigate}
          onStartBackup={handleStartBackup}
        />
      );

      await waitFor(() => {
        expect(screen.getByText("Backup Profiles")).toBeDefined();
        expect(screen.getByText("Active Schedules")).toBeDefined();
        expect(screen.getByText("Transfer Queue")).toBeDefined();
        expect(screen.getByText("Total Snapshots")).toBeDefined();
      });

      // Quick action triggers
      const backupBtn = screen.getByRole("button", { name: /Run Backup Now/i });
      fireEvent.click(backupBtn);
      expect(handleStartBackup).toHaveBeenCalledWith(mockProfiles[0]);

      const restoreBtn = screen.getByRole("button", { name: /Restore Files/i });
      fireEvent.click(restoreBtn);
      expect(handleNavigate).toHaveBeenCalledWith("restore");
    });
  });

  describe("ProfilesView", () => {
    it("lists backup profiles and allows creating a new profile", async () => {
      const notify = vi.fn();

      render(
        <ProfilesView
          onNotify={notify}
          onStartBackup={vi.fn()}
          onNavigate={vi.fn()}
        />
      );

      await waitFor(() => {
        expect(screen.getByText("Documents & Databases")).toBeDefined();
        expect(screen.getByText("Media Archives")).toBeDefined();
      });

      // Open Create Profile modal
      const newBtn = screen.getByRole("button", { name: /Create New Profile/i });
      fireEvent.click(newBtn);

      expect(screen.getByText(/Create New Backup Profile/i)).toBeDefined();

      // Submit new profile
      const nameInput = screen.getByPlaceholderText("e.g. Work Documents");
      const pathInput = screen.getByPlaceholderText(/Important/i);

      fireEvent.change(nameInput, { target: { value: "Project Alpha" } });
      fireEvent.change(pathInput, { target: { value: "D:\\Alpha" } });

      const saveBtn = screen.getByRole("button", { name: /Create Profile/i });
      fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "success",
          "Profile Created",
          expect.stringContaining("Project Alpha")
        );
      });
    });

    it("allows editing an existing profile", async () => {
      const notify = vi.fn();

      render(
        <ProfilesView
          onNotify={notify}
          onStartBackup={vi.fn()}
          onNavigate={vi.fn()}
        />
      );

      await waitFor(() => {
        expect(screen.getByText("Documents & Databases")).toBeDefined();
      });

      // Click Edit on the first profile
      const editBtns = screen.getAllByRole("button", { name: "Edit" });
      fireEvent.click(editBtns[0]);

      expect(screen.getByText(/Edit Profile:/i)).toBeDefined();

      const nameInput = screen.getByDisplayValue("Documents & Databases");
      fireEvent.change(nameInput, { target: { value: "Updated Documents" } });

      const updateBtn = screen.getByRole("button", { name: /Save Changes/i });
      fireEvent.click(updateBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "success",
          "Profile Updated",
          expect.stringContaining("Updated Documents")
        );
      });
    });
  });
});
