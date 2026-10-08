import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SchedulesView } from "../views/SchedulesView";
import { RetentionView } from "../views/RetentionView";
import { createTauriMock } from "./test_utils";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("Scheduler & Retention Integration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as any).mockImplementation(createTauriMock());
    window.confirm = vi.fn().mockReturnValue(true);
  });

  describe("SchedulesView", () => {
    it("displays configured schedules and enables running on-demand", async () => {
      const notify = vi.fn();
      render(<SchedulesView onNotify={notify} />);

      await waitFor(() => {
        expect(screen.queryByText(/Loading schedules/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText(/sched-001/i)).toBeDefined();
        expect(screen.getByText("0 2 * * *")).toBeDefined();
      });

      const runNowBtn = screen.getByRole("button", { name: /Run Now/i });
      fireEvent.click(runNowBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "success",
          "Execution Dispatched",
          expect.stringContaining("Scheduled job completed or running")
        );
      });
    });

    it("allows toggling schedule active state", async () => {
      const notify = vi.fn();
      render(<SchedulesView onNotify={notify} />);

      await waitFor(() => {
        expect(screen.queryByText(/Loading schedules/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText(/sched-001/i)).toBeDefined();
      });

      const disableBtn = screen.getByRole("button", { name: "Disable" });
      fireEvent.click(disableBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "info",
          "Schedule Disabled",
          expect.stringContaining("sched-001")
        );
      });
    });
  });

  describe("RetentionView", () => {
    it("allows previewing dry-run retention decisions before pruning", async () => {
      const notify = vi.fn();
      render(<RetentionView onNotify={notify} />);

      await waitFor(() => {
        expect(screen.queryByText(/Loading profiles/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("Documents & Databases")).toBeDefined();
      });

      // Click Preview / Dry-Run
      const previewBtn = screen.getByRole("button", { name: /Preview Dry-Run/i });
      fireEvent.click(previewBtn);

      await waitFor(() => {
        expect(screen.getByText(/Retention Evaluation \(Dry-Run Preview\)/i)).toBeDefined();
        expect(screen.getByText("KEEP_LATEST")).toBeDefined();
        expect(screen.getByText("PRUNE_EXCESS_SNAPSHOT")).toBeDefined();
      });
    });

    it("executes safe local pruning preserving immutable Telegram storage objects", async () => {
      const notify = vi.fn();
      render(<RetentionView onNotify={notify} />);

      await waitFor(() => {
        expect(screen.queryByText(/Loading profiles/i)).toBeNull();
      });

      await waitFor(() => {
        expect(screen.getByText("Documents & Databases")).toBeDefined();
      });

      // Preview first to populate prunable items
      const previewBtn = screen.getByRole("button", { name: /Preview Dry-Run/i });
      fireEvent.click(previewBtn);

      await waitFor(() => {
        expect(screen.getByText(/Retention Evaluation \(Dry-Run Preview\)/i)).toBeDefined();
      });

      // Click Execute Retention in the modal
      const executeBtn = screen.getByRole("button", { name: /Execute Pruning/i });
      fireEvent.click(executeBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "success",
          "Retention Pruned",
          expect.stringContaining("Pruned 1 local snapshot records (5 versions unlinked).")
        );
      });
    });
  });
});
