import { FileCategory } from "../types";

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

export function formatSpeed(bps: number): string {
  if (bps <= 0) return "0 KB/s";
  return formatBytes(bps) + "/s";
}

export function formatEta(seconds: number | null): string {
  if (!seconds || seconds <= 0) return "Estimating...";
  if (seconds < 60) return `~${Math.round(seconds)}s remaining`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.round(seconds % 60);
  if (mins < 60) return `~${mins}m ${secs}s`;
  const hours = Math.floor(mins / 60);
  return `~${hours}h ${mins % 60}m`;
}

export function formatDate(isoString: string): string {
  if (!isoString) return "—";
  try {
    const d = new Date(isoString);
    return d.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  } catch {
    return isoString;
  }
}

export function getFileCategory(name: string, mime?: string | null): FileCategory {
  const ext = name.split(".").pop()?.toLowerCase() || "";

  if (["png", "jpg", "jpeg", "webp", "gif", "svg", "bmp", "avif"].includes(ext) || mime?.startsWith("image/")) {
    return "images";
  }
  if (["mp4", "mkv", "mov", "webm", "avi", "wmv", "flv"].includes(ext) || mime?.startsWith("video/")) {
    return "videos";
  }
  if (["mp3", "wav", "flac", "aac", "ogg", "m4a"].includes(ext) || mime?.startsWith("audio/")) {
    return "audio";
  }
  if (["pdf", "doc", "docx", "txt", "md", "rtf", "odt", "xls", "xlsx", "csv", "ppt", "pptx"].includes(ext) || mime?.startsWith("text/") || mime?.includes("pdf")) {
    return "documents";
  }
  if (["zip", "tar", "gz", "7z", "rar", "bz2", "xz", "iso"].includes(ext)) {
    return "archives";
  }
  if (["rs", "ts", "tsx", "js", "jsx", "py", "c", "cpp", "h", "hpp", "go", "json", "html", "css", "yaml", "yml", "toml", "sh"].includes(ext)) {
    return "code";
  }
  return "all";
}

export function getCategoryAccent(category: FileCategory): string {
  switch (category) {
    case "images":
      return "var(--file-image)";
    case "videos":
      return "var(--file-video)";
    case "audio":
      return "var(--file-audio)";
    case "documents":
      return "var(--file-doc)";
    case "archives":
      return "var(--file-archive)";
    case "code":
      return "var(--file-code)";
    default:
      return "var(--accent-primary)";
  }
}
