import React, { useState } from "react";
import { FolderPlus } from "lucide-react";
import { Modal } from "../ui/Modal";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";

export interface CreateFolderModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreate: (name: string) => void;
}

export const CreateFolderModal: React.FC<CreateFolderModalProps> = ({
  isOpen,
  onClose,
  onCreate,
}) => {
  const [folderName, setFolderName] = useState("");
  const [error, setError] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!folderName.trim()) {
      setError("Folder name cannot be empty");
      return;
    }
    onCreate(folderName.trim());
    setFolderName("");
    setError("");
    onClose();
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Create New Folder"
      subtitle="Organize your files inside your personal cloud drive."
      icon={<FolderPlus size={18} />}
      maxWidth="420px"
    >
      <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: "18px" }}>
        <Input
          label="Folder Name"
          placeholder="e.g. Financial Reports, Projects, Photos"
          value={folderName}
          onChange={(e) => {
            setFolderName(e.target.value);
            setError("");
          }}
          error={error}
          autoFocus
        />

        <div style={{ display: "flex", justifyContent: "flex-end", gap: "10px" }}>
          <Button type="button" variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant="primary">
            Create Folder
          </Button>
        </div>
      </form>
    </Modal>
  );
};
