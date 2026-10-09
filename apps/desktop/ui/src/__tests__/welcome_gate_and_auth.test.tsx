import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import App from "../App";
import { WelcomeGate } from "../views/WelcomeGate";
import { createTauriMock } from "./test_utils";
import type { TelegramAuthStatusDto } from "../bindings";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("Phase 20: Welcome Gate & Telegram MTProto Auth Flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders Welcome Gate with creator attribution when unauthenticated", async () => {
    const customMock = createTauriMock({
      get_telegram_auth_status: () => ({
        state: "authentication_required",
        account: null,
        channel: null,
        requires_password: false,
        error_message: null,
      }),
    });
    (invoke as any).mockImplementation(customMock);

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText("Connect Your Telegram Account")).toBeDefined();
      expect(screen.getByText("Created by Ankit Sharma")).toBeDefined();
      expect(screen.getByText("Your files. Your Telegram. Your control.")).toBeDefined();
    });

    // Sidebar should NOT be rendered while locked
    expect(screen.queryByText("System Dashboard")).toBeNull();
  });

  it("handles multi-step authentication flow from phone to channel setup", async () => {
    const onAuth = vi.fn();
    const initialStatus: TelegramAuthStatusDto = {
      state: "authentication_required",
      account: null,
      channel: null,
      requires_password: false,
      error_message: null,
    };

    (invoke as any).mockImplementation(
      createTauriMock({
        start_telegram_auth: () => ({
          state: "authenticating",
          account: null,
          channel: null,
          requires_password: false,
          error_message: null,
        }),
      })
    );

    const { rerender } = render(
      <WelcomeGate onAuthenticated={onAuth} authStatus={initialStatus} />
    );

    // 1. Submit phone number
    const phoneInput = screen.getByLabelText(/Phone Number/i);
    fireEvent.change(phoneInput, { target: { value: "+1 555 123 4567" } });

    const submitBtn = screen.getByRole("button", { name: /Send Verification Code/i });
    fireEvent.click(submitBtn);

    await waitFor(() => {
      expect(onAuth).toHaveBeenCalled();
    });

    // 2. Rerender in Authenticating state (Code input)
    const codeStatus: TelegramAuthStatusDto = {
      state: "authenticating",
      account: null,
      channel: null,
      requires_password: false,
      error_message: null,
    };

    (invoke as any).mockImplementation(
      createTauriMock({
        submit_telegram_auth_code: () => ({
          state: "channel_setup_required",
          account: {
            user_id: 987654321,
            first_name: "Ankit",
            last_name: "Sharma",
            username: "ankitshx",
            phone_number: "+1 555 *** 4567",
          },
          channel: null,
          requires_password: false,
          error_message: null,
        }),
      })
    );

    rerender(<WelcomeGate onAuthenticated={onAuth} authStatus={codeStatus} />);

    expect(screen.getByText("Enter Verification Code")).toBeDefined();
    const codeInput = screen.getByLabelText(/Telegram Verification Code/i);
    fireEvent.change(codeInput, { target: { value: "12345" } });

    const verifyCodeBtn = screen.getByRole("button", { name: /Verify Code/i });
    fireEvent.click(verifyCodeBtn);

    await waitFor(() => {
      expect(onAuth).toHaveBeenCalledTimes(2);
    });

    // 3. Rerender in Channel Setup state
    const channelStatus: TelegramAuthStatusDto = {
      state: "channel_setup_required",
      account: {
        user_id: 987654321,
        first_name: "Ankit",
        last_name: "Sharma",
        username: "ankitshx",
        phone_number: "+1 555 *** 4567",
      },
      channel: null,
      requires_password: false,
      error_message: null,
    };

    (invoke as any).mockImplementation(
      createTauriMock({
        setup_backup_channel: () => ({
          state: "ready",
          account: channelStatus.account,
          channel: {
            channel_id: -1001234567890,
            channel_title: "TELEVAULT Backup Vault",
            is_private: true,
            verified: true,
            created_by_televault: true,
          },
          requires_password: false,
          error_message: null,
        }),
      })
    );

    rerender(<WelcomeGate onAuthenticated={onAuth} authStatus={channelStatus} />);

    expect(screen.getByText("Set Up Your Dedicated Backup Channel")).toBeDefined();
    expect(screen.getAllByText(/Ankit/i).length).toBeGreaterThanOrEqual(1);

    const createChannelBtn = screen.getByRole("button", { name: /Create Dedicated Vault Channel/i });
    fireEvent.click(createChannelBtn);

    await waitFor(() => {
      expect(onAuth).toHaveBeenCalledTimes(3);
    });
  });

  it("renders startup splash screen while authentication check is pending and never flashes dashboard", () => {
    // Delay resolution indefinitely during this assertion
    let resolveAuth: any;
    const pendingPromise = new Promise((resolve) => {
      resolveAuth = resolve;
    });

    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "get_telegram_auth_status") return pendingPromise;
      return Promise.resolve({ status: "ok", data: {} });
    });

    render(<App />);

    // While pending, splash screen must be rendered
    expect(screen.getByTestId("app-splash")).toBeDefined();
    expect(screen.getByText("Initializing secure Telegram vault environment...")).toBeDefined();
    expect(screen.getByTestId("splash-spinner")).toBeDefined();

    // The protected dashboard must NOT be rendered during pending
    expect(screen.queryByText("System Dashboard")).toBeNull();
    expect(screen.queryByTestId("welcome-gate-container")).toBeNull();

    // Clean up promise
    resolveAuth({
      status: "ok",
      data: {
        state: "authentication_required",
        account: null,
        channel: null,
        requires_password: false,
        error_message: null,
      },
    });
  });

  it("renders safe error screen with retry button when authentication check fails", async () => {
    let callCount = 0;
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "get_telegram_auth_status") {
        callCount++;
        if (callCount === 1) {
          return Promise.reject(new Error("Core IPC connection failure"));
        }
        return Promise.resolve({
          status: "ok",
          data: {
            state: "authentication_required",
            account: null,
            channel: null,
            requires_password: false,
            error_message: null,
          },
        });
      }
      return Promise.resolve({ status: "ok", data: {} });
    });

    render(<App />);

    // 1. Error screen is shown, protected content is never exposed
    await waitFor(() => {
      expect(screen.getByTestId("app-splash-error")).toBeDefined();
      expect(screen.getByText("Initialization Failed")).toBeDefined();
      expect(screen.getByText(/Core IPC connection failure/i)).toBeDefined();
    });
    expect(screen.queryByText("System Dashboard")).toBeNull();

    // 2. Click retry button
    const retryBtn = screen.getByRole("button", { name: /Retry Connection/i });
    fireEvent.click(retryBtn);

    // 3. After retry succeeds, transitions to WelcomeGate
    await waitFor(() => {
      expect(screen.getByText("Connect Your Telegram Account")).toBeDefined();
    });
    expect(screen.queryByTestId("app-splash-error")).toBeNull();
    expect(screen.queryByText("System Dashboard")).toBeNull();
  });

  it("renders authenticated dashboard only when auth state is ready", async () => {
    (invoke as any).mockImplementation(
      createTauriMock({
        get_telegram_auth_status: () => ({
          state: "ready",
          account: {
            user_id: 123456,
            first_name: "Ankit",
            last_name: "Sharma",
            username: "ankitshx",
            phone_number: "+1 555 *** 1234",
          },
          channel: {
            channel_id: -100987654321,
            channel_title: "TELEVAULT Backup Vault",
            is_private: true,
            verified: true,
            created_by_televault: true,
          },
          requires_password: false,
          error_message: null,
        }),
      })
    );

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText("System Dashboard")).toBeDefined();
    });

    // WelcomeGate and Splash must not be visible
    expect(screen.queryByTestId("app-splash")).toBeNull();
    expect(screen.queryByTestId("welcome-gate-container")).toBeNull();
  });
});
