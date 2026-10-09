import { useEffect, useState, useCallback } from "react";
import {
  commands,
  AppConfigDto,
  AppInfoDto,
  SystemPathsDto,
  TelegramStatusDto,
  SaveTelegramConfigDto,
  TelegramAuthStatusDto,
} from "../bindings";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";

interface SettingsViewProps {
  onNotify: (type: "success" | "error" | "warning" | "info", title: string, message: string) => void;
}

export function SettingsView({ onNotify }: SettingsViewProps) {
  const [config, setConfig] = useState<AppConfigDto | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfoDto | null>(null);
  const [paths, setPaths] = useState<SystemPathsDto | null>(null);
  const [telegramStatus, setTelegramStatus] = useState<TelegramStatusDto | null>(null);
  const [authStatus, setAuthStatus] = useState<TelegramAuthStatusDto | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [saving, setSaving] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [verifyingChannel, setVerifyingChannel] = useState<boolean>(false);
  const [loggingOut, setLoggingOut] = useState<boolean>(false);
  const [showLogoutConfirm, setShowLogoutConfirm] = useState<boolean>(false);

  // Telegram Configuration Form Fields
  const [botToken, setBotToken] = useState<string>("");
  const [targetChatId, setTargetChatId] = useState<string>("");
  const [apiEndpoint, setApiEndpoint] = useState<string>("");
  const [testingConnection, setTestingConnection] = useState<boolean>(false);
  const [savingTelegram, setSavingTelegram] = useState<boolean>(false);
  const [telegramNotice, setTelegramNotice] = useState<{ type: "success" | "error" | "info"; msg: string } | null>(null);

  // General Form Fields
  const [theme, setTheme] = useState<string>("dark");
  const [language, setLanguage] = useState<string>("en-US");
  const [checkUpdates, setCheckUpdates] = useState<boolean>(true);

  const [maxCacheSizeMb, setMaxCacheSizeMb] = useState<number>(2048);
  const [tempRetentionHours, setTempRetentionHours] = useState<number>(24);

  const [chunkSizeKb, setChunkSizeKb] = useState<number>(4096);
  const [maxConcurrentTransfers, setMaxConcurrentTransfers] = useState<number>(3);
  const [uploadLimitKbps, setUploadLimitKbps] = useState<number>(0);

  const [defaultCompression, setDefaultCompression] = useState<string>("Zstd");
  const [retentionDays, setRetentionDays] = useState<number>(30);
  const [verifyAfterBackup, setVerifyAfterBackup] = useState<boolean>(true);

  const loadSettings = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const [cfgRes, infoRes, pathsRes, tgRes, authRes] = await Promise.all([
        commands.getAppConfig(),
        commands.getAppInfo(),
        commands.getSystemPaths(),
        commands.getTelegramStatus(),
        commands.getTelegramAuthStatus(),
      ]);

      if (cfgRes.status === "ok") {
        const c = cfgRes.data;
        setConfig(c);
        setTheme(c.general.theme);
        setLanguage(c.general.language);
        setCheckUpdates(c.general.check_updates);

        setMaxCacheSizeMb(c.storage.max_cache_size_mb);
        setTempRetentionHours(c.storage.temp_retention_hours);

        setChunkSizeKb(c.transfer.chunk_size_kb);
        setMaxConcurrentTransfers(c.transfer.max_concurrent_transfers);
        setUploadLimitKbps(c.transfer.upload_limit_kbps);

        setDefaultCompression(c.backup.default_compression);
        setRetentionDays(c.backup.retention_days);
        setVerifyAfterBackup(c.backup.verify_after_backup);
      } else {
        setError(cfgRes.error.message);
      }

      if (infoRes.status === "ok") setAppInfo(infoRes.data);
      if (pathsRes.status === "ok") setPaths(pathsRes.data);
      if (tgRes.status === "ok") {
        setTelegramStatus(tgRes.data);
        if (tgRes.data.target_chat_id) {
          setTargetChatId(tgRes.data.target_chat_id.toString());
        }
      }
      if (authRes.status === "ok") {
        setAuthStatus(authRes.data);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const handleVerifyChannel = async () => {
    try {
      setVerifyingChannel(true);
      const res = await commands.verifyBackupChannel();
      if (res.status === "ok") {
        setAuthStatus(res.data);
        onNotify("success", "Channel Verified", "Private backup channel connectivity and permissions confirmed.");
      } else {
        onNotify("error", "Verification Failed", res.error.message || "Failed to verify channel.");
      }
    } catch (err: unknown) {
      onNotify("error", "Verification Error", String(err));
    } finally {
      setVerifyingChannel(false);
    }
  };

  const handleLogout = async () => {
    try {
      setLoggingOut(true);
      const res = await commands.logoutTelegram();
      if (res.status === "ok") {
        setAuthStatus(res.data);
        onNotify("info", "Logged Out", "Telegram session cleared. Returning to welcome gate.");
        window.location.reload();
      } else {
        onNotify("error", "Logout Failed", res.error.message || "Failed to log out.");
      }
    } catch (err: unknown) {
      onNotify("error", "Logout Error", String(err));
    } finally {
      setLoggingOut(false);
      setShowLogoutConfirm(false);
    }
  };

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const handleSaveTelegram = async () => {
    if (!botToken.trim()) {
      setTelegramNotice({ type: "error", msg: "Bot token is required." });
      return;
    }
    const chatIdNum = parseInt(targetChatId.trim(), 10);
    if (isNaN(chatIdNum) || chatIdNum === 0) {
      setTelegramNotice({ type: "error", msg: "A valid non-zero Target Chat ID is required." });
      return;
    }

    try {
      setSavingTelegram(true);
      setTelegramNotice(null);
      const req: SaveTelegramConfigDto = {
        bot_token: botToken.trim(),
        target_chat_id: chatIdNum,
        api_endpoint: apiEndpoint.trim() ? apiEndpoint.trim() : null,
      };
      const res = await commands.saveTelegramConfig(req);
      if (res.status === "ok") {
        setTelegramStatus(res.data);
        setBotToken(""); // Immediately clear raw token from state
        setTelegramNotice({
          type: "success",
          msg: "Telegram configuration saved securely. Test connection to verify cloud access.",
        });
        onNotify("success", "Telegram Configured", "Credentials stored in secure application config.");
      } else {
        setTelegramNotice({ type: "error", msg: res.error.message });
      }
    } catch (err: unknown) {
      setTelegramNotice({ type: "error", msg: String(err) });
    } finally {
      setSavingTelegram(false);
    }
  };

  const handleTestConnection = async () => {
    try {
      setTestingConnection(true);
      setTelegramNotice(null);
      const res = await commands.testTelegramConnection();
      if (res.status === "ok") {
        const testRes = res.data;
        if (testRes.success) {
          setTelegramNotice({
            type: "success",
            msg: `Connected successfully as @${testRes.bot_username || "Bot"} (ID: ${testRes.bot_id || "N/A"}) to ${testRes.chat_title || "chat"}.`,
          });
          onNotify("success", "Connection Verified", "Telegram Bot API is fully accessible.");
        } else {
          setTelegramNotice({
            type: "error",
            msg: testRes.error_message || "Connection test failed. Check token and chat permissions.",
          });
        }
        // Refresh status
        const tgRes = await commands.getTelegramStatus();
        if (tgRes.status === "ok") setTelegramStatus(tgRes.data);
      } else {
        setTelegramNotice({ type: "error", msg: res.error.message });
      }
    } catch (err: unknown) {
      setTelegramNotice({ type: "error", msg: String(err) });
    } finally {
      setTestingConnection(false);
    }
  };

  const handleDisconnectTelegram = async () => {
    try {
      const res = await commands.disconnectTelegram();
      if (res.status === "ok") {
        setTelegramStatus(res.data);
        setBotToken("");
        setTargetChatId("");
        setApiEndpoint("");
        setTelegramNotice({
          type: "info",
          msg: "Telegram disconnected. Active backend reverted to local mock storage.",
        });
        onNotify("info", "Telegram Disconnected", "Credentials cleared from local disk.");
      } else {
        setTelegramNotice({ type: "error", msg: res.error.message });
      }
    } catch (err: unknown) {
      setTelegramNotice({ type: "error", msg: String(err) });
    }
  };

  const handleSaveSettings = async () => {
    try {
      setSaving(true);
      setError(null);

      const updatedDto: AppConfigDto = {
        general: {
          theme,
          language,
          check_updates: checkUpdates,
        },
        storage: {
          max_cache_size_mb: maxCacheSizeMb,
          temp_retention_hours: tempRetentionHours,
        },
        transfer: {
          chunk_size_kb: chunkSizeKb,
          max_concurrent_transfers: maxConcurrentTransfers,
          upload_limit_kbps: uploadLimitKbps,
        },
        backup: {
          default_compression: defaultCompression,
          retention_days: retentionDays,
          verify_after_backup: verifyAfterBackup,
        },
      };

      const res = await commands.updateAppConfig(updatedDto);
      if (res.status === "ok") {
        setConfig(res.data);
        onNotify("success", "Settings Persisted", "Application configuration validated and written to disk.");
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  const getStatusBadgeClass = (status?: string) => {
    switch (status) {
      case "Connected":
        return "badge-success";
      case "Configured":
      case "Connecting":
        return "badge-info";
      case "ConnectionFailed":
        return "badge-danger";
      case "Reconnecting":
        return "badge-warning";
      default:
        return "badge-neutral";
    }
  };

  if (loading && !config) {
    return <LoadingSpinner message="Reading application settings from disk..." />;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Configuration Error" onDismiss={() => setError(null)} />}

      {/* Personal Telegram Account & Dedicated Private Channel Section */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Personal Telegram Account & Backup Channel</div>
            <div className="card-subtitle">
              Direct MTProto connection and private cloud vault for backup payloads
            </div>
          </div>
          <span
            className={`badge ${authStatus?.state === "ready" ? "badge-success" : "badge-warning"}`}
            style={{ padding: "0.3rem 0.6rem", borderRadius: "4px", fontSize: "0.75rem", fontWeight: 600 }}
          >
            {authStatus?.state === "ready" ? "🟢 Vault Ready" : `🟡 State: ${authStatus?.state || "Unknown"}`}
          </span>
        </div>

        <div className="grid-2" style={{ gap: "1.5rem" }}>
          <div style={{ background: "var(--bg-card)", padding: "1rem", borderRadius: "8px", border: "1px solid var(--border-card)" }}>
            <div style={{ fontWeight: 600, marginBottom: "0.5rem", color: "var(--text-main)" }}>
              Authenticated Account
            </div>
            {authStatus?.account ? (
              <div style={{ display: "flex", flexDirection: "column", gap: "0.35rem", fontSize: "0.85rem" }}>
                <div><strong>Name:</strong> {authStatus.account.first_name} {authStatus.account.last_name || ""}</div>
                <div><strong>User ID:</strong> <span className="mono">{authStatus.account.user_id}</span></div>
                {authStatus.account.username && <div><strong>Username:</strong> @{authStatus.account.username}</div>}
                <div><strong>Phone:</strong> {authStatus.account.phone_number}</div>
                <div style={{ fontSize: "0.75rem", color: "var(--text-dim)", marginTop: "0.2rem" }}>
                  Connected via official Telegram MTProto protocol
                </div>
              </div>
            ) : (
              <div style={{ color: "var(--text-dim)", fontSize: "0.85rem" }}>No personal account active</div>
            )}
          </div>

          <div style={{ background: "var(--bg-card)", padding: "1rem", borderRadius: "8px", border: "1px solid var(--border-card)" }}>
            <div style={{ fontWeight: 600, marginBottom: "0.5rem", color: "var(--text-main)" }}>
              Dedicated Backup Channel
            </div>
            {authStatus?.channel ? (
              <div style={{ display: "flex", flexDirection: "column", gap: "0.35rem", fontSize: "0.85rem" }}>
                <div><strong>Channel Title:</strong> {authStatus.channel.channel_title}</div>
                <div><strong>Channel ID:</strong> <span className="mono">{authStatus.channel.channel_id}</span></div>
                <div><strong>Privacy:</strong> {authStatus.channel.is_private ? "✓ Private Channel (Restricted)" : "Public"}</div>
                <div><strong>Status:</strong> {authStatus.channel.verified ? "✓ Verified & Write-Ready" : "Unverified"}</div>
                <div style={{ marginTop: "0.5rem" }}>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={handleVerifyChannel}
                    disabled={verifyingChannel}
                  >
                    {verifyingChannel ? "Verifying..." : "🔄 Re-verify Channel"}
                  </button>
                </div>
              </div>
            ) : (
              <div style={{ color: "var(--text-dim)", fontSize: "0.85rem" }}>No channel configured</div>
            )}
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: "1rem", borderTop: "1px solid var(--border-subtle)", paddingTop: "1rem" }}>
          {showLogoutConfirm ? (
            <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
              <span style={{ fontSize: "0.85rem", color: "var(--danger)" }}>
                Sign out of Telegram? This locks the desktop application until you re-authenticate.
              </span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setShowLogoutConfirm(false)}
                disabled={loggingOut}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn btn-danger btn-sm"
                onClick={handleLogout}
                disabled={loggingOut}
              >
                {loggingOut ? "Signing Out..." : "Confirm Sign Out"}
              </button>
            </div>
          ) : (
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => setShowLogoutConfirm(true)}
              disabled={loggingOut}
            >
              🚪 Sign Out of Telegram Account
            </button>
          )}
        </div>
      </div>

      {/* Telegram Cloud Storage Section */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Telegram Cloud Storage Integration</div>
            <div className="card-subtitle">
              Encrypted cloud backup transport and remote storage credentials
            </div>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "0.6rem" }}>
            <span style={{ fontSize: "0.8rem", color: "var(--text-dim)" }}>
              Backend: <strong>{telegramStatus?.active_backend || "unknown"}</strong>
            </span>
            <span
              className={`badge ${getStatusBadgeClass(telegramStatus?.status)}`}
              style={{ padding: "0.3rem 0.6rem", borderRadius: "4px", fontSize: "0.75rem", fontWeight: 600 }}
            >
              {telegramStatus?.status === "Connected"
                ? "🟢 Connected"
                : telegramStatus?.status === "Configured"
                ? "🔵 Configured"
                : telegramStatus?.status === "ConnectionFailed"
                ? "🔴 Connection Failed"
                : telegramStatus?.status === "Connecting"
                ? "🟡 Connecting..."
                : "⚪ Not Configured"}
            </span>
          </div>
        </div>

        {telegramNotice && (
          <div
            style={{
              padding: "0.75rem 1rem",
              borderRadius: "6px",
              marginBottom: "1rem",
              fontSize: "0.85rem",
              background:
                telegramNotice.type === "success"
                  ? "rgba(34, 197, 94, 0.1)"
                  : telegramNotice.type === "error"
                  ? "rgba(239, 68, 68, 0.1)"
                  : "rgba(59, 130, 246, 0.1)",
              border: `1px solid ${
                telegramNotice.type === "success"
                  ? "var(--success)"
                  : telegramNotice.type === "error"
                  ? "var(--danger)"
                  : "var(--accent)"
              }`,
              color:
                telegramNotice.type === "success"
                  ? "var(--success)"
                  : telegramNotice.type === "error"
                  ? "var(--danger)"
                  : "var(--accent)",
            }}
          >
            {telegramNotice.msg}
          </div>
        )}

        {telegramStatus?.is_configured && (
          <div
            style={{
              display: "flex",
              gap: "2rem",
              padding: "0.75rem 1rem",
              background: "var(--bg-card)",
              borderRadius: "6px",
              marginBottom: "1rem",
              fontSize: "0.85rem",
              border: "1px solid var(--border)",
            }}
          >
            <div>
              <span style={{ color: "var(--text-dim)" }}>Target Chat ID: </span>
              <strong className="mono">{telegramStatus.target_chat_id || "None"}</strong>
            </div>
            {telegramStatus.bot_username && (
              <div>
                <span style={{ color: "var(--text-dim)" }}>Bot Username: </span>
                <strong className="mono">@{telegramStatus.bot_username}</strong>
              </div>
            )}
            {telegramStatus.last_error && (
              <div style={{ color: "var(--danger)" }}>
                <span>Error: </span>
                <span>{telegramStatus.last_error}</span>
              </div>
            )}
          </div>
        )}

        <div className="grid-3">
          <div className="form-group">
            <label>Bot Token:</label>
            <input
              type="password"
              placeholder={telegramStatus?.is_configured ? "•••••••••••••••• (Leave blank to keep)" : "123456789:ABCdefGHI..."}
              value={botToken}
              onChange={(e) => setBotToken(e.target.value)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Isolated in secure credentials file. Never exposed in UI state.
            </span>
          </div>

          <div className="form-group">
            <label>Target Chat ID:</label>
            <input
              type="text"
              placeholder="-1001234567890"
              value={targetChatId}
              onChange={(e) => setTargetChatId(e.target.value)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Target private channel or Saved Messages chat ID.
            </span>
          </div>

          <div className="form-group">
            <label>API Endpoint (Optional):</label>
            <input
              type="text"
              placeholder="https://api.telegram.org (Default)"
              value={apiEndpoint}
              onChange={(e) => setApiEndpoint(e.target.value)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Custom or local Bot API server for 2 GB file chunking.
            </span>
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: "1rem" }}>
          <div>
            {telegramStatus?.is_configured && (
              <button
                className="btn btn-outline"
                style={{ color: "var(--danger)", borderColor: "var(--danger)" }}
                onClick={handleDisconnectTelegram}
              >
                Disconnect Telegram
              </button>
            )}
          </div>
          <div style={{ display: "flex", gap: "0.75rem" }}>
            {telegramStatus?.is_configured && (
              <button
                className="btn btn-outline"
                onClick={handleTestConnection}
                disabled={testingConnection}
              >
                {testingConnection ? "Testing..." : "🔌 Test Connection"}
              </button>
            )}
            <button
              className="btn btn-primary"
              onClick={handleSaveTelegram}
              disabled={savingTelegram}
            >
              {savingTelegram ? "Saving..." : "💾 Save Telegram Settings"}
            </button>
          </div>
        </div>
      </div>

      {/* General Settings */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">General Preferences</div>
            <div className="card-subtitle">User interface and automatic maintenance</div>
          </div>
        </div>

        <div className="grid-3">
          <div className="form-group">
            <label>Interface Theme:</label>
            <select value={theme} onChange={(e) => setTheme(e.target.value)}>
              <option value="dark">Dark Theme (Standard)</option>
              <option value="light">Light Theme</option>
              <option value="system">System Default</option>
            </select>
          </div>

          <div className="form-group">
            <label>Language:</label>
            <select value={language} onChange={(e) => setLanguage(e.target.value)}>
              <option value="en-US">English (United States)</option>
            </select>
          </div>

          <div className="form-group">
            <label>Updates:</label>
            <div style={{ marginTop: "0.5rem" }}>
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={checkUpdates}
                  onChange={(e) => setCheckUpdates(e.target.checked)}
                />
                <span>Automatically check for updates</span>
              </label>
            </div>
          </div>
        </div>
      </div>

      {/* Storage & Staging Settings */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Storage & Local Cache</div>
            <div className="card-subtitle">Local directory limits and temporary file lifecycles</div>
          </div>
        </div>

        <div className="grid-2">
          <div className="form-group">
            <label>Max Cache Size (MB):</label>
            <input
              type="number"
              min={128}
              max={65536}
              value={maxCacheSizeMb}
              onChange={(e) => setMaxCacheSizeMb(parseInt(e.target.value) || 2048)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Bound on local SQLite journal and transient chunk cache.
            </span>
          </div>

          <div className="form-group">
            <label>Temporary File Retention (Hours):</label>
            <input
              type="number"
              min={1}
              max={720}
              value={tempRetentionHours}
              onChange={(e) => setTempRetentionHours(parseInt(e.target.value) || 24)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Orphaned staging files older than this window are swept on startup.
            </span>
          </div>
        </div>
      </div>

      {/* Transfer Pipeline Settings */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Transfer Workers & Chunking Pipeline</div>
            <div className="card-subtitle">
              Slicing parameters, concurrent network workers, and upload throttling
            </div>
          </div>
        </div>

        <div className="grid-3">
          <div className="form-group">
            <label>Default Chunk Size (KB):</label>
            <input
              type="number"
              min={64}
              max={32768}
              step={1024}
              value={chunkSizeKb}
              onChange={(e) => setChunkSizeKb(parseInt(e.target.value) || 4096)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Standard slice size (64 KB to 32 MB). Default: 4096 KB (4 MB).
            </span>
          </div>

          <div className="form-group">
            <label>Max Concurrent Transfers:</label>
            <input
              type="number"
              min={1}
              max={16}
              value={maxConcurrentTransfers}
              onChange={(e) => setMaxConcurrentTransfers(parseInt(e.target.value) || 3)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Number of parallel chunk upload/download workers.
            </span>
          </div>

          <div className="form-group">
            <label>Upload Bandwidth Limit (KB/s):</label>
            <input
              type="number"
              min={0}
              max={1000000}
              value={uploadLimitKbps}
              onChange={(e) => setUploadLimitKbps(parseInt(e.target.value) || 0)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              0 = Unlimited. Prevents saturating residential internet connections.
            </span>
          </div>
        </div>
      </div>

      {/* Backup Defaults */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Backup Engine Defaults</div>
            <div className="card-subtitle">Default compression, retention, and verification policies</div>
          </div>
        </div>

        <div className="grid-3">
          <div className="form-group">
            <label>Default Compression:</label>
            <select
              value={defaultCompression}
              onChange={(e) => setDefaultCompression(e.target.value)}
            >
              <option value="Zstd">Zstandard (Zstd) — High throughput</option>
              <option value="None">None (Uncompressed)</option>
            </select>
          </div>

          <div className="form-group">
            <label>Default Snapshot Retention (Days):</label>
            <input
              type="number"
              min={0}
              max={3650}
              value={retentionDays}
              onChange={(e) => setRetentionDays(parseInt(e.target.value) || 30)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              0 = Keep indefinitely.
            </span>
          </div>

          <div className="form-group">
            <label>Verification:</label>
            <div style={{ marginTop: "0.5rem" }}>
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={verifyAfterBackup}
                  onChange={(e) => setVerifyAfterBackup(e.target.checked)}
                />
                <span>Verify cryptographic hash immediately after backup</span>
              </label>
            </div>
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: "1rem" }}>
          <button
            className="btn btn-primary"
            onClick={handleSaveSettings}
            disabled={saving}
          >
            {saving ? "Persisting Settings..." : "💾 Save Settings"}
          </button>
        </div>
      </div>

      {/* Diagnostics & Resolved System Paths */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Runtime Diagnostics & System Directories</div>
            <div className="card-subtitle">Deterministic PathManager locations and runtime metadata</div>
          </div>
        </div>

        <div className="grid-2">
          {appInfo && (
            <div style={{ display: "flex", flexDirection: "column", gap: "0.4rem", fontSize: "0.8rem" }}>
              <div><strong>Application:</strong> {appInfo.app_name}</div>
              <div><strong>Version:</strong> v{appInfo.version}</div>
              <div><strong>Platform:</strong> {appInfo.platform} ({appInfo.arch})</div>
              <div><strong>Build Mode:</strong> {appInfo.build_mode}</div>
              <div><strong>Architecture:</strong> Rust + Tauri 2 + React (Single Process)</div>
              <div><strong>Author:</strong> Created by Ankit Sharma</div>
            </div>
          )}

          {paths && (
            <div style={{ display: "flex", flexDirection: "column", gap: "0.4rem", fontSize: "0.78rem" }}>
              <div>
                <strong>Base Directory:</strong> <span className="mono">{paths.base_dir}</span>
              </div>
              <div>
                <strong>Database File:</strong> <span className="mono">{paths.database_path}</span>
              </div>
              <div>
                <strong>Temp Directory:</strong> <span className="mono">{paths.temp_dir}</span>
              </div>
              <div>
                <strong>Logs Directory:</strong> <span className="mono">{paths.logs_dir}</span>
              </div>
              <div>
                <strong>Config Directory:</strong> <span className="mono">{paths.config_dir}</span>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
