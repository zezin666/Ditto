import { Button, Input, Switch } from "@fluentui/react-components";
import { FolderOpenRegular } from "@fluentui/react-icons";
import { useI18n } from "../i18n";
import { SelectControl, SettingRow, SettingsCard } from "../components/Controls";
import type { LogLevel, SettingsState } from "../types";

export function GeneralPage({
  settings,
  onChange,
  onOpenLogs,
}: {
  settings: SettingsState;
  onChange: (next: SettingsState) => void;
  onOpenLogs: () => void;
}) {
  const { t } = useI18n();
  const general = settings.general;
  const patch = (value: Partial<SettingsState["general"]>) =>
    onChange({ ...settings, general: { ...general, ...value } });

  return (
    <div className="page-stack">
      <SettingsCard title={t("general.startupSection")}>
        <SettingRow label={t("general.runAtStartup")} help={t("general.runAtStartupHelp")}>
          <Switch
            checked={general.runAtStartup}
            onChange={(_, data) => patch({ runAtStartup: data.checked })}
            aria-label={t("general.runAtStartup")}
          />
        </SettingRow>
        <SettingRow label={t("general.startMinimized")} help={t("general.startMinimizedHelp")}>
          <Switch
            checked={general.startMinimized}
            onChange={(_, data) => patch({ startMinimized: data.checked })}
            aria-label={t("general.startMinimized")}
          />
        </SettingRow>
      </SettingsCard>

      <SettingsCard title={t("general.traySection")}>
        <SettingRow label={t("general.showTrayIcon")} help={t("general.showTrayIconHelp")}>
          <Switch
            checked={general.showTrayIcon}
            onChange={(_, data) => patch({ showTrayIcon: data.checked })}
            aria-label={t("general.showTrayIcon")}
          />
        </SettingRow>
        <SettingRow label={t("general.singleInstance")} help={t("general.singleInstanceHelp")}>
          <Switch
            checked={general.singleInstance}
            onChange={(_, data) => patch({ singleInstance: data.checked })}
            aria-label={t("general.singleInstance")}
          />
        </SettingRow>
      </SettingsCard>

      <SettingsCard title={t("general.languageSection")}>
        <SettingRow label={t("general.language")} help={t("general.languageHelp")}>
          <SelectControl
            value={general.language}
            options={[
              { value: "en-US", label: t("general.english") },
              { value: "zh-CN", label: t("general.chinese") },
            ]}
            onChange={(language) => patch({ language })}
            ariaLabel={t("general.language")}
          />
        </SettingRow>
        <SettingRow label={t("general.startupMessage")} help={t("general.startupMessageHelp")} align="start">
          <Input
            value={general.startupMessage}
            placeholder={t("general.startupMessagePlaceholder")}
            onChange={(_, data) => patch({ startupMessage: data.value })}
            aria-label={t("general.startupMessage")}
          />
        </SettingRow>
      </SettingsCard>

      <SettingsCard title={t("general.loggingSection")}>
        <SettingRow label={t("general.logLevel")} help={t("general.logLevelHelp")}>
          <SelectControl<LogLevel>
            value={general.logLevel}
            options={[
              { value: "error", label: t("log.error") },
              { value: "warn", label: t("log.warn") },
              { value: "info", label: t("log.info") },
              { value: "debug", label: t("log.debug") },
              { value: "trace", label: t("log.trace") },
            ]}
            onChange={(logLevel) => patch({ logLevel })}
            ariaLabel={t("general.logLevel")}
          />
        </SettingRow>
        <SettingRow label={t("general.openLogs")}>
          <Button icon={<FolderOpenRegular />} onClick={onOpenLogs}>
            {t("general.openLogs")}
          </Button>
        </SettingRow>
        <SettingRow label={t("general.checkUpdates")} help={t("general.updatesComingLater")}>
          <Switch
            checked={false}
            disabled
            aria-label={t("general.checkUpdates")}
          />
        </SettingRow>
      </SettingsCard>
    </div>
  );
}
