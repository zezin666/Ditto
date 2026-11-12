import {
  Button,
  MessageBar,
  MessageBarBody,
  ProgressBar,
  Tab,
  TabList,
  type SelectTabData,
} from "@fluentui/react-components";
import {
  ArrowExportRegular,
  ArrowImportRegular,
  ArchiveRegular,
  ArrowSyncRegular,
  DatabaseRegular,
  FolderOpenRegular,
  HardDriveRegular,
  HistoryRegular,
} from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { commands } from "../api/commands";
import {
  ConfirmDialog,
  formatBytes,
  SettingRow,
  SettingsCard,
} from "../components/Controls";
import { useI18n } from "../i18n";
import type {
  CopyBuffer,
  GroupNode,
  MigrationStatus,
  Notify,
  SettingsState,
} from "../types";
import { HistoryManager } from "./HistoryManager";

type DataOperation =
  | "backup"
  | "restore"
  | "optimize"
  | "export"
  | "import"
  | null;

const confirmationKeys: Record<
  Exclude<DataOperation, null>,
  { title: string; body: string; success: string; failure: string }
> = {
  backup: {
    title: "data.confirmBackupTitle",
    body: "data.confirmBackupBody",
    success: "toast.backupComplete",
    failure: "toast.backupFailed",
  },
  restore: {
    title: "data.confirmRestoreTitle",
    body: "data.confirmRestoreBody",
    success: "toast.restoreComplete",
    failure: "toast.restoreFailed",
  },
  optimize: {
    title: "data.confirmOptimizeTitle",
    body: "data.confirmOptimizeBody",
    success: "toast.optimizeComplete",
    failure: "toast.optimizeFailed",
  },
  export: {
    title: "data.confirmExportTitle",
    body: "data.confirmExportBody",
    success: "toast.exportComplete",
    failure: "toast.exportFailed",
  },
  import: {
    title: "data.confirmImportTitle",
    body: "data.confirmImportBody",
    success: "toast.importComplete",
    failure: "toast.importFailed",
  },
};

