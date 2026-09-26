import React from "react";

export interface ProgressBarProps {
  progress: number; // 0 to 100
  color?: string;
  height?: number;
  glow?: boolean;
  style?: React.CSSProperties;
}

export const ProgressBar: React.FC<ProgressBarProps> = ({
  progress,
  color = "var(--accent-primary)",
  height = 6,
  glow = true,
  style,
}) => {
  const clampedProgress = Math.min(100, Math.max(0, progress));

  return (
    <div
      style={{
        width: "100%",
        height: `${height}px`,
        backgroundColor: "rgba(255, 255, 255, 0.08)",
        borderRadius: "var(--radius-full)",
        overflow: "hidden",
        position: "relative",
        ...style,
      }}
    >
      <div
        style={{
          width: `${clampedProgress}%`,
          height: "100%",
          background: color,
          borderRadius: "var(--radius-full)",
          transition: "width 0.3s cubic-bezier(0.16, 1, 0.3, 1)",
          boxShadow: glow ? `0 0 10px ${color}` : "none",
        }}
      />
    </div>
  );
};
