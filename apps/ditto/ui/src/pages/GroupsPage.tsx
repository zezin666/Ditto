import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  Input,
  Switch,
} from "@fluentui/react-components";
import {
  AddRegular,
  DeleteRegular,
  EditRegular,
  FolderRegular,
  FolderOpenRegular,
} from "@fluentui/react-icons";
import { useState, type CSSProperties } from "react";
import { commands } from "../api/commands";
import {
  ConfirmDialog,
  EmptyState,
  SelectControl,
  SettingRow,
  SettingsCard,
} from "../components/Controls";
import { useI18n } from "../i18n";
import type { GroupNode, Notify, SettingsState } from "../types";

interface GroupDialogState {
  mode: "create" | "rename";
  open: boolean;
  name: string;
  parentId: number | null;
}

function groupDepth(group: GroupNode, groups: GroupNode[]) {
  let depth = 0;
  let parentId = group.parentId;
  const visited = new Set<number>();
  while (parentId !== null && depth < 32 && !visited.has(parentId)) {
    visited.add(parentId);
    const parent = groups.find((candidate) => candidate.id === parentId);
    if (!parent) {
      break;
    }
    depth += 1;
    parentId = parent.parentId;
  }
  return depth;
}

export function GroupsPage({
  settings,
  groups,
  onChange,
  onRefresh,
  notify,
}: {
  settings: SettingsState;
  groups: GroupNode[];
  onChange: (next: SettingsState) => void;
  onRefresh: () => Promise<void>;
  notify: Notify;
}) {
  const { t } = useI18n();
  const [selectedId, setSelectedId] = useState<number | null>(groups[0]?.id ?? null);
  const [dialog, setDialog] = useState<GroupDialogState>({
    mode: "create",
    open: false,
    name: "",
    parentId: null,
  });
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const selected = groups.find((group) => group.id === selectedId) ?? null;
  const patchGroups = (value: Partial<SettingsState["groups"]>) =>
    onChange({ ...settings, groups: { ...settings.groups, ...value } });
  const groupOptions = [
    { value: "none", label: t("groups.root") },
    ...groups
      .filter((group) => group.id !== selectedId)
      .map((group) => ({ value: String(group.id), label: group.name })),
  ];

  const submitDialog = async () => {
    const name = dialog.name.trim();
    if (!name) {
      return;
    }
    setBusy(true);
    try {
      if (dialog.mode === "create") {
        const group = await commands.createGroup(name, dialog.parentId);
        setSelectedId(group.id);
        notify("success", t("toast.groupCreated"));
      } else if (selected) {
        await commands.renameGroup(selected.id, name);
        notify("success", t("toast.groupRenamed"));
      }
      setDialog((current) => ({ ...current, open: false, name: "" }));
      await onRefresh();
    } catch {
      notify("error", t(dialog.mode === "create" ? "toast.groupCreateFailed" : "toast.groupRenameFailed"));
    } finally {
      setBusy(false);
    }
  };

  const deleteSelected = async () => {
    if (!selected) {
      return;
    }
    setBusy(true);
    try {
      await commands.deleteGroup(selected.id);
      setSelectedId(null);
      setDeleteOpen(false);
      notify("success", t("toast.groupDeleted"));
      await onRefresh();
    } catch {
      notify("error", t("toast.groupDeleteFailed"));
    } finally {
      setBusy(false);
    }
  };

  const moveSelected = async (parentId: number | null) => {
    if (!selected) {
      return;
    }
    setBusy(true);
    try {
      await commands.moveGroup(selected.id, parentId);
      notify("success", t("toast.groupMoved"));
      await onRefresh();
    } catch {
      notify("error", t("toast.groupMoveFailed"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page-stack">
      <SettingsCard
        title={t("groups.treeSection")}
        className="groups-card"
      >
        <div className="card-toolbar">
          <Button
            appearance="primary"
            icon={<AddRegular />}
            onClick={() => setDialog({ mode: "create", open: true, name: "", parentId: null })}
          >
            {t("groups.newGroup")}
          </Button>
        </div>
        {groups.length === 0 ? (
          <EmptyState title={t("groups.empty")} />
        ) : (
          <div className="group-tree" role="tree" aria-label={t("groups.treeSection")}>
            {groups.map((group) => {
              const depth = groupDepth(group, groups);
              return (
                <button
                  type="button"
                  role="treeitem"
                  aria-selected={group.id === selectedId}
                  aria-level={depth + 1}
                  className={`group-row ${group.id === selectedId ? "group-row-selected" : ""}`}
                  key={group.id}
                  onClick={() => setSelectedId(group.id)}
                  style={{ "--group-depth": depth } as CSSProperties}
                >
                  {group.id === selectedId ? <FolderOpenRegular /> : <FolderRegular />}
                  <span className="group-name">{group.name}</span>
                  <span className="group-count">{group.count}</span>
                </button>
              );
            })}
          </div>
        )}
        <div className="card-actions">
          <Button
            icon={<EditRegular />}
            disabled={!selected}
            onClick={() =>
              selected
                ? setDialog({
                    mode: "rename",
                    open: true,
                    name: selected.name,
                    parentId: selected.parentId,
                  })
                : undefined
            }
          >
            {t("common.rename")}
          </Button>
          <Button
            icon={<DeleteRegular />}
            disabled={!selected}
            onClick={() => setDeleteOpen(true)}
          >
            {t("common.delete")}
          </Button>
        </div>
        {selected ? (
          <div className="move-row">
            <SelectControl
              label={t("groups.moveTo")}
              value={selected.parentId === null ? "none" : String(selected.parentId)}
              options={groupOptions}
              onChange={(value) => void moveSelected(value === "none" ? null : Number(value))}
              disabled={busy}
            />
          </div>
        ) : null}
      </SettingsCard>

      <SettingsCard title={t("groups.defaultGroup")}>
        <SettingRow label={t("groups.defaultGroup")} help={t("groups.defaultGroupHelp")}>
          <SelectControl
            value={
              settings.groups.defaultGroupId === null
                ? "none"
                : String(settings.groups.defaultGroupId)
            }
            options={[
              { value: "none", label: t("history.ungrouped") },
              ...groups.map((group) => ({ value: String(group.id), label: group.name })),
            ]}
            onChange={(value) =>
              patchGroups({
                defaultGroupId: value === "none" ? null : Number(value),
              })
            }
            ariaLabel={t("groups.defaultGroup")}
          />
        </SettingRow>
        <SettingRow label={t("groups.promptForName")}>
          <Switch
            checked={settings.groups.promptForName}
            onChange={(_, data) => patchGroups({ promptForName: data.checked })}
            aria-label={t("groups.promptForName")}
          />
        </SettingRow>
        <SettingRow label={t("groups.showGroupedInMain")}>
          <Switch
            checked={settings.groups.showGroupedInMain}
            onChange={(_, data) => patchGroups({ showGroupedInMain: data.checked })}
            aria-label={t("groups.showGroupedInMain")}
          />
        </SettingRow>
      </SettingsCard>

      <Dialog
        open={dialog.open}
        onOpenChange={(_, data) =>
          setDialog((current) => ({ ...current, open: data.open }))
        }
      >
        <DialogSurface>
          <DialogBody>
            <DialogTitle>
              {dialog.mode === "create" ? t("groups.newGroup") : t("common.rename")}
            </DialogTitle>
            <DialogContent>
              <Field label={t("groups.groupName")}>
                <Input
                  autoFocus
                  value={dialog.name}
                  onChange={(_, data) =>
                    setDialog((current) => ({ ...current, name: data.value }))
                  }
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      void submitDialog();
                    }
                  }}
                />
              </Field>
              {dialog.mode === "create" ? (
                <SelectControl
                  label={t("groups.parentGroup")}
                  value={dialog.parentId === null ? "none" : String(dialog.parentId)}
                  options={[
                    { value: "none", label: t("groups.root") },
                    ...groups.map((group) => ({ value: String(group.id), label: group.name })),
                  ]}
                  onChange={(value) =>
                    setDialog((current) => ({
                      ...current,
                      parentId: value === "none" ? null : Number(value),
                    }))
                  }
                />
              ) : null}
            </DialogContent>
            <DialogActions>
              <Button
                appearance="secondary"
                onClick={() => setDialog((current) => ({ ...current, open: false }))}
              >
                {t("common.cancel")}
              </Button>
              <Button
                appearance="primary"
                disabled={busy || !dialog.name.trim()}
                onClick={() => void submitDialog()}
              >
                {dialog.mode === "create" ? t("common.create") : t("common.rename")}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <ConfirmDialog
        open={deleteOpen}
        title={t("groups.deleteTitle", { name: selected?.name ?? "" })}
        body={t("groups.deleteBody")}
        confirmLabel={t("common.delete")}
        cancelLabel={t("common.cancel")}
        danger
        busy={busy}
        onOpenChange={setDeleteOpen}
        onConfirm={() => void deleteSelected()}
      />
    </div>
  );
}
