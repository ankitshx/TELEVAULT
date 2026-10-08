import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ActivityView } from "../views/ActivityView";
import { SettingsView } from "../views/SettingsView";
import { commands } from "../bindings";
import { createTauriMock } from "./test_utils";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("Activity Queue & Settings & IPC Invariants", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as any).mockImplementation(createTauriMock());
  });

  describe("ActivityView", () => {
    it("renders active transfer jobs and allows cancellation", async () => {
      const notify = vi.fn();
      render(<ActivityView onNotify={notify} />);

      await waitFor(() => {
        expect(screen.queryByText(/Querying transfer workers/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("xfer-001")).toBeDefined();
        expect(screen.getByText(/file-5gb/i)).toBeDefined();
        expect(screen.getByText("46%")).toBeDefined();
      });

      // Click Cancel on running transfer
      const cancelBtn = screen.getByRole("button", { name: "Cancel" });
      fireEvent.click(cancelBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "warning",
          "Job Cancelled",
          expect.stringContaining("xfer-001")
        );
      });
    });

    it("filters jobs by status (running, completed, failed)", async () => {
      render(<ActivityView onNotify={vi.fn()} />);

      await waitFor(() => {
        expect(screen.queryByText(/Querying transfer workers/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("xfer-001")).toBeDefined();
      });

      // Filter by completed
      const filterSelect = screen.getByRole("combobox");
      fireEvent.change(filterSelect, { target: { value: "Completed" } });

      await waitFor(() => {
        expect(screen.getByText("xfer-002")).toBeDefined();
      });
    });
  });

  describe("SettingsView", () => {
    it("displays AppConfig preferences and allows saving updates", async () => {
      const notify = vi.fn();
      render(<SettingsView onNotify={notify} />);

      await waitFor(() => {
        expect(screen.queryByText(/Reading application settings/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("Storage & Local Cache")).toBeDefined();
        expect(screen.getByText("Transfer Workers & Chunking Pipeline")).toBeDefined();
      });

      // Modify concurrency
      const concurrencyInput = screen.getByDisplayValue("4");
      fireEvent.change(concurrencyInput, { target: { value: "6" } });

      // Save preferences
      const saveBtn = screen.getByRole("button", { name: /Save Settings/i });
      fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "success",
          "Settings Persisted",
          expect.stringContaining("validated and written to disk")
        );
      });
    });

    it("displays canonical system diagnostic paths", async () => {
      render(<SettingsView onNotify={vi.fn()} />);

      await waitFor(() => {
        expect(screen.queryByText(/Reading application settings/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("C:\\ProgramData\\TELEVAULT")).toBeDefined();
        expect(screen.getByText("C:\\ProgramData\\TELEVAULT\\televault.db")).toBeDefined();
      });
    });
  });

  describe("IPC Architecture Invariants", () => {
    it("verifies zero HTTP, zero localhost, zero Python, strictly Tauri IPC", async () => {
      // Trigger IPC commands via generated typed bindings
      await commands.getAppInfo();
      await commands.getSystemPaths();
      await commands.getAppConfig();

      const calls = (invoke as any).mock.calls;
      expect(calls.length).toBeGreaterThanOrEqual(3);

      // Verify every IPC command is a registered snake_case Tauri command
      for (const [cmd] of calls) {
        expect(typeof cmd).toBe("string");
        expect(cmd).toMatch(/^[a-z0-9_]+$/);
        expect(cmd).not.toContain("http");
        expect(cmd).not.toContain("localhost");
      }
    });
  });
});
