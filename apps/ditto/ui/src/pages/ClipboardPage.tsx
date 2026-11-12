import { Input, Switch } from "@fluentui/react-components";
import { SettingRow, SettingsCard } from "../components/Controls";
import { useI18n } from "../i18n";
import type { SettingsState } from "../types";

function numberValue(value: string, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallback;
}

export function ClipboardPage({
  settings,
  onChange,
}: {
  settings: SettingsState;
  onChange: (next: SettingsState) => void;
}) {
  const { t } = useI18n();
  const clipboard = settings.clipboard;
  const patch = (value: Partial<SettingsState["clipboard"]>) =>
    onChange({ ...settings, clipboard: { ...clipboard, ...value } });

  return (
    <div className="page-stack">
      <SettingsCard title={t("clipboard.limitsSection")}>
        <SettingRow label={t("clipboard.maxEntries")} help={t("clipboard.limitsHelp")}>
          <Input
            type="number"
            min={1}
            max={1_000_000}
            value={String(clipboard.maxEntries)}
            onChange={(_, data) =>
              patch({ maxEntries: numberValue(data.value, clipboard.maxEntries) })
            }
            aria-label={t("clipboard.maxEntries")}
          />
        </SettingRow>
        <SettingRow label={t("clipboard.expireDays")}>
          <Input
            type="number"
            min={0}
            max={3650}
            value={String(clipboard.expireDays)}
            onChange={(_, data) =>
              patch({ expireDays: numberValue(data.value, clipboard.expireDays) })
            }
            aria-label={t("clipboard.expireDays")}
          />
        </SettingRow>
        <SettingRow label={t("clipboard.maxClipBytes")}>
          <Input
            type="number"
            min={1024}
            step={1024}
            value={String(clipboard.maxClipBytes)}
            onChange={(_, data) =>
              patch({ maxClipBytes: numberValue(data.value, clipboard.maxClipBytes) })
            }
            aria-label={t("clipboard.maxClipBytes")}
          />
        </SettingRow>
      </SettingsCard>

      <SettingsCard title={t("clipboard.formatsSection")}>
        <SettingRow label={t("clipboard.ignoreFormats")} help={t("clipboard.ignoreFormatsHelp")} align="start">
          <Input
            value={clipboard.ignoreFormats.join(", ")}
            onChange={(_, data) =>
              patch({
                ignoreFormats: data.value
                  .split(",")
                  .map((value) => value.trim())
                  .filter(Boolean),
              })
            }
            aria-label={t("clipboard.ignoreFormats")}
          />
        </SettingRow>
        <SettingRow label={t("clipboard.captureFiles")}>
          <Switch
            checked={clipboard.captureFiles}
            onChange={(_, data) => patch({ captureFiles: data.checked })}
            aria-label={t("clipboard.captureFiles")}
          />
        </SettingRow>
        <SettingRow label={t("clipboard.captureImages")}>
          <Switch
            checked={clipboard.captureImages}
            onChange={(_, data) => patch({ captureImages: data.checked })}
            aria-label={t("clipboard.captureImages")}
          />
        </SettingRow>
      </SettingsCard>

      <SettingsCard title={t("clipboard.behaviorSection")}>
        <SettingRow label={t("clipboard.allowDuplicates")}>
          <Switch
            checked={clipboard.allowDuplicates}
            onChange={(_, data) => patch({ allowDuplicates: data.checked })}
            aria-label={t("clipboard.allowDuplicates")}
          />
        </SettingRow>
        <SettingRow
          label={t("clipboard.recordSourceApp")}
          help={t("clipboard.recordSourceAppHelp")}
        >
          <Switch
            checked={clipboard.recordSourceApp}
            onChange={(_, data) => patch({ recordSourceApp: data.checked })}
            aria-label={t("clipboard.recordSourceApp")}
          />
        </SettingRow>
      </SettingsCard>
    </div>
  );
}
