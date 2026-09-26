import React, { useState, useEffect } from "react";
import { Upload, Lock, Eye, EyeOff, FolderOpen, FileText, CheckCircle2, Loader2 } from "lucide-react";
import { Modal } from "../ui/Modal";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { tauriApi } from "../../services/tauri";

export interface UploadModalProps {
  isOpen: boolean;
  initialFilePath?: string;
  onClose: () => void;
  onUpload: (filePath: string, isEncrypted: boolean, passphrase?: string) => Promise<void>;
}

export const UploadModal: React.FC<UploadModalProps> = ({
  isOpen,
  initialFilePath = "",
  onClose,
  onUpload,
}) => {
  const [filePath, setFilePath] = useState(initialFilePath);
  const [isEncrypted, setIsEncrypted] = useState(false);
  const [passphrase, setPassphrase] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState("");
  const [isUploading, setIsUploading] = useState(false);

  useEffect(() => {
    if (initialFilePath) {
      setFilePath(initialFilePath);
      setError("");
    }
  }, [initialFilePath]);

  const handleBrowse = async () => {
    try {
      const selected = await tauriApi.selectFile();
      if (selected) {
        setFilePath(selected);
        setError("");
      }
    } catch (err) {
      console.error("Browse failed:", err);
    }
  };

  const fileName = filePath ? filePath.split(/[\\/]/).pop() || filePath : "";

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!filePath.trim()) {
      setError("Please select or enter a valid local file path.");
      return;
    }
    setError("");
    setIsUploading(true);
    try {
      await onUpload(filePath.trim(), isEncrypted, passphrase.trim() || undefined);
      setFilePath("");
      setPassphrase("");
      setIsUploading(false);
      onClose();
    } catch (err: any) {
      setError(err?.message || "Upload failed. Please try again.");
      setIsUploading(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        if (!isUploading) onClose();
      }}
      title="Upload File to Telegram Drive"
      subtitle="Direct MTProto cloud upload to your private TeleVault Primary & Mirror channels."
      icon={<Upload size={18} />}
      maxWidth="520px"
    >
      <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: "18px" }}>
        {/* Selected File Banner or Input */}
        <div style={{ display: "flex", gap: "10px", alignItems: "flex-end" }}>
          <div style={{ flex: 1 }}>
            <Input
              label="Local File Path"
              placeholder="C:\Users\...\document.pdf"
              value={filePath}
              disabled={isUploading}
              onChange={(e) => {
                setFilePath(e.target.value);
                setError("");
              }}
              error={error}
              autoFocus
            />
          </div>
          <Button
            type="button"
            variant="secondary"
            icon={<FolderOpen size={16} />}
            onClick={handleBrowse}
            disabled={isUploading}
            style={{ marginBottom: error ? "22px" : "0", height: "40px" }}
          >
            Browse
          </Button>
        </div>

        {/* Selected File Card */}
        {fileName && !error && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: "10px",
              padding: "10px 14px",
              backgroundColor: "rgba(59, 130, 246, 0.08)",
              border: "1px solid rgba(59, 130, 246, 0.25)",
              borderRadius: "var(--radius-md)",
            }}
          >
            <FileText size={20} color="var(--accent-primary)" />
            <div style={{ flex: 1, minWidth: 0 }}>
              <p
                style={{
                  fontSize: "13px",
                  fontWeight: 600,
                  color: "var(--text-primary)",
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }}
              >
                {fileName}
              </p>
              <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                Ready for MTProto cloud upload & Telegram channel backup
              </p>
            </div>
            <CheckCircle2 size={16} color="var(--status-success)" />
          </div>
        )}

        {/* Zero-Knowledge Encryption Toggle Box */}
        <div
          className="telecloud-card"
          style={{
            padding: "16px",
            backgroundColor: "var(--bg-surface)",
            display: "flex",
            flexDirection: "column",
            gap: "14px",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
              <div
                style={{
                  width: "32px",
                  height: "32px",
                  borderRadius: "var(--radius-sm)",
                  backgroundColor: isEncrypted ? "var(--status-success-subtle)" : "rgba(255, 255, 255, 0.05)",
                  color: isEncrypted ? "var(--status-success)" : "var(--text-muted)",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                }}
              >
                <Lock size={16} />
              </div>
              <div>
                <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                  Zero-Knowledge Encryption
                </p>
                <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                  {isEncrypted
                    ? "AES-256-GCM authenticated per chunk (Private Vault)"
                    : "Upload original unencrypted document to Telegram"}
                </p>
              </div>
            </div>

            <label
              style={{
                position: "relative",
                display: "inline-block",
                width: "40px",
                height: "22px",
                cursor: isUploading ? "not-allowed" : "pointer",
              }}
            >
              <input
                type="checkbox"
                checked={isEncrypted}
                disabled={isUploading}
                onChange={(e) => setIsEncrypted(e.target.checked)}
                style={{ opacity: 0, width: 0, height: 0 }}
              />
              <span
                style={{
                  position: "absolute",
                  inset: 0,
                  backgroundColor: isEncrypted ? "var(--accent-primary)" : "rgba(255, 255, 255, 0.15)",
                  borderRadius: "var(--radius-full)",
                  transition: "background var(--transition-fast)",
                }}
              >
                <span
                  style={{
                    position: "absolute",
                    left: isEncrypted ? "20px" : "3px",
                    top: "3px",
                    width: "16px",
                    height: "16px",
                    borderRadius: "50%",
                    backgroundColor: "#ffffff",
                    transition: "left var(--transition-fast)",
                  }}
                />
              </span>
            </label>
          </div>

          {isEncrypted && (
            <div style={{ paddingTop: "6px", borderTop: "1px solid var(--border-subtle)" }}>
              <Input
                label="Custom Encryption Passphrase (Optional)"
                type={showPassword ? "text" : "password"}
                placeholder="Leave blank to use default account key"
                value={passphrase}
                disabled={isUploading}
                onChange={(e) => setPassphrase(e.target.value)}
                iconRight={
                  <button
                    type="button"
                    onClick={() => setShowPassword((p) => !p)}
                    style={{
                      background: "none",
                      border: "none",
                      color: "var(--text-muted)",
                      cursor: "pointer",
                      padding: 0,
                      display: "flex",
                    }}
                  >
                    {showPassword ? <EyeOff size={15} /> : <Eye size={15} />}
                  </button>
                }
              />
              <p style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: "6px" }}>
                Keys are derived via Argon2id with random 96-bit nonces. Plaintext never touches the network.
              </p>
            </div>
          )}
        </div>

        {/* Uploading Status Banner */}
        {isUploading && (
          <div
            style={{
              padding: "12px 16px",
              backgroundColor: "rgba(59, 130, 246, 0.12)",
              border: "1px solid var(--accent-primary)",
              borderRadius: "var(--radius-md)",
              display: "flex",
              alignItems: "center",
              gap: "12px",
            }}
          >
            <Loader2
              size={20}
              color="var(--accent-primary)"
              style={{ animation: "spin 1s linear infinite" }}
            />
            <div>
              <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                Uploading to Telegram...
              </p>
              <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                Computing SHA-256 and dual-writing to Primary & Mirror channels.
              </p>
            </div>
          </div>
        )}

        {/* Buttons */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "10px", marginTop: "4px" }}>
          <Button type="button" variant="secondary" onClick={onClose} disabled={isUploading}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" disabled={isUploading || !filePath.trim()}>
            {isUploading ? "Uploading to Telegram..." : "Upload to Telegram"}
          </Button>
        </div>
      </form>
    </Modal>
  );
};