export function DataPage({
  settings,
  groups,
  buffers,
  selectedClipId,
  onSelectedClipChange,
  onDataChanged,
  notify,
}: {
  settings: SettingsState;
  groups: GroupNode[];
  buffers: CopyBuffer[];
  selectedClipId: number | null;
  onSelectedClipChange: (id: number | null) => void;
  onDataChanged: () => Promise<void>;
  notify: Notify;
}) {
  const { t, locale } = useI18n();
  const [tab, setTab] = useState<"storage" | "history">("storage");
  const [migration, setMigration] = useState<MigrationStatus | null>(null);
  const [operation, setOperation] = useState<DataOperation>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    commands
      .getMigrationStatus()
      .then(setMigration)
      .catch(() => setMigration(null));
  }, []);

  const onTabSelect = (_: unknown, data: SelectTabData) => {
    setTab(data.value as "storage" | "history");
  };

  const runOperation = async () => {
    if (!operation) {
      return;
    }
    const keys = confirmationKeys[operation];
    setBusy(true);
    try {
      if (operation === "backup") {
        await commands.backupDb();
      } else if (operation === "restore") {
        await commands.restoreDb();
      } else if (operation === "optimize") {
        await commands.optimizeStorage();
      } else if (operation === "export") {
        await commands.exportData();
      } else if (operation === "import") {
        await commands.importData();
      }
      notify("success", t(keys.success));
      setOperation(null);
      await onDataChanged();
    } catch {
      notify("error", t(keys.failure));
    } finally {
      setBusy(false);
    }
  };

  const openFolder = async () => {
    try {
      await commands.openDataFolder();
      notify("success", t("toast.actionComplete"));
    } catch {
      notify("error", t("toast.openFolderFailed"));
    }
  };

  const confirmation = operation ? confirmationKeys[operation] : null;
  const migrationLabel = migration?.inProgress
    ? t("data.migrationRunning")
    : migration?.completed
      ? t("data.migrationCompleted")
      : t("data.migrationNotStarted");

  return (
    <div className="page-stack">
      <TabList selectedValue={tab} onTabSelect={onTabSelect} aria-label={t("page.data.title")}>
        <Tab value="storage" data-tab="storage" icon={<HardDriveRegular />}>
          {t("data.tabStorage")}
        </Tab>
        <Tab value="history" data-tab="history" icon={<HistoryRegular />}>
          {t("data.tabHistory")}
        </Tab>
      </TabList>

      {tab === "history" ? (
        <HistoryManager
          groups={groups}
          buffers={buffers}
          selectedClipId={selectedClipId}
          onSelectedClipChange={onSelectedClipChange}
          onDataChanged={onDataChanged}
          notify={notify}
        />
      ) : (
        <>
          <SettingsCard title={t("data.tabStorage")}>
            <SettingRow label={t("data.root")} align="start">
              <code className="path-value">{settings.data.dataRoot}</code>
            </SettingRow>
            <SettingRow label={t("data.mode")}>
              <span>{settings.data.portable ? t("data.portable") : t("data.installed")}</span>
            </SettingRow>
            <SettingRow label={t("data.databaseSize")}>
              <span>{formatBytes(settings.data.databaseBytes, t)}</span>
            </SettingRow>
            <SettingRow label={t("data.lastBackup")}>
              <span>
                {settings.data.lastBackupAt
                  ? new Intl.DateTimeFormat(locale, {
                      dateStyle: "medium",
                      timeStyle: "short",
                    }).format(new Date(settings.data.lastBackupAt))
                  : t("common.none")}
              </span>
            </SettingRow>
            <div className="card-actions">
              <Button icon={<FolderOpenRegular />} onClick={() => void openFolder()}>
                {t("data.openFolder")}
              </Button>
            </div>
          </SettingsCard>

          <SettingsCard title={t("data.actionsSection")}>
            <div className="data-action-grid">
              <Button icon={<ArchiveRegular />} onClick={() => setOperation("backup")}>
                {t("data.backupNow")}
              </Button>
              <Button icon={<ArrowSyncRegular />} onClick={() => setOperation("restore")}>
                {t("data.restore")}
              </Button>
              <Button icon={<DatabaseRegular />} onClick={() => setOperation("optimize")}>
                {t("data.optimize")}
              </Button>
              <Button icon={<ArrowExportRegular />} onClick={() => setOperation("export")}>
                {t("data.export")}
              </Button>
              <Button icon={<ArrowImportRegular />} onClick={() => setOperation("import")}>
                {t("data.import")}
              </Button>
            </div>
            <MessageBar intent="warning" className="inline-message">
              <MessageBarBody>{t("data.unencryptedWarning")}</MessageBarBody>
            </MessageBar>
          </SettingsCard>

          <SettingsCard title={t("data.migrationSection")}>
            <SettingRow label={t("data.migrationStatus")}>
              <span>{migrationLabel}</span>
            </SettingRow>
            {migration?.inProgress ? (
              <ProgressBar
                value={migration.total ? migration.processed / migration.total : 0}
                aria-label={t("data.migrationRunning")}
              />
            ) : null}
            <p className="setting-help standalone-help">{t("data.migrationMock")}</p>
            <div className="card-actions">
              <Button disabled>{t("data.migrationEntry")}</Button>
            </div>
          </SettingsCard>
        </>
      )}

      <ConfirmDialog
        open={operation !== null}
        title={confirmation ? t(confirmation.title) : ""}
        body={confirmation ? t(confirmation.body) : ""}
        confirmLabel={t("common.confirm")}
        cancelLabel={t("common.cancel")}
        danger={operation !== "backup" && operation !== "export"}
        busy={busy}
        onOpenChange={(open) => {
          if (!open) {
            setOperation(null);
          }
        }}
        onConfirm={() => void runOperation()}
      />
    </div>
  );
}
