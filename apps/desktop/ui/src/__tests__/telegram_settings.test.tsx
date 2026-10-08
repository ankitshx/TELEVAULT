import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SettingsView } from "../views/SettingsView";
import { createTauriMock } from "./test_utils";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("Telegram Cloud Storage UI Integration & Credential Isolation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as any).mockImplementation(createTauriMock());
  });

  it("renders Telegram integration section with connected badge and backend", async () => {
    render(<SettingsView onNotify={vi.fn()} />);

    await waitFor(() => {
      expect(screen.queryByText(/Reading application settings/i)).toBeNull();
    });

    await waitFor(() => {
      expect(screen.getByText("Telegram Cloud Storage Integration")).toBeDefined();
      expect(screen.getByText(/🟢 Connected/i)).toBeDefined();
      expect(screen.getByText("telegram-cloud-storage")).toBeDefined();
      expect(screen.getByText("@TeleVaultMockBot")).toBeDefined();
    });
  });

  it("saves Telegram credentials and immediately redacts the token from the input", async () => {
    const notify = vi.fn();
    render(<SettingsView onNotify={notify} />);

    await waitFor(() => {
      expect(screen.getByText("Telegram Cloud Storage Integration")).toBeDefined();
    });

    const tokenInput = screen.getByPlaceholderText(/••••••••••••••••/i) as HTMLInputElement;
    const chatInput = screen.getByPlaceholderText("-1001234567890") as HTMLInputElement;

    // Type sensitive token and chat ID
    fireEvent.change(tokenInput, { target: { value: "987654321:SecretLiveTelegramBotTokenABC" } });
    fireEvent.change(chatInput, { target: { value: "-100987654321" } });

    expect(tokenInput.value).toBe("987654321:SecretLiveTelegramBotTokenABC");

    // Click Save
    const saveBtn = screen.getByRole("button", { name: /Save Telegram Settings/i });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(notify).toHaveBeenCalledWith(
        "success",
        "Telegram Configured",
        expect.stringContaining("Credentials stored in secure application config")
      );
    });

    // Credential Isolation Invariant: Raw secret token must be purged from input state immediately
    expect(tokenInput.value).toBe("");
  });

  it("triggers connection test and displays verified bot identity", async () => {
    const notify = vi.fn();
    render(<SettingsView onNotify={notify} />);

    await waitFor(() => {
      expect(screen.getByText("Telegram Cloud Storage Integration")).toBeDefined();
    });

    const testBtn = screen.getByRole("button", { name: /Test Connection/i });
    fireEvent.click(testBtn);

    await waitFor(() => {
      expect(notify).toHaveBeenCalledWith(
        "success",
        "Connection Verified",
        expect.stringContaining("Telegram Bot API is fully accessible")
      );
      expect(screen.getByText(/Connected successfully as @TeleVaultMockBot/i)).toBeDefined();
    });
  });

  it("displays sanitized error when connection test fails without leaking secrets", async () => {
    (invoke as any).mockImplementation(
      createTauriMock({
        test_telegram_connection: () => ({
          success: false,
          bot_username: null,
          bot_id: null,
          chat_title: null,
          error_message: "Telegram API error 401: Unauthorized",
        }),
      })
    );

    render(<SettingsView onNotify={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Telegram Cloud Storage Integration")).toBeDefined();
    });

    const testBtn = screen.getByRole("button", { name: /Test Connection/i });
    fireEvent.click(testBtn);

    await waitFor(() => {
      expect(screen.getByText(/Telegram API error 401: Unauthorized/i)).toBeDefined();
      // Ensure no raw secret tokens are present anywhere in the DOM
      expect(screen.queryByText(/SecretLiveTelegramBotToken/i)).toBeNull();
    });
  });

  it("disconnects Telegram and reverts active backend to mock storage", async () => {
    const notify = vi.fn();
    render(<SettingsView onNotify={notify} />);

    await waitFor(() => {
      expect(screen.getByText("Telegram Cloud Storage Integration")).toBeDefined();
    });

    const disconnectBtn = screen.getByRole("button", { name: /Disconnect Telegram/i });
    fireEvent.click(disconnectBtn);

    await waitFor(() => {
      expect(notify).toHaveBeenCalledWith(
        "info",
        "Telegram Disconnected",
        expect.stringContaining("Credentials cleared from local disk")
      );
      expect(screen.getByText(/⚪ Not Configured/i)).toBeDefined();
    });
  });
});
