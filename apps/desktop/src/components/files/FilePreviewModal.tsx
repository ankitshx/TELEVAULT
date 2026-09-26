import React, { useState } from "react";
import {
  Download,
  Star,
  Trash2,
  ShieldCheck,
  Lock,
  Copy,
  Check,
  FileText,
  FileImage,
  FileVideo,
  Music,
  FileCode,
  Archive,
} from "lucide-react";
import { LogicalFile } from "../../types";
import { formatBytes, formatDate, getFileCategory, getCategoryAccent } from "../../utils/format";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import { Badge } from "../ui/Badge";

export interface FilePreviewModalProps {
  file: LogicalFile | null;
  isOpen: boolean;
  onClose: () => void;
  onDownload: (file: LogicalFile) => void;
  onToggleFavorite: (fileId: string) => void;
  onDelete: (fileId: string) => void;
}

export const FilePreviewModal: React.FC<FilePreviewModalProps> = ({
  file,
  isOpen,
  onClose,
  onDownload,
  onToggleFavorite,
  onDelete,
}) => {
  const [copiedHash, setCopiedHash] = useState(false);

  if (!file) return null;

  const category = getFileCategory(file.name, file.mime_type);
  const accentColor = getCategoryAccent(category);

  const handleCopyHash = () => {
    navigator.clipboard.writeText(file.sha256);
    setCopiedHash(true);
    setTimeout(() => setCopiedHash(false), 2000);
  };

  const renderMediaPreview = () => {
    return (
      <div
        style={{
          width: "100%",
          height: "220px",
          backgroundColor: "rgba(0, 0, 0, 0.4)",
          borderRadius: "var(--radius-lg)",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          border: "1px solid var(--border-subtle)",
          position: "relative",
          overflow: "hidden",
        }}
      >
        {category === "images" && (
          <div style={{ textAlign: "center", color: accentColor }}>
            <FileImage size={56} />
            <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "8px" }}>
              High-Resolution Encrypted Image
            </p>
          </div>
        )}
        {category === "videos" && (
          <div style={{ textAlign: "center", color: accentColor }}>
            <FileVideo size={56} />
            <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "8px" }}>
              Multi-Chunk Video Stream
            </p>
          </div>
        )}
        {category === "audio" && (
          <div style={{ textAlign: "center", color: accentColor }}>
            <Music size={56} />
            <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "8px" }}>
              Audio Track Preview
            </p>
          </div>
        )}
        {category === "code" && (
          <div style={{ textAlign: "center", color: accentColor }}>
            <FileCode size={56} />
            <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "8px" }}>
              Source Code Module
            </p>
          </div>
        )}
        {category === "archives" && (
          <div style={{ textAlign: "center", color: accentColor }}>
            <Archive size={56} />
            <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "8px" }}>
              Compressed Archive Container
            </p>
          </div>
        )}
        {category === "documents" && (
          <div style={{ textAlign: "center", color: accentColor }}>
            <FileText size={56} />
            <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "8px" }}>
              Encrypted Document
            </p>
          </div>
        )}
      </div>
    );
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="File Details & Preview"
      subtitle={file.name}
      maxWidth="560px"
    >
      <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
        {/* Media Preview Box */}
        {renderMediaPreview()}

        {/* Security & Integrity Banner */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "12px 16px",
            backgroundColor: file.is_encrypted
              ? "var(--status-success-subtle)"
              : "rgba(255, 255, 255, 0.04)",
            border: `1px solid ${
              file.is_encrypted ? "rgba(16, 185, 129, 0.25)" : "var(--border-subtle)"
            }`,
            borderRadius: "var(--radius-md)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            {file.is_encrypted ? (
              <Lock size={16} color="var(--status-success)" />
            ) : (
              <ShieldCheck size={16} color="var(--accent-primary)" />
            )}
            <div>
              <p
                style={{
                  fontSize: "12px",
                  fontWeight: 600,
                  color: file.is_encrypted ? "var(--status-success)" : "var(--text-primary)",
                }}
              >
                {file.is_encrypted
                  ? "Zero-Knowledge Encrypted (AES-256-GCM)"
                  : "Cryptographically Verified"}
              </p>
              <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                {file.is_encrypted
                  ? "Encrypted client-side. Telegram servers cannot read this content."
                  : "Stored across partitioned Telegram MTProto channels."}
              </p>
            </div>
          </div>
          <Badge variant={file.is_encrypted ? "success" : "blue"} size="sm">
            {file.is_encrypted ? "Protected" : "Plaintext"}
          </Badge>
        </div>

        {/* Metadata Grid */}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 1fr",
            gap: "12px",
            fontSize: "12px",
          }}
        >
          <div
            className="telecloud-card"
            style={{ padding: "10px 14px", backgroundColor: "var(--bg-surface)" }}
          >
            <span style={{ color: "var(--text-muted)" }}>Size</span>
            <p style={{ fontWeight: 600, color: "var(--text-primary)", marginTop: "2px" }}>
              {formatBytes(file.size)}
            </p>
          </div>

          <div
            className="telecloud-card"
            style={{ padding: "10px 14px", backgroundColor: "var(--bg-surface)" }}
          >
            <span style={{ color: "var(--text-muted)" }}>Uploaded</span>
            <p style={{ fontWeight: 600, color: "var(--text-primary)", marginTop: "2px" }}>
              {formatDate(file.created_at)}
            </p>
          </div>
        </div>

        {/* SHA-256 Hash Display */}
        <div
          className="telecloud-card"
          style={{
            padding: "12px 14px",
            backgroundColor: "var(--bg-surface)",
            display: "flex",
            flexDirection: "column",
            gap: "6px",
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "11px", color: "var(--text-muted)", fontWeight: 600 }}>
              SHA-256 INTEGRITY HASH
            </span>
            <button
              onClick={handleCopyHash}
              style={{
                background: "transparent",
                border: "none",
                color: copiedHash ? "var(--status-success)" : "var(--accent-hover)",
                fontSize: "11px",
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
                gap: "4px",
              }}
            >
              {copiedHash ? <Check size={12} /> : <Copy size={12} />}
              {copiedHash ? "Copied" : "Copy Hash"}
            </button>
          </div>
          <code
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: "11px",
              color: "var(--text-secondary)",
              wordBreak: "break-all",
              backgroundColor: "rgba(0, 0, 0, 0.3)",
              padding: "6px 8px",
              borderRadius: "var(--radius-xs)",
            }}
          >
            {file.sha256}
          </code>
        </div>

        {/* Bottom Actions */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: "4px" }}>
          <Button
            variant="danger"
            size="sm"
            icon={<Trash2 size={14} />}
            onClick={() => {
              onDelete(file.id);
              onClose();
            }}
          >
            Delete File
          </Button>

          <div style={{ display: "flex", gap: "10px" }}>
            <Button
              variant="secondary"
              size="sm"
              icon={<Star size={14} fill={file.is_favorite ? "var(--status-warning)" : "none"} />}
              onClick={() => onToggleFavorite(file.id)}
            >
              {file.is_favorite ? "Favorited" : "Favorite"}
            </Button>
            <Button
              variant="primary"
              size="sm"
              icon={<Download size={14} />}
              onClick={() => {
                onDownload(file);
                onClose();
              }}
            >
              Download
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
};
