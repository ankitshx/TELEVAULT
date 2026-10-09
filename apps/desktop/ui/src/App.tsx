import { useState, useEffect, useCallback } from "react";
import {
  commands,
  AppInfoDto,
  BackupProfileDto,
  TelegramAuthStatusDto,
} from "./bindings";
import { Sidebar } from "./components/Sidebar";
import { Header } from "./components/Header";
import { ToastContainer } from "./components/ToastContainer";
import { NavTab, ToastMessage } from "./types";

// Views
import { WelcomeGate } from "./views/WelcomeGate";
import { DashboardView } from "./views/DashboardView";
import { BackupsView } from "./views/BackupsView";
import { RestoreView } from "./views/RestoreView";
import { SchedulesView } from "./views/SchedulesView";
import { VerificationView } from "./views/VerificationView";
import { RepairView } from "./views/RepairView";
import { ActivityView } from "./views/ActivityView";
import { ProfilesView } from "./views/ProfilesView";
import { RetentionView } from "./views/RetentionView";
import { SettingsView } from "./views/SettingsView";

const screenTitles: Record<NavTab, { title: string; subtitle: string }> = {
  dashboard: {
    title: "System Dashboard",
    subtitle: "Real-time overview of backup targets, schedules, and cloud activity",
  },
  backups: {
    title: "Backup Management",
    subtitle: "Coordinate point-in-time snapshots, incremental scans, and file catalogs",
  },
  restore: {
    title: "Disaster Recovery & Restore",
    subtitle: "Reconstruct backed-up files and snapshots with verified checksum integrity",
  },
  schedules: {
    title: "Automated Schedules",
    subtitle: "Event-driven background backup coordination with zero idle CPU overhead",
  },
  verification: {
    title: "Remote Storage Verification",
    subtitle: "Four-level cryptographic integrity audits across Telegram Cloud chunks",
  },
  repair: {
    title: "Remote Repair & Chunk Recovery",
    subtitle: "Isolated reconstruction of damaged chunks into fresh remote references",
  },
  activity: {
    title: "Transfer Activity & Queue",
    subtitle: "Monitor active transfer workers, network speeds, and cancellation status",
  },
  profiles: {
    title: "Backup Profiles",
    subtitle: "Configure source folders, exclusions, compression, and encryption keys",
  },
  retention: {
    title: "Snapshot Retention Policies",
    subtitle: "Local metadata pruning preserving immutable remote Telegram objects",
  },
  settings: {
    title: "Application Settings",
    subtitle: "AppConfig preferences, transfer limits, and filesystem diagnostic paths",
  },
};

