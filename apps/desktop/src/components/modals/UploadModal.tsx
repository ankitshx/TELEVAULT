import React, { useState } from "react";
import { Upload, Lock, Eye, EyeOff, FolderOpen } from "lucide-react";
import { Modal } from "../ui/Modal";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { tauriApi } from "../../services/tauri";

export interface UploadModalProps {
  isOpen: boolean;
  onClose: () => void;
  onUpload: (filePath: string, isEncrypted: boolean, passphrase?: string) => void;
}

export const UploadModal: React.FC<UploadModalProps> = ({ isOpen, onClose, onUpload }) => {
  const [filePath, setFilePath] = useState("");
  const [isEncrypted, setIsEncrypted] = useState(true);
  const [passphrase, setPassphrase] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState("");

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

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!filePath.trim()) {
      setError("Please specify a valid local file path");
      return;
    }
    setError("");
    onUpload(filePath.trim(), isEncrypted, passphrase.trim() || undefined);
    setFilePath("");
    setPassphrase("");
    onClose();
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Upload to Cloud"
      subtitle="Files are sliced into chunks and streamed with bounded RAM usage."
      icon={<Upload size={18} />}
      maxWidth="500px"
    >
      <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: "18px" }}>
        <div style={{ display: "flex", gap: "10px", alignItems: "flex-end" }}>
          <div style={{ flex: 1 }}>
            <Input
              label="Local File Path"
              placeholder="C:\Users\...\document.pdf"
              value={filePath}
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
            style={{ marginBottom: error ? "22px" : "0", height: "40px" }}
          >
            Browse
          </Button>
        </div>

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
                  AES-256-GCM authenticated per chunk
                </p>
              </div>
            </div>

            <label style={{ position: "relative", display: "inline-block", width: "40px", height: "22px", cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={isEncrypted}
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

        {/* Buttons */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "10px", marginTop: "4px" }}>
          <Button type="button" variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant="primary">
            Start Upload
          </Button>
        </div>
      </form>
    </Modal>
  );
};
