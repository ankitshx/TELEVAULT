import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { VerificationView } from "../views/VerificationView";
import { RepairView } from "../views/RepairView";
import { createTauriMock } from "./test_utils";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("Verification & Remote Repair Workflows", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as any).mockImplementation(createTauriMock());
  });

  describe("VerificationView", () => {
    it("runs Level 1 Metadata Verification and displays report", async () => {
      const notify = vi.fn();
      render(<VerificationView onNotify={notify} onNavigate={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText("Documents & Databases (prof-001)")).toBeDefined();
      });

      const auditBtn = screen.getByRole("button", { name: /Run Verification Audit/i });
      fireEvent.click(auditBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "success",
          "Verification Completed",
          expect.stringContaining("Status: healthy")
        );
      });
    });

    it("presents all four verification levels (Metadata, Availability, Integrity, Restore Readiness)", async () => {
      render(<VerificationView onNotify={vi.fn()} onNavigate={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/Level 1: Metadata Only/i)).toBeDefined();
        expect(screen.getByText(/Level 2: Remote Availability/i)).toBeDefined();
        expect(screen.getByText(/Level 3: Remote Integrity/i)).toBeDefined();
        expect(screen.getByText(/Level 4: Restore Readiness/i)).toBeDefined();
      });
    });
  });

  describe("RepairView", () => {
    it("previews repair candidates and identifies repair eligibility", async () => {
      const notify = vi.fn();
      render(<RepairView onNotify={notify} />);

      await waitFor(() => {
        expect(screen.getByText("Documents & Databases (prof-001)")).toBeDefined();
      });

      const previewBtn = screen.getByRole("button", { name: /Preview Repair/i });
      fireEvent.click(previewBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "info",
          "Repair Analysis Complete",
          expect.stringContaining("Found 1 chunks needing recovery")
        );
      });

      expect(screen.getByText(/Candidate Items Breakdown:/i)).toBeDefined();
    });

    it("executes remote chunk repair and reports repaired chunks count", async () => {
      const notify = vi.fn();
      render(<RepairView onNotify={notify} />);

      await waitFor(() => {
        expect(screen.getByText("Documents & Databases (prof-001)")).toBeDefined();
      });

      // Preview first
      const previewBtn = screen.getByRole("button", { name: /Preview Repair/i });
      fireEvent.click(previewBtn);

      await waitFor(() => {
        expect(screen.getByText(/Candidate Items Breakdown:/i)).toBeDefined();
      });

      // Execute repair
      const execBtn = screen.getByRole("button", { name: /Execute Remote Repair/i });
      fireEvent.click(execBtn);

      await waitFor(() => {
        expect(notify).toHaveBeenCalledWith(
          "success",
          "Remote Repair Completed",
          expect.stringContaining("Repaired 1 chunks")
        );
      });
    });

    it("displays auditable historical repair logs", async () => {
      render(<RepairView onNotify={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText("REMOTE_CHUNK_MISSING")).toBeDefined();
        expect(screen.getByText("SUCCESS")).toBeDefined();
      });
    });
  });
});
