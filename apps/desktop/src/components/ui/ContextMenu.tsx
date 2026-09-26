import React, { useEffect, useRef } from "react";

export interface ContextMenuItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
  danger?: boolean;
  divider?: boolean;
  disabled?: boolean;
  onClick: () => void;
}

export interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export const ContextMenu: React.FC<ContextMenuProps> = ({ x, y, items, onClose }) => {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };

    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  // Adjust positioning to stay within window bounds
  const adjustedX = Math.min(x, window.innerWidth - 220);
  const adjustedY = Math.min(y, window.innerHeight - items.length * 36 - 20);

  return (
    <div
      ref={menuRef}
      className="glass-surface-elevated animate-fade-in"
      style={{
        position: "fixed",
        left: `${adjustedX}px`,
        top: `${adjustedY}px`,
        minWidth: "200px",
        backgroundColor: "var(--bg-surface-elevated)",
        border: "1px solid var(--border-card)",
        borderRadius: "var(--radius-md)",
        padding: "6px",
        zIndex: 2000,
        boxShadow: "var(--shadow-popover)",
      }}
    >
      {items.map((item, idx) => (
        <React.Fragment key={item.id || idx}>
          {item.divider && (
            <div
              style={{
                height: "1px",
                backgroundColor: "var(--border-subtle)",
                margin: "4px 6px",
              }}
            />
          )}
          <button
            disabled={item.disabled}
            onClick={() => {
              item.onClick();
              onClose();
            }}
            style={{
              width: "100%",
              display: "flex",
              alignItems: "center",
              gap: "10px",
              padding: "7px 10px",
              borderRadius: "var(--radius-sm)",
              border: "none",
              background: "transparent",
              color: item.danger
                ? "var(--status-error)"
                : item.disabled
                ? "var(--text-muted)"
                : "var(--text-primary)",
              fontSize: "12px",
              fontWeight: 500,
              cursor: item.disabled ? "not-allowed" : "pointer",
              textAlign: "left",
              transition: "background var(--transition-fast)",
            }}
            onMouseEnter={(e) => {
              if (!item.disabled) {
                e.currentTarget.style.backgroundColor = item.danger
                  ? "var(--status-error-subtle)"
                  : "var(--accent-subtle)";
              }
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "transparent";
            }}
          >
            {item.icon && (
              <span
                style={{
                  display: "flex",
                  alignItems: "center",
                  color: item.danger
                    ? "var(--status-error)"
                    : "var(--text-secondary)",
                }}
              >
                {item.icon}
              </span>
            )}
            <span style={{ flex: 1 }}>{item.label}</span>
          </button>
        </React.Fragment>
      ))}
    </div>
  );
};