export default function App() {
  const [currentTab, setCurrentTab] = useState<NavTab>("dashboard");
  const [appInfo, setAppInfo] = useState<AppInfoDto | null>(null);
  const [schedulerStatus, setSchedulerStatus] = useState<string>("Unknown");
  const [activeTransfers, setActiveTransfers] = useState<number>(0);
  const [authStatus, setAuthStatus] = useState<TelegramAuthStatusDto | null>(null);
  const [isAuthInitializing, setIsAuthInitializing] = useState<boolean>(true);
  const [initError, setInitError] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState<boolean>(false);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  // Navigation contextual parameters
  const [contextProfileId, setContextProfileId] = useState<string | undefined>(undefined);
  const [contextSnapshotId, setContextSnapshotId] = useState<string | undefined>(undefined);

  const addToast = useCallback(
    (type: "success" | "error" | "warning" | "info", title: string, message: string) => {
      const id = `${Date.now()}-${Math.random()}`;
      setToasts((prev) => [...prev, { id, type, title, message }]);
      setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
      }, 5000);
    },
    []
  );

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const loadGlobalTelemetry = useCallback(async () => {
    try {
      setIsRefreshing(true);
      const [infoRes, schedRes, transferRes, authRes] = await Promise.all([
        commands.getAppInfo(),
        commands.getSchedulerStatus(),
        commands.getTransferStatus(),
        commands.getTelegramAuthStatus(),
      ]);

      if (infoRes.status === "ok") setAppInfo(infoRes.data);
      if (schedRes.status === "ok") setSchedulerStatus(schedRes.data.status);
      if (transferRes.status === "ok") setActiveTransfers(transferRes.data.active_count);
      if (authRes.status === "ok") {
        setAuthStatus(authRes.data);
        setInitError(null);
      } else {
        const errObj = authRes.error as { message?: string; code?: string } | string | undefined;
        const msg =
          typeof errObj === "string"
            ? errObj
            : errObj?.message || errObj?.code || "Failed to retrieve Telegram authentication state";
        setInitError((prev) => prev || msg);
      }
    } catch (err: unknown) {
      console.error("Global telemetry error:", err);
      setInitError((prev) => prev || (err instanceof Error ? err.message : String(err)));
    } finally {
      setIsRefreshing(false);
      setIsAuthInitializing(false);
    }
  }, []);

  useEffect(() => {
    loadGlobalTelemetry();
    // Heartbeat poll for global status bar every 5 seconds
    const interval = setInterval(loadGlobalTelemetry, 5000);
    return () => clearInterval(interval);
  }, [loadGlobalTelemetry]);

  const handleNavigate = (tab: NavTab, profileId?: string, snapshotId?: string) => {
    setContextProfileId(profileId);
    setContextSnapshotId(snapshotId);
    setCurrentTab(tab);
  };

  const handleStartBackupFromProfile = (profile: BackupProfileDto) => {
    setContextProfileId(profile.profile_id);
    setCurrentTab("backups");
  };

  // State 1: Explicit initial loading splash — protects against dashboard flash
  if (isAuthInitializing && !authStatus && !initError) {
    return (
      <div className="app-splash" data-testid="app-splash">
        <div className="splash-card">
          <div className="splash-logo">🛡️</div>
          <h1 className="splash-title">TELEVAULT</h1>
          <p className="splash-subtitle">Initializing secure Telegram vault environment...</p>
          <div className="splash-spinner" data-testid="splash-spinner"></div>
          <div className="splash-attribution">Created by Ankit Sharma</div>
        </div>
      </div>
    );
  }

  // State 2: Initialization failure — safe retry screen without exposing dashboard
  if (initError && !authStatus) {
    return (
      <div className="app-splash" data-testid="app-splash-error">
        <div className="splash-card splash-card-error">
          <div className="splash-logo">⚠️</div>
          <h1 className="splash-title">Initialization Failed</h1>
          <p className="splash-subtitle">Unable to establish connection with TELEVAULT desktop core.</p>
          <div className="splash-error-message">{initError}</div>
          <button
            className="btn btn-primary"
            onClick={() => {
              setInitError(null);
              setIsAuthInitializing(true);
              loadGlobalTelemetry();
            }}
          >
            🔄 Retry Connection
          </button>
          <div className="splash-attribution">Created by Ankit Sharma</div>
        </div>
        <ToastContainer toasts={toasts} onDismiss={dismissToast} />
      </div>
    );
  }

  // State 3: Centralized Authentication Gate — unauthenticated or channel setup required
  if (authStatus && authStatus.state !== "ready") {
    return (
      <div className="app-container" data-testid="welcome-gate-container">
        <WelcomeGate onAuthenticated={loadGlobalTelemetry} authStatus={authStatus} />
        <ToastContainer toasts={toasts} onDismiss={dismissToast} />
      </div>
    );
  }

  const activeMeta = screenTitles[currentTab];
  const telegramUser = authStatus?.account?.username
    ? `@${authStatus.account.username}`
    : authStatus?.account?.first_name;

  return (
    <div className="app-container">
      <Sidebar
        currentTab={currentTab}
        onTabChange={(tab) => {
          setContextProfileId(undefined);
          setContextSnapshotId(undefined);
          setCurrentTab(tab);
        }}
        appVersion={appInfo?.version}
        schedulerStatus={schedulerStatus}
        telegramUser={telegramUser}
        channelTitle={authStatus?.channel?.channel_title}
      />

      <main className="main-content">
        <Header
          title={activeMeta.title}
          subtitle={activeMeta.subtitle}
          schedulerStatus={schedulerStatus}
          activeTransfers={activeTransfers}
          onRefresh={loadGlobalTelemetry}
          isRefreshing={isRefreshing}
        />

        <div className="content-body">
          {currentTab === "dashboard" && (
            <DashboardView
              onNavigate={handleNavigate}
              onStartBackup={handleStartBackupFromProfile}
            />
          )}

          {currentTab === "backups" && (
            <BackupsView
              initialProfileId={contextProfileId}
              onNavigate={handleNavigate}
              onNotify={addToast}
            />
          )}

          {currentTab === "restore" && (
            <RestoreView
              initialProfileId={contextProfileId}
              initialSnapshotId={contextSnapshotId}
              onNotify={addToast}
            />
          )}

          {currentTab === "schedules" && (
            <SchedulesView onNotify={addToast} />
          )}

          {currentTab === "verification" && (
            <VerificationView
              initialProfileId={contextProfileId}
              initialSnapshotId={contextSnapshotId}
              onNavigate={handleNavigate}
              onNotify={addToast}
            />
          )}

          {currentTab === "repair" && (
            <RepairView
              initialProfileId={contextProfileId}
              initialSnapshotId={contextSnapshotId}
              onNotify={addToast}
            />
          )}

          {currentTab === "activity" && (
            <ActivityView onNotify={addToast} />
          )}

          {currentTab === "profiles" && (
            <ProfilesView
              onNavigate={handleNavigate}
              onStartBackup={handleStartBackupFromProfile}
              onNotify={addToast}
            />
          )}

          {currentTab === "retention" && (
            <RetentionView
              initialProfileId={contextProfileId}
              onNotify={addToast}
            />
          )}

          {currentTab === "settings" && (
            <SettingsView onNotify={addToast} />
          )}
        </div>
      </main>

      <ToastContainer toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}
