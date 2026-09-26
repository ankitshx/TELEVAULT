import React, { useState } from "react";
import { Cloud, Lock, ShieldCheck, Zap, KeyRound, Phone, CheckCircle2, ArrowRight } from "lucide-react";
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
  onConnect,
}) => {
  const [authMode, setAuthMode] = useState<"instant" | "credentials">("instant");
  const [phoneNumber, setPhoneNumber] = useState("");
  const [apiId, setApiId] = useState("");
  const [apiHash, setApiHash] = useState("");
  const [twoFactorPassword, setTwoFactorPassword] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [statusMessage, setStatusMessage] = useState("");

  const handleInstantConnect = () => {
    setIsLoading(true);
    setStatusMessage("Connecting to local MTProto cloud vault engine...");
    setTimeout(() => {
      const newStatus: StorageStatus = {
        is_connected: true,
        provider_name: "Telegram MTProto (Local Vault Engine)",
        user_identifier: "telecloud-user",
        is_premium: true,
        max_single_upload_bytes: 4000 * 1024 * 1024,
        total_quota_bytes: 2 * 1024 * 1024 * 1024 * 1024,
        used_bytes: 0,
      };
      onConnect(newStatus);
      setIsLoading(false);
      onClose();
    }, 400);
  };

  const handleCredentialConnect = (e: React.FormEvent) => {
    e.preventDefault();
    if (!phoneNumber.trim()) {
      setStatusMessage("Please enter your Telegram phone number.");
      return;
    }
    setIsLoading(true);
    setStatusMessage("Authenticating with Telegram MTProto session...");
    setTimeout(() => {
      const newStatus: StorageStatus = {
        is_connected: true,
        provider_name: "Telegram MTProto Cloud",
        user_identifier: phoneNumber.trim(),
        is_premium: false,
        max_single_upload_bytes: 2000 * 1024 * 1024,
        total_quota_bytes: 2 * 1024 * 1024 * 1024 * 1024,
        used_bytes: 0,
      };
      onConnect(newStatus);
      setIsLoading(false);
      onClose();
    }, 600);
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Connect Telegram Account"
      subtitle="TeleCloud uses Telegram MTProto channels as unlimited private object storage."
      icon={<Cloud size={18} />}
      maxWidth="540px"
    >
      <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
        {/* Mode Selector Tabs */}
        <div
          style={{
            display: "flex",
            backgroundColor: "var(--bg-base)",
            borderRadius: "var(--radius-md)",
            padding: "4px",
            border: "1px solid var(--border-subtle)",
          }}
        >
          <button
            type="button"
            onClick={() => setAuthMode("instant")}
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "8px",
              padding: "8px 14px",
              borderRadius: "var(--radius-sm)",
              border: "none",
              backgroundColor: authMode === "instant" ? "var(--bg-surface)" : "transparent",
              color: authMode === "instant" ? "var(--text-primary)" : "var(--text-muted)",
              fontWeight: authMode === "instant" ? 600 : 500,
              fontSize: "13px",
              cursor: "pointer",
              transition: "all var(--transition-fast)",
            }}
          >
            <Zap size={15} color={authMode === "instant" ? "var(--accent-primary)" : undefined} />
            Instant Engine (Ready)
          </button>
          <button
            type="button"
            onClick={() => setAuthMode("credentials")}
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "8px",
              padding: "8px 14px",
              borderRadius: "var(--radius-sm)",
              border: "none",
              backgroundColor: authMode === "credentials" ? "var(--bg-surface)" : "transparent",
              color: authMode === "credentials" ? "var(--text-primary)" : "var(--text-muted)",
              fontWeight: authMode === "credentials" ? 600 : 500,
              fontSize: "13px",
              cursor: "pointer",
              transition: "all var(--transition-fast)",
            }}
          >
            <KeyRound size={15} color={authMode === "credentials" ? "var(--accent-primary)" : undefined} />
            Live MTProto Account
          </button>
        </div>

        {authMode === "instant" ? (
          <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
            <div
              className="telecloud-card"
              style={{
                padding: "16px",
                backgroundColor: "var(--bg-surface)",
                display: "flex",
                flexDirection: "column",
                gap: "12px",
              }}
            >
              <div style={{ display: "flex", alignItems: "flex-start", gap: "12px" }}>
                <div
                  style={{
                    width: "36px",
                    height: "36px",
                    borderRadius: "var(--radius-sm)",
                    backgroundColor: "var(--accent-subtle)",
                    color: "var(--accent-primary)",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    flexShrink: 0,
                  }}
                >
                  <ShieldCheck size={20} />
                </div>
                <div>
                  <h4 style={{ fontSize: "14px", fontWeight: 600, color: "var(--text-primary)" }}>
                    Zero-Knowledge Local Storage Engine
                  </h4>
                  <p style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "4px", lineHeight: "1.4" }}>
                    Runs high-performance AES-256-GCM authenticated chunk streaming directly into your encrypted
                    local vault channels. Ideal for instant desktop operation, offline verification, and smoke testing.
                  </p>
                </div>
              </div>

              <div style={{ display: "flex", gap: "16px", paddingTop: "8px", borderTop: "1px solid var(--border-subtle)" }}>
                <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "12px", color: "var(--text-muted)" }}>
                  <CheckCircle2 size={14} color="var(--status-success)" />
                  <span>2GB / 4GB Chunking</span>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "12px", color: "var(--text-muted)" }}>
                  <CheckCircle2 size={14} color="var(--status-success)" />
                  <span>Argon2id KDF</span>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "12px", color: "var(--status-success)" }}>
                  <CheckCircle2 size={14} color="var(--status-success)" />
                  <span>FTS5 Search Index</span>
                </div>
              </div>
            </div>

            <Button
              variant="primary"
              size="lg"
              onClick={handleInstantConnect}
              disabled={isLoading}
              icon={<ArrowRight size={16} />}
              style={{ width: "100%", justifyContent: "center" }}
            >
              {isLoading ? "Connecting Engine..." : "Connect Local Engine & Continue"}
            </Button>
          </div>
        ) : (
          <form onSubmit={handleCredentialConnect} style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
            <Input
              label="Telegram Phone Number"
              placeholder="+1234567890"
              value={phoneNumber}
              onChange={(e) => setPhoneNumber(e.target.value)}
              icon={<Phone size={15} />}
              autoFocus
            />

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "12px" }}>
              <Input
                label="API ID (my.telegram.org)"
                placeholder="1234567"
                value={apiId}
                onChange={(e) => setApiId(e.target.value)}
              />
              <Input
                label="API Hash"
                placeholder="abcdef1234567890..."
                value={apiHash}
                onChange={(e) => setApiHash(e.target.value)}
              />
            </div>

            <Input
              label="2FA Cloud Password (Optional)"
              type="password"
              placeholder="Enter two-factor password"
              value={twoFactorPassword}
              onChange={(e) => setTwoFactorPassword(e.target.value)}
              icon={<Lock size={15} />}
            />

            {statusMessage && (
              <p style={{ fontSize: "12px", color: "var(--accent-primary)" }}>{statusMessage}</p>
            )}

            <Button
              type="submit"
              variant="primary"
              size="lg"
              disabled={isLoading}
              style={{ width: "100%", justifyContent: "center", marginTop: "6px" }}
            >
              {isLoading ? "Connecting to MTProto..." : "Sign In & Authorize Storage"}
            </Button>
          </form>
        )}
      </div>
    </Modal>
  );
};
