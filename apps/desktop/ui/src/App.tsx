import React, { useEffect, useState } from "react";
import { commands } from "./bindings";

interface AppInfo {
  app_name: string;
  version: string;
  platform: string;
}

export default function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [status, setStatus] = useState<string>("Initializing typed Tauri IPC connection...");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function testIpc() {
      try {
        const res = await commands.getAppInfo();
        if (res.status === "ok") {
          setAppInfo(res.data);
          setStatus("Connected to in-process Rust core via typed Tauri IPC");
        } else {
          setError(`[${res.error.code}] ${res.error.message}`);
          setStatus("IPC returned error");
        }
      } catch (err: unknown) {
        setError(typeof err === "object" && err !== null ? JSON.stringify(err) : String(err));
        setStatus("IPC verification failed");
      }
    }
    testIpc();
  }, []);

  return (
    <div style={{ fontFamily: "Segoe UI, sans-serif", padding: "2rem", maxWidth: "600px", margin: "0 auto" }}>
      <h1 style={{ color: "#1e293b", fontSize: "1.5rem" }}>TELEVAULT Desktop Core</h1>
      <p style={{ color: "#475569" }}>
        <strong>IPC Status:</strong> {status}
      </p>
      {appInfo && (
        <div style={{ background: "#f8fafc", border: "1px solid #e2e8f0", padding: "1rem", borderRadius: "8px", marginTop: "1rem" }}>
          <p style={{ margin: "0.25rem 0" }}><strong>Application:</strong> {appInfo.app_name}</p>
          <p style={{ margin: "0.25rem 0" }}><strong>Version:</strong> {appInfo.version}</p>
          <p style={{ margin: "0.25rem 0" }}><strong>Platform:</strong> {appInfo.platform}</p>
        </div>
      )}
      {error && (
        <div style={{ color: "#dc2626", background: "#fef2f2", border: "1px solid #fecaca", padding: "1rem", borderRadius: "8px", marginTop: "1rem" }}>
          <p style={{ margin: 0 }}><strong>Error:</strong> {error}</p>
        </div>
      )}
    </div>
  );
}
