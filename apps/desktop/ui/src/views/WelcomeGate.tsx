import { useState, useEffect } from "react";
import {
  commands,
  TelegramAuthStatusDto,
  TelegramApiConfigStatusDto,
  StartTelegramAuthDto,
  SubmitAuthCodeDto,
  SubmitAuthPasswordDto,
  SetupChannelDto,
} from "../bindings";

interface WelcomeGateProps {
  onAuthenticated: () => void;
  authStatus: TelegramAuthStatusDto | null;
}

export function WelcomeGate({ onAuthenticated, authStatus }: WelcomeGateProps) {
  const [phoneNumber, setPhoneNumber] = useState("");
  const [authCode, setAuthCode] = useState("");
  const [password, setPassword] = useState("");
  const [manualChannelId, setManualChannelId] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showManualChannel, setShowManualChannel] = useState(false);
  const [apiId, setApiId] = useState("");
  const [apiHash, setApiHash] = useState("");
  const [apiConfig, setApiConfig] = useState<TelegramApiConfigStatusDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const state = authStatus?.state || "authentication_required";
  const account = authStatus?.account;

  useEffect(() => {
    commands.getTelegramApiConfig().then((res) => {
      if (res.status === "ok") {
        setApiConfig(res.data);
        if (res.data.is_configured && res.data.api_id) {
          setApiId(res.data.api_id.toString());
        } else if (!res.data.is_configured) {
          setShowAdvanced(true);
        }
      }
    }).catch(() => {});
  }, []);

  const handleStartAuth = async (e: React.FormEvent) => {
    e.preventDefault();
    if (loading) return;

    const trimmedPhone = phoneNumber.trim();
    if (!trimmedPhone) {
      setError("Please enter your phone number with country code (e.g. +1 555 123 4567)");
      return;
    }
    const cleanDigits = trimmedPhone.replace(/[^\d]/g, "");
    if (!trimmedPhone.startsWith("+") || cleanDigits.length < 8) {
      setError("Please enter a valid international phone number starting with '+' and your country code (e.g. +1 555 123 4567)");
      return;
    }

    try {
      setLoading(true);
      setError(null);
      const parsedApiId = apiId.trim() ? parseInt(apiId.trim(), 10) : null;
      if (apiId.trim() && (isNaN(parsedApiId!) || parsedApiId! <= 0)) {
        setError("API ID must be a positive integer.");
        setLoading(false);
        return;
      }

      const payload: StartTelegramAuthDto = {
        phone_number: trimmedPhone,
        api_id: parsedApiId,
        api_hash: apiHash.trim() ? apiHash.trim() : null,
      };
      const res = await commands.startTelegramAuth(payload);
      if (res.status === "ok") {
        if (res.data.error_message) {
          setError(res.data.error_message);
          if (res.data.error_message.includes("API ID and API Hash")) {
            setShowAdvanced(true);
          }
        } else {
          onAuthenticated();
        }
      } else {
        setError(res.error.message || "Failed to start Telegram authentication");
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleSubmitCode = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!authCode.trim()) {
      setError("Please enter the verification code sent to your Telegram app");
      return;
    }
    try {
      setLoading(true);
      setError(null);
      const payload: SubmitAuthCodeDto = {
        code: authCode.trim(),
      };
      const res = await commands.submitTelegramAuthCode(payload);
      if (res.status === "ok") {
        onAuthenticated();
      } else {
        setError(res.error.message || "Invalid or expired verification code");
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleSubmitPassword = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!password) {
      setError("Please enter your Telegram Two-Step Verification cloud password");
      return;
    }
    try {
      setLoading(true);
      setError(null);
      const payload: SubmitAuthPasswordDto = {
        password,
      };
      const res = await commands.submitTelegramAuthPassword(payload);
      if (res.status === "ok") {
        onAuthenticated();
      } else {
        setError(res.error.message || "Incorrect Two-Step Verification password");
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleCancelAuth = async () => {
    try {
      setLoading(true);
      await commands.cancelTelegramAuth();
      setAuthCode("");
      setPassword("");
      setError(null);
      onAuthenticated();
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleAutoSetupChannel = async () => {
    try {
      setLoading(true);
      setError(null);
      const payload: SetupChannelDto = {
        channel_id: null,
      };
      const res = await commands.setupBackupChannel(payload);
      if (res.status === "ok") {
        onAuthenticated();
      } else {
        setError(res.error.message || "Failed to create dedicated backup channel");
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleManualSetupChannel = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!manualChannelId.trim()) {
      setError("Please enter your private Telegram channel ID (e.g. -1001234567890)");
      return;
    }
    const parsed = parseInt(manualChannelId.trim(), 10);
    if (isNaN(parsed)) {
      setError("Channel ID must be a valid integer");
      return;
    }
    try {
      setLoading(true);
      setError(null);
      const payload: SetupChannelDto = {
        channel_id: parsed,
      };
      const res = await commands.setupBackupChannel(payload);
      if (res.status === "ok") {
        onAuthenticated();
      } else {
        setError(res.error.message || "Channel verification failed. Make sure TELEVAULT has post permissions.");
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="welcome-gate-screen">
      <div className="welcome-gate-container">
        {/* Brand Header */}
        <div className="welcome-header">
          <div className="welcome-logo-badge">
            <span className="welcome-logo-icon">🛡️</span>
          </div>
          <h1 className="welcome-title">TELEVAULT</h1>
          <p className="welcome-subtitle">Secure Telegram Cloud Backup</p>
          <div className="welcome-tagline">
            Your files. Your Telegram. Your control.
          </div>
        </div>

        {/* Step Indicator */}
        <div className="gate-steps-indicator">
          <div
            className={`gate-step-pill ${
              state === "authentication_required" || state === "initializing"
                ? "active"
                : "completed"
            }`}
          >
            1. Phone
          </div>
          <div
            className={`gate-step-pill ${
              state === "authenticating" ? "active" : state === "channel_setup_required" || state === "ready" ? "completed" : ""
            }`}
          >
            2. Code
          </div>
          {authStatus?.requires_password && (
            <div
              className={`gate-step-pill ${
                state === "authenticating" && authStatus.requires_password ? "active" : "completed"
              }`}
            >
              3. 2FA Password
            </div>
          )}
          <div
            className={`gate-step-pill ${
              state === "channel_setup_required" || state === "channel_setup_in_progress" || state === "channel_verification_failed"
                ? "active"
                : state === "ready"
                ? "completed"
                : ""
            }`}
          >
            {authStatus?.requires_password ? "4. Vault Channel" : "3. Vault Channel"}
          </div>
        </div>

        {/* Error Alert */}
        {error && (
          <div className="gate-error-banner" role="alert">
            <span className="gate-error-icon">⚠️</span>
            <div className="gate-error-text">{error}</div>
            <button
              type="button"
              className="gate-error-dismiss"
              onClick={() => setError(null)}
              aria-label="Dismiss error"
            >
              ✕
            </button>
          </div>
        )}

        {/* Card Content based on Step */}
        <div className="welcome-card">
          {/* STEP 1: Phone input */}
          {(state === "authentication_required" || state === "initializing" || state === "authentication_failed") && (
            <form onSubmit={handleStartAuth} className="gate-form">
              <div className="gate-form-header">
                <h2>Connect Your Telegram Account</h2>
                <p>
                  Authenticate directly with Telegram using the secure MTProto protocol.
                  No bot tokens or third-party servers required.
                </p>
              </div>

              <div className="form-group">
                <label htmlFor="gate-phone-input">Phone Number (with country code):</label>
                <input
                  id="gate-phone-input"
                  type="tel"
                  className="input-text"
                  placeholder="+1 555 123 4567"
                  value={phoneNumber}
                  onChange={(e) => setPhoneNumber(e.target.value)}
                  disabled={loading}
                  autoFocus
                />
                <span className="form-help-text">
                  Telegram will send an official verification code directly to your Telegram app.
                </span>
              </div>

              <div className="advanced-toggle">
                <button
                  type="button"
                  className="link-btn"
                  onClick={() => setShowAdvanced(!showAdvanced)}
                >
                  {showAdvanced
                    ? "▼ Hide Telegram API Credentials"
                    : apiConfig?.is_configured
                    ? `✓ Telegram API Credentials Configured (App ID: ${apiConfig.api_id}) • Edit`
                    : "⚠️ Custom Telegram API Credentials Required (Click to configure)"}
                </button>
              </div>

              {showAdvanced && (
                <div className="advanced-fields-box">
                  <div
                    className="credentials-help-box"
                    style={{
                      fontSize: "0.82rem",
                      color: "#94a3b8",
                      marginBottom: "0.75rem",
                      lineHeight: 1.45,
                      background: "rgba(15, 23, 42, 0.5)",
                      padding: "0.6rem 0.8rem",
                      borderRadius: "6px",
                      border: "1px solid rgba(148, 163, 184, 0.15)",
                    }}
                  >
                    Personal MTProto authentication requires credentials from Telegram:
                    <br />
                    1. Sign in to{" "}
                    <a
                      href="https://my.telegram.org"
                      target="_blank"
                      rel="noreferrer"
                      style={{ color: "#60a5fa", textDecoration: "underline" }}
                    >
                      https://my.telegram.org
                    </a>{" "}
                    with your phone number.
                    <br />
                    2. Go to <strong>&quot;API development tools&quot;</strong> and create an application.
                    <br />
                    3. Copy your <strong>App api_id</strong> and <strong>App api_hash</strong> below.
                  </div>
                  <div className="form-group">
                    <label htmlFor="gate-api-id">Telegram API ID:</label>
                    <input
                      id="gate-api-id"
                      type="text"
                      className="input-text mono"
                      placeholder="e.g. 1234567"
                      value={apiId}
                      onChange={(e) => setApiId(e.target.value)}
                      disabled={loading}
                    />
                  </div>
                  <div className="form-group">
                    <label htmlFor="gate-api-hash">Telegram API Hash:</label>
                    <input
                      id="gate-api-hash"
                      type="password"
                      className="input-text mono"
                      placeholder="32-character hex hash"
                      value={apiHash}
                      onChange={(e) => setApiHash(e.target.value)}
                      disabled={loading}
                    />
                  </div>
                </div>
              )}

              <button
                type="submit"
                className="btn btn-primary btn-block"
                disabled={loading}
              >
                {loading ? "Connecting to Telegram..." : "Send Verification Code ➜"}
              </button>
            </form>
          )}

          {/* STEP 2: Verification Code */}
          {state === "authenticating" && !authStatus?.requires_password && (
            <form onSubmit={handleSubmitCode} className="gate-form">
              <div className="gate-form-header">
                <h2>Enter Verification Code</h2>
                <p>
                  Telegram has sent an official login code{phoneNumber ? ` for ${phoneNumber}` : ""}.
                </p>
                <div
                  className="delivery-notice-box"
                  style={{
                    background: "rgba(59, 130, 246, 0.08)",
                    border: "1px solid rgba(59, 130, 246, 0.25)",
                    borderRadius: "8px",
                    padding: "0.75rem 1rem",
                    marginTop: "0.75rem",
                    fontSize: "0.85rem",
                    lineHeight: 1.45,
                    color: "#cbd5e1",
                    textAlign: "left",
                  }}
                >
                  <div style={{ fontWeight: 600, color: "#60a5fa", marginBottom: "0.35rem" }}>
                    📲 How Telegram delivers your verification code:
                  </div>
                  <div>
                    • <strong>Telegram App (Active Sessions):</strong> If you are logged into Telegram on another phone, computer, or web session, Telegram delivers the code inside the official <em>Telegram</em> service notifications chat.
                  </div>
                  <div style={{ marginTop: "0.3rem" }}>
                    • <strong>SMS text message:</strong> Telegram will only send an SMS if you do not have any active Telegram sessions logged in.
                  </div>
                </div>
              </div>

              <div className="form-group" style={{ marginTop: "1.25rem" }}>
                <label htmlFor="gate-code-input">Telegram Verification Code:</label>
                <input
                  id="gate-code-input"
                  type="text"
                  className="input-text mono text-center"
                  placeholder="12345"
                  value={authCode}
                  onChange={(e) => setAuthCode(e.target.value)}
                  disabled={loading}
                  autoFocus
                  maxLength={10}
                />
              </div>

              <div className="gate-actions-row">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={handleCancelAuth}
                  disabled={loading}
                >
                  ← Change Phone / Re-request
                </button>
                <button
                  type="submit"
                  className="btn btn-primary"
                  disabled={loading}
                >
                  {loading ? "Verifying..." : "Verify Code ➜"}
                </button>
              </div>
            </form>
          )}

          {/* STEP 3: 2FA Password */}
          {state === "authenticating" && authStatus?.requires_password && (
            <form onSubmit={handleSubmitPassword} className="gate-form">
              <div className="gate-form-header">
                <h2>Two-Step Verification (2FA)</h2>
                <p>
                  Your Telegram account is protected by an additional cloud password. Enter it below to complete sign-in.
                </p>
              </div>

              <div className="form-group">
                <label htmlFor="gate-password-input">Cloud Password:</label>
                <input
                  id="gate-password-input"
                  type="password"
                  className="input-text"
                  placeholder="Enter your 2FA password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={loading}
                  autoFocus
                />
              </div>

              <div className="gate-actions-row">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={handleCancelAuth}
                  disabled={loading}
                >
                  ← Cancel
                </button>
                <button
                  type="submit"
                  className="btn btn-primary"
                  disabled={loading}
                >
                  {loading ? "Unlocking Account..." : "Confirm Password ➜"}
                </button>
              </div>
            </form>
          )}

          {/* STEP 4: Dedicated Channel Setup */}
          {(state === "channel_setup_required" ||
            state === "channel_setup_in_progress" ||
            state === "channel_verification_failed") && (
            <div className="gate-form">
              <div className="gate-form-header">
                <h2>Set Up Your Dedicated Backup Channel</h2>
                {account && (
                  <div className="account-welcome-badge">
                    <span className="badge-avatar">👤</span>
                    <span>
                      Signed in as <strong>{account.first_name} {account.last_name || ""}</strong> ({account.phone_number})
                    </span>
                  </div>
                )}
                <p>
                  TELEVAULT stores encrypted, chunked backup files in a dedicated private channel.
                  Your backups remain completely private, accessible only to your account.
                </p>
              </div>

              {/* Primary choice: Automatic setup */}
              <div className="channel-choice-card">
                <div className="choice-header">
                  <span className="choice-icon">⚡</span>
                  <div>
                    <div className="choice-title">Automatic Setup (Recommended)</div>
                    <div className="choice-subtitle">
                      TELEVAULT will automatically create a private, dedicated channel named{" "}
                      <strong>&quot;TELEVAULT Backup Vault&quot;</strong> and verify write access.
                    </div>
                  </div>
                </div>
                <button
                  type="button"
                  className="btn btn-primary btn-block"
                  onClick={handleAutoSetupChannel}
                  disabled={loading}
                >
                  {loading ? "Creating & Verifying Channel..." : "✨ Create Dedicated Vault Channel"}
                </button>
              </div>

              {/* Secondary choice: Existing manual channel */}
              <div className="manual-channel-box">
                <button
                  type="button"
                  className="link-btn"
                  onClick={() => setShowManualChannel(!showManualChannel)}
                >
                  {showManualChannel ? "▼ Hide Manual Channel Setup" : "▶ Use an Existing Private Channel"}
                </button>

                {showManualChannel && (
                  <form onSubmit={handleManualSetupChannel} style={{ marginTop: "0.75rem" }}>
                    <div className="form-group">
                      <label htmlFor="gate-channel-id">Channel ID (e.g. -1001234567890):</label>
                      <input
                        id="gate-channel-id"
                        type="text"
                        className="input-text mono"
                        placeholder="-1001234567890"
                        value={manualChannelId}
                        onChange={(e) => setManualChannelId(e.target.value)}
                        disabled={loading}
                      />
                      <span className="form-help-text">
                        Make sure your account is an Administrator with permission to post and read messages in this channel.
                      </span>
                    </div>
                    <button
                      type="submit"
                      className="btn btn-secondary btn-block"
                      disabled={loading}
                    >
                      {loading ? "Verifying Channel..." : "Verify & Connect Existing Channel"}
                    </button>
                  </form>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Welcome Footer with Creator Attribution */}
        <div className="welcome-footer">
          <div className="welcome-attribution">
            Created by Ankit Sharma
          </div>
          <div className="welcome-security-guarantee">
            Direct Telegram MTProto • Client-Side AES-256-GCM Encryption • Zero Third-Party Servers
          </div>
        </div>
      </div>
    </div>
  );
}
