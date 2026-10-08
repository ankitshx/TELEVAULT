import { useEffect, useState, useCallback } from "react";
import {
  commands,
  BackupProfileDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";
import { Modal } from "../components/Modal";
import { NavTab } from "../types";

interface ProfilesViewProps {
  onNavigate: (tab: NavTab, profileId?: string) => void;
  onStartBackup: (profile: BackupProfileDto) => void;
  onNotify: (type: "success" | "error" | "warning" | "info", title: string, message: string) => void;
}

export function ProfilesView({
  onNavigate,
  onStartBackup,
  onNotify,
}: ProfilesViewProps) {
  const [profiles, setProfiles] = useState<BackupProfileDto[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // Create Profile Modal
  const [createModalOpen, setCreateModalOpen] = useState<boolean>(false);
  const [newProfileId, setNewProfileId] = useState<string>("");
  const [newName, setNewName] = useState<string>("");
  const [newDescription, setNewDescription] = useState<string>("");
  const [newSourcePath, setNewSourcePath] = useState<string>("");

  // Edit Profile Modal
  const [editModalOpen, setEditModalOpen] = useState<boolean>(false);
  const [editingProfile, setEditingProfile] = useState<BackupProfileDto | null>(null);
  const [editName, setEditName] = useState<string>("");
  const [editDescription, setEditDescription] = useState<string>("");
  const [editSourcePath, setEditSourcePath] = useState<string>("");
  const [editEnabled, setEditEnabled] = useState<boolean>(true);

  const [isSubmitting, setIsSubmitting] = useState<boolean>(false);

  const loadProfiles = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await commands.listBackupProfiles();
      if (res.status === "ok") {
        setProfiles(res.data);
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  const handleOpenCreate = () => {
    const id = `profile-${Date.now().toString(36)}`;
    setNewProfileId(id);
    setNewName("");
    setNewDescription("");
    setNewSourcePath("");
    setCreateModalOpen(true);
  };

  const handleCreateProfile = async () => {
    if (!newName.trim() || !newSourcePath.trim()) {
      setError("Name and source path are required.");
      return;
    }

    try {
      setIsSubmitting(true);
      setError(null);

      const res = await commands.createBackupProfile({
        profile_id: newProfileId.trim(),
        name: newName.trim(),
        description: newDescription.trim() ? newDescription.trim() : null,
        source_path: newSourcePath.trim(),
      });

      if (res.status === "ok") {
        onNotify("success", "Profile Created", `Created backup profile "${res.data.name}".`);
        setCreateModalOpen(false);
        loadProfiles();
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleOpenEdit = (p: BackupProfileDto) => {
    setEditingProfile(p);
    setEditName(p.name);
    setEditDescription(p.description || "");
    setEditSourcePath(p.source_path);
    setEditEnabled(p.enabled);
    setEditModalOpen(true);
  };

  const handleUpdateProfile = async () => {
    if (!editingProfile) return;

    try {
      setIsSubmitting(true);
      setError(null);

      const res = await commands.updateBackupProfile({
        profile_id: editingProfile.profile_id,
        name: editName.trim() ? editName.trim() : null,
        description: editDescription.trim() ? editDescription.trim() : null,
        source_path: editSourcePath.trim() ? editSourcePath.trim() : null,
        enabled: editEnabled,
      });

      if (res.status === "ok") {
        onNotify("success", "Profile Updated", `Updated backup profile "${res.data.name}".`);
        setEditModalOpen(false);
        loadProfiles();
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleDeleteProfile = async (profileId: string, name: string) => {
    if (!confirm(`Are you sure you want to delete profile "${name}"? This removes the local profile and cascade deletes local schedules and retention rules. (Remote cloud chunks remain untouched).`)) {
      return;
    }

    try {
      const res = await commands.deleteBackupProfile(profileId);
      if (res.status === "ok") {
        onNotify("success", "Profile Deleted", `Deleted profile "${name}".`);
        loadProfiles();
      } else {
        onNotify("error", "Failed to Delete", res.error.message);
      }
    } catch (err: unknown) {
      onNotify("error", "Error", String(err));
    }
  };

  if (loading && profiles.length === 0) {
    return <LoadingSpinner message="Loading backup profiles..." />;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Profile Management Error" onDismiss={() => setError(null)} />}

      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Configured Backup Profiles</div>
            <div className="card-subtitle">
              Manage local source directories, encryption, compression, and policies
            </div>
          </div>
          <button className="btn btn-primary btn-sm" onClick={handleOpenCreate}>
            + Create New Profile
          </button>
        </div>

        {profiles.length === 0 ? (
          <EmptyState
            title="No Profiles Configured"
            description="Create a backup profile by selecting a local folder to begin backing up to Telegram Cloud."
            action={
              <button className="btn btn-primary btn-sm" onClick={handleOpenCreate}>
                Create First Profile
              </button>
            }
          />
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Name & ID</th>
                  <th>Source Directory</th>
                  <th>Status</th>
                  <th>Created</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {profiles.map((p) => (
                  <tr key={p.profile_id}>
                    <td>
                      <div style={{ fontWeight: 600 }}>{p.name}</div>
                      <div className="mono" style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
                        {p.profile_id}
                      </div>
                    </td>
                    <td className="mono" style={{ fontSize: "0.8rem", maxWidth: "260px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {p.source_path}
                    </td>
                    <td>
                      <StatusBadge
                        status={p.enabled ? "Active" : "Disabled"}
                        label={p.enabled ? "Active" : "Disabled"}
                      />
                    </td>
                    <td style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                      {new Date(p.created_at).toLocaleDateString()}
                    </td>
                    <td>
                      <div style={{ display: "flex", gap: "0.4rem" }}>
                        <button
                          className="btn btn-primary btn-sm"
                          onClick={() => onStartBackup(p)}
                          title="Run backup now for this profile"
                        >
                          Run
                        </button>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => handleOpenEdit(p)}
                          title="Edit profile settings"
                        >
                          Edit
                        </button>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => onNavigate("retention", p.profile_id)}
                          title="Manage retention policy"
                        >
                          Retention
                        </button>
                        <button
                          className="btn btn-danger btn-sm"
                          onClick={() => handleDeleteProfile(p.profile_id, p.name)}
                          title="Delete profile"
                        >
                          Delete
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Create Profile Modal */}
      <Modal
        isOpen={createModalOpen}
        onClose={() => !isSubmitting && setCreateModalOpen(false)}
        title="Create New Backup Profile"
        large={true}
        footer={
          <div style={{ display: "flex", gap: "0.75rem" }}>
            <button className="btn btn-secondary" onClick={() => setCreateModalOpen(false)} disabled={isSubmitting}>
              Cancel
            </button>
            <button className="btn btn-primary" onClick={handleCreateProfile} disabled={isSubmitting}>
              {isSubmitting ? "Creating..." : "Create Profile"}
            </button>
          </div>
        }
      >
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <div className="grid-2">
            <div className="form-group">
              <label>Profile Identifier (Internal ID):</label>
              <input type="text" value={newProfileId} onChange={(e) => setNewProfileId(e.target.value)} />
            </div>

            <div className="form-group">
              <label>Profile Display Name:</label>
              <input
                type="text"
                placeholder="e.g. Work Documents"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
              />
            </div>
          </div>

          <div className="form-group">
            <label>Source Directory Path (Local folder to back up):</label>
            <input
              type="text"
              placeholder="e.g. D:\\Documents\\Important"
              value={newSourcePath}
              onChange={(e) => setNewSourcePath(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label>Description (Optional):</label>
            <input
              type="text"
              placeholder="Brief description..."
              value={newDescription}
              onChange={(e) => setNewDescription(e.target.value)}
            />
          </div>
        </div>
      </Modal>

      {/* Edit Profile Modal */}
      <Modal
        isOpen={editModalOpen}
        onClose={() => !isSubmitting && setEditModalOpen(false)}
        title={`Edit Profile: ${editingProfile?.name || ""}`}
        footer={
          <div style={{ display: "flex", gap: "0.75rem" }}>
            <button className="btn btn-secondary" onClick={() => setEditModalOpen(false)} disabled={isSubmitting}>
              Cancel
            </button>
            <button className="btn btn-primary" onClick={handleUpdateProfile} disabled={isSubmitting}>
              {isSubmitting ? "Saving..." : "Save Changes"}
            </button>
          </div>
        }
      >
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <div className="form-group">
            <label>Display Name:</label>
            <input type="text" value={editName} onChange={(e) => setEditName(e.target.value)} />
          </div>

          <div className="form-group">
            <label>Source Directory Path:</label>
            <input type="text" value={editSourcePath} onChange={(e) => setEditSourcePath(e.target.value)} />
          </div>

          <div className="form-group">
            <label>Description:</label>
            <input type="text" value={editDescription} onChange={(e) => setEditDescription(e.target.value)} />
          </div>

          <div className="form-group">
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={editEnabled}
                onChange={(e) => setEditEnabled(e.target.checked)}
              />
              <span>Profile Enabled</span>
            </label>
          </div>
        </div>
      </Modal>
    </div>
  );
}
