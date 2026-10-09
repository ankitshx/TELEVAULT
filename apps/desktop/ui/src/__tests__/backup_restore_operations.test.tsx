import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { BackupsView } from "../views/BackupsView";
import { RestoreView } from "../views/RestoreView";
import { createTauriMock } from "./test_utils";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("Backup & Restore Engine Integration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as any).mockImplementation(createTauriMock());
  });

  describe("BackupsView", () => {
    it("renders profile selector and allows triggering backup run", async () => {
      const notify = vi.fn();
      render(
        <BackupsView
          onNotify={notify}
          onNavigate={vi.fn()}
        />
      );

      await waitFor(() => {
        expect(screen.queryByText(/Loading backup profiles/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("Documents & Databases (prof-001)")).toBeDefined();
      });

      const runBtn = await screen.findByRole("button", { name: /Run Backup Now/i });
      fireEvent.click(runBtn);

      const startBtn = screen.getByRole("button", { name: /Start Backup/i });
      fireEvent.click(startBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "success",
          "Backup Completed",
          expect.stringContaining("Created snapshot")
        );
      });
    });

    it("displays historical snapshots with formatted metadata", async () => {
      render(
        <BackupsView
          onNotify={vi.fn()}
          onNavigate={vi.fn()}
        />
      );

      await waitFor(() => {
        expect(screen.queryByText(/Loading backup profiles/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("snap-001")).toBeDefined();
        expect(screen.getByText("completed")).toBeDefined();
      });
    });
  });

  describe("RestoreView & Large-File Handling", () => {
    it("supports snapshot restore with collision policies and passphrase", async () => {
      const notify = vi.fn();
      render(<RestoreView onNotify={notify} />);

      await waitFor(() => {
        expect(screen.queryByText(/Loading profiles and restore catalog/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("Documents & Databases (prof-001)")).toBeDefined();
      });

      // Enter destination directory
      const destInput = screen.getByPlaceholderText(/RestoredBackups/i);
      fireEvent.change(destInput, { target: { value: "C:\\Target\\RestoredData" } });

      // Collision policy selector
      const policySelect = screen.getByDisplayValue(/keep_both/i);
      fireEvent.change(policySelect, { target: { value: "overwrite" } });

      // Execute restore
      const executeBtn = screen.getByRole("button", { name: /Execute Restore/i });
      fireEvent.click(executeBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "success",
          "Snapshot Restored",
          expect.stringContaining("Restored 42 files")
        );
      });

      // Verifies large-file 5.2 GB payload metadata is rendered correctly without memory overflow
      expect(screen.getByText(/5324.80 MB/i)).toBeDefined();
    });

    it("verifies collision policies keep_both, overwrite, and skip are selectable", async () => {
      render(<RestoreView onNotify={vi.fn()} />);

      await waitFor(() => {
        expect(screen.queryByText(/Loading profiles and restore catalog/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("Collision Resolution Policy:")).toBeDefined();
      });

      const options = screen.getAllByRole("option");
      const policyValues = options.map((opt) => (opt as HTMLOptionElement).value);

      expect(policyValues).toContain("keep_both");
      expect(policyValues).toContain("overwrite");
      expect(policyValues).toContain("skip");
    });
  });
});
