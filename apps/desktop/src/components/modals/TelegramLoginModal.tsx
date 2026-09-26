import React, { useState } from "react";
import {
  Cloud,
  CheckCircle2,
  ArrowRight,
  ArrowLeft,
  Info,
  LogOut,
  Loader2,
} from "lucide-react";
import { Modal } from "../ui/Modal";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { StorageStatus } from "../../types";

export interface TelegramLoginModalProps {
  isOpen: boolean;
  onClose: () => void;
  storageStatus?: StorageStatus;
  onConnect: (status: StorageStatus) => void;
}

export const TelegramLoginModal: React.FC<TelegramLoginModalProps> = ({
  isOpen,
  onClose,
  storageStatus,
  onConnect,
}) => {
  const [step, setStep] = useState<1 | 2>(1);
  const [phoneNumber, setPhoneNumber] = useState("");
  const [apiId, setApiId] = useState("");
  const [apiHash, setApiHash] = useState("");
  const [code, setCode] = useState("");
  const [phoneCodeHash, setPhoneCodeHash] = useState<string | null>(null);
  const [twoFactorPassword, setTwoFactorPassword] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [statusMessage, setStatusMessage] = useState("");

  const isAlreadyConnected = Boolean(storageStatus?.is_connected);

  const handleSendCode = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!phoneNumber.trim()) {
      setStatusMessage("Please enter your Telegram phone number with country code (e.g. +91...).");
      return;
    }
    if (!apiId.trim() || !apiHash.trim()) {
      setStatusMessage("Please enter both your Telegram API ID and API Hash.");
      return;
    }

    setIsLoading(true);
    setStatusMessage("Connecting to Telegram MTProto and requesting login code...");

    try {
      const res = await fetch("http://127.0.0.1:8000/api/login/send-code", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          phone: phoneNumber.trim(),
          api_id: apiId.trim(),
          api_hash: apiHash.trim(),
        }),
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({ detail: res.statusText }));
        throw new Error(err.detail || "Failed to send code from Telegram");
      }

      const data = await res.json();
      setPhoneCodeHash(data.phone_code_hash);
      setStep(2);
      setStatusMessage(`Verification code sent to ${phoneNumber}. Check your Telegram app.`);
    } catch (err: any) {
      setStatusMessage(`Error: ${err.message || String(err)}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleVerifyCode = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!code.trim()) {
      setStatusMessage("Please enter the login code sent to your Telegram app.");
      return;
    }

    setIsLoading(true);
    setStatusMessage("Verifying login code and securing vault channels...");

    try {
      const res = await fetch("http://127.0.0.1:8000/api/login/verify-code", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          code: code.trim(),
          phone_code_hash: phoneCodeHash,
          password: twoFactorPassword.trim() || null,
        }),
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({ detail: res.statusText }));
        throw new Error(err.detail || "Authentication failed");
      }

      const data = await res.json();
      if (data.status === "2fa_required") {
        setStatusMessage("2-Step Verification password is required for this account. Enter it below.");
        setIsLoading(false);
        return;
      }

      const user = data.user || {};
      const newStatus: StorageStatus = {
        is_connected: true,
        provider_name: "Telegram MTProto Cloud",
        user_identifier: user.username ? `@${user.username}` : user.first_name || phoneNumber,
        is_premium: Boolean(user.is_premium),
        max_single_upload_bytes: 2000 * 1024 * 1024,
        total_quota_bytes: 2 * 1024 * 1024 * 1024 * 1024,
        used_bytes: 0,
      };

      onConnect(newStatus);
      setStatusMessage("Successfully connected to Telegram MTProto!");
      setTimeout(() => {
        onClose();
      }, 1000);
    } catch (err: any) {
      setStatusMessage(`Verification Error: ${err.message || String(err)}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogout = async () => {
    setIsLoading(true);
    try {
      await fetch("http://127.0.0.1:8000/api/logout", { method: "POST" });
      onConnect({
        is_connected: false,
        provider_name: "Telegram MTProto",
        is_premium: false,
        max_single_upload_bytes: 2000 * 1024 * 1024,
        total_quota_bytes: 2 * 1024 * 1024 * 1024 * 1024,
        used_bytes: 0,
      });
      setStep(1);
      setPhoneNumber("");
      setApiId("");
      setApiHash("");
      setCode("");
      setStatusMessage("Disconnected. Please log in with a Telegram account.");
    } catch (err: any) {
      setStatusMessage(`Logout Error: ${err.message}`);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        // Enforce login: don't allow closing if not connected
        if (isAlreadyConnected) {
          onClose();
        }
      }}
      title="Telegram Cloud Storage Login"
      subtitle="Connect your real Telegram account for secure, unlimited MTProto cloud backups."
      icon={<Cloud size={18} />}
      maxWidth="540px"
    >
      <div style={{ display: "flex", flexDirection: "column", gap: "18px" }}>
        {/* If already connected, show active account card */}
        {isAlreadyConnected ? (
          <div
            className="telecloud-card"
            style={{
              padding: "20px",
              backgroundColor: "var(--bg-surface)",
              display: "flex",
              flexDirection: "column",
              gap: "16px",
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: "14px" }}>
              <div
                style={{
                  width: "48px",
                  height: "48px",
                  borderRadius: "50%",
                  backgroundColor: "rgba(16, 185, 129, 0.15)",
                  color: "var(--status-success)",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  flexShrink: 0,
                }}
              >
                <CheckCircle2 size={28} />
              </div>
              <div>
                <h4 style={{ fontSize: "15px", fontWeight: 700, color: "var(--text-primary)" }}>
                  Telegram Account Connected
                </h4>
                <p style={{ fontSize: "13px", color: "var(--accent-primary)", marginTop: "2px" }}>
                  {storageStatus?.user_identifier}
                </p>
                <p style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: "2px" }}>
                  Active MTProto session linked to private Primary & Mirror channels.
                </p>
              </div>
            </div>

            <div style={{ height: "1px", backgroundColor: "var(--border-subtle)" }} />

            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <Button
                variant="danger"
                icon={<LogOut size={15} />}
                onClick={handleLogout}
                disabled={isLoading}
              >
                {isLoading ? "Disconnecting..." : "Disconnect / Switch Account"}
              </Button>
              <Button variant="primary" onClick={onClose}>
                Continue to Drive
              </Button>
            </div>
          </div>
        ) : (
          /* Step 1 or Step 2 Login Form */
          <>
            {/* Guide on how to get API ID / API Hash */}
            <div
              style={{
                padding: "12px 14px",
                backgroundColor: "rgba(59, 130, 246, 0.08)",
                border: "1px solid rgba(59, 130, 246, 0.2)",
                borderRadius: "var(--radius-md)",
                display: "flex",
                flexDirection: "column",
                gap: "6px",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <Info size={16} color="var(--accent-primary)" />
                <span style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)" }}>
                  Where to get Telegram API ID & Hash:
                </span>
              </div>
              <ol
                style={{
                  fontSize: "11px",
                  color: "var(--text-secondary)",
                  margin: 0,
                  paddingLeft: "20px",
                  lineHeight: "1.5",
                }}
              >
                <li>
                  Open <strong>my.telegram.org</strong> in your web browser.
                </li>
                <li>Log in with your Telegram phone number and enter the confirmation code.</li>
                <li>Click <strong>API development tools</strong> and create an app (e.g. TeleVault).</li>
                <li>Copy the numeric <strong>api_id</strong> and hexadecimal <strong>api_hash</strong>.</li>
              </ol>
            </div>

            {step === 1 ? (
              <form onSubmit={handleSendCode} style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
                <Input
                  label="Telegram Phone Number"
                  placeholder="+919876543210 (include country code)"
                  value={phoneNumber}
                  onChange={(e) => setPhoneNumber(e.target.value)}
                  disabled={isLoading}
                  autoFocus
                />
                <Input
                  label="API ID"
                  placeholder="e.g. 12345678"
                  value={apiId}
                  onChange={(e) => setApiId(e.target.value)}
                  disabled={isLoading}
                />
                <Input
                  label="API Hash"
                  placeholder="e.g. 0123456789abcdef0123456789abcdef"
                  value={apiHash}
                  onChange={(e) => setApiHash(e.target.value)}
                  disabled={isLoading}
                />

                {statusMessage && (
                  <p
                    style={{
                      fontSize: "12px",
                      color: statusMessage.startsWith("Error") ? "var(--status-error)" : "var(--accent-primary)",
                      margin: 0,
                    }}
                  >
                    {statusMessage}
                  </p>
                )}

                <Button
                  type="submit"
                  variant="primary"
                  disabled={isLoading || !phoneNumber.trim() || !apiId.trim() || !apiHash.trim()}
                  icon={isLoading ? <Loader2 size={15} style={{ animation: "spin 1s linear infinite" }} /> : <ArrowRight size={15} />}
                  style={{ width: "100%", marginTop: "6px", height: "42px" }}
                >
                  {isLoading ? "Sending Code via Telegram..." : "Send Verification Code"}
                </Button>
              </form>
            ) : (
              <form onSubmit={handleVerifyCode} style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                  <span style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    Sending code to: <strong>{phoneNumber}</strong>
                  </span>
                  <button
                    type="button"
                    onClick={() => {
                      setStep(1);
                      setStatusMessage("");
                    }}
                    style={{
                      background: "none",
                      border: "none",
                      color: "var(--accent-primary)",
                      fontSize: "12px",
                      cursor: "pointer",
                      display: "flex",
                      alignItems: "center",
                      gap: "4px",
                    }}
                  >
                    <ArrowLeft size={12} /> Edit Phone
                  </button>
                </div>

                <Input
                  label="Telegram Login Code"
                  placeholder="12345"
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                  disabled={isLoading}
                  autoFocus
                />

                <Input
                  label="2-Step Verification Password (Optional)"
                  placeholder="Only required if 2FA cloud password is set"
                  type="password"
                  value={twoFactorPassword}
                  onChange={(e) => setTwoFactorPassword(e.target.value)}
                  disabled={isLoading}
                />

                {statusMessage && (
                  <p
                    style={{
                      fontSize: "12px",
                      color: statusMessage.startsWith("Verification Error")
                        ? "var(--status-error)"
                        : "var(--accent-primary)",
                      margin: 0,
                    }}
                  >
                    {statusMessage}
                  </p>
                )}

                <Button
                  type="submit"
                  variant="primary"
                  disabled={isLoading || !code.trim()}
                  icon={isLoading ? <Loader2 size={15} style={{ animation: "spin 1s linear infinite" }} /> : <CheckCircle2 size={15} />}
                  style={{ width: "100%", marginTop: "6px", height: "42px" }}
                >
                  {isLoading ? "Authenticating MTProto Session..." : "Verify & Connect Vault"}
                </Button>
              </form>
            )}
          </>
        )}
      </div>
    </Modal>
  );
};
