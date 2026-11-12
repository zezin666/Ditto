import { Button, MessageBar, MessageBarBody, Switch } from "@fluentui/react-components";
import { ArrowResetRegular } from "@fluentui/react-icons";
import { useMemo, useState } from "react";
import { HotkeyRecorder, SettingRow, SettingsCard } from "../components/Controls";
import { useI18n } from "../i18n";
import type { HotkeySettings, SettingsState } from "../types";

function defaultHotkeys(): HotkeySettings {
  return {
    open: "Ctrl+`",
    openAlt: "",
    plainTextPaste: "Ctrl+Shift+V",
    firstTenEnabled: true,
    firstTenChords: Array.from({ length: 10 }, (_, index) =>
      `Ctrl+${index === 9 ? 0 : index + 1}`,
    ),
    copyBuffers: Array.from({ length: 5 }, (_, index) => ({
      slot: index + 1,
      copy: `Ctrl+Alt+${index + 1}`,
      paste: `Alt+${index + 1}`,
      cut: "",
    })),
  };
}

export function HotkeysPage({
  settings,
  onChange,
}: {
  settings: SettingsState;
  onChange: (next: SettingsState) => void;
}) {
  const { t } = useI18n();
  const hotkeys = settings.hotkeys;
  const [conflict, setConflict] = useState<string | null>(null);
  const patch = (value: Partial<HotkeySettings>) =>
    onChange({ ...settings, hotkeys: { ...hotkeys, ...value } });

  const bindingLabels = useMemo(() => {
    const entries: { key: string; label: string; chord: string }[] = [
      { key: "open", label: t("hotkeys.open"), chord: hotkeys.open },
      { key: "openAlt", label: t("hotkeys.openAlt"), chord: hotkeys.openAlt },
      {
        key: "plainTextPaste",
        label: t("hotkeys.plainTextPaste"),
        chord: hotkeys.plainTextPaste,
      },
      ...hotkeys.firstTenChords.map((chord, index) => ({
        key: `firstTen.${index}`,
        label: t("hotkeys.slot", { slot: index + 1 }),
        chord,
      })),
      ...hotkeys.copyBuffers.flatMap((buffer) => [
        {
          key: `copy.${buffer.slot}`,
          label: `${t("hotkeys.slot", { slot: buffer.slot })} ${t("hotkeys.copy")}`,
          chord: buffer.copy,
        },
        {
          key: `paste.${buffer.slot}`,
          label: `${t("hotkeys.slot", { slot: buffer.slot })} ${t("hotkeys.paste")}`,
          chord: buffer.paste,
        },
        {
          key: `cut.${buffer.slot}`,
          label: `${t("hotkeys.slot", { slot: buffer.slot })} ${t("hotkeys.cut")}`,
          chord: buffer.cut,
        },
      ]),
    ];
    return entries.filter((entry) => entry.chord);
  }, [hotkeys, t]);

  const updateBinding = (key: string, value: string) => {
    const duplicate = bindingLabels.find(
      (entry) => entry.key !== key && entry.chord.toLocaleLowerCase() === value.toLocaleLowerCase(),
    );
    setConflict(duplicate && value ? duplicate.label : null);
    if (key === "open" || key === "openAlt" || key === "plainTextPaste") {
      patch({ [key]: value });
      return;
    }
    if (key.startsWith("firstTen.")) {
      const index = Number(key.split(".")[1]);
      const firstTenChords = hotkeys.firstTenChords.map((chord, current) =>
        current === index ? value : chord,
      );
      patch({ firstTenChords });
      return;
    }
    const [field, slotText] = key.split(".");
    const slot = Number(slotText);
    const copyBuffers = hotkeys.copyBuffers.map((buffer) =>
      buffer.slot === slot ? { ...buffer, [field]: value } : buffer,
    );
    patch({ copyBuffers });
  };

  return (
    <div className="page-stack">
      {conflict ? (
        <MessageBar intent="warning" className="inline-message">
          <MessageBarBody>
            {t("hotkeys.conflict", { action: conflict })}
          </MessageBarBody>
        </MessageBar>
      ) : null}

      <SettingsCard title={t("hotkeys.open")}>
        <SettingRow label={t("hotkeys.open")} help={t("hotkeys.globalOpen")}>
          <HotkeyRecorder
            value={hotkeys.open}
            actionLabel={t("hotkeys.open")}
            onChange={(value) => updateBinding("open", value)}
          />
        </SettingRow>
        <SettingRow label={t("hotkeys.openAlt")} help={t("hotkeys.notImplemented")}>
          <HotkeyRecorder
            value={hotkeys.openAlt}
            actionLabel={t("hotkeys.openAlt")}
            disabled
            onChange={(value) => updateBinding("openAlt", value)}
          />
        </SettingRow>
        <SettingRow label={t("hotkeys.plainTextPaste")} help={t("hotkeys.notImplemented")}>
          <HotkeyRecorder
            value={hotkeys.plainTextPaste}
            actionLabel={t("hotkeys.plainTextPaste")}
            disabled
            onChange={(value) => updateBinding("plainTextPaste", value)}
          />
        </SettingRow>
      </SettingsCard>

      <SettingsCard title={t("hotkeys.firstTenGrid")}>
        <SettingRow label={t("hotkeys.firstTen")} help={t("hotkeys.worksWhenOpen")}>
          <Switch
            checked={hotkeys.firstTenEnabled}
            disabled
            onChange={(_, data) => patch({ firstTenEnabled: data.checked })}
            aria-label={t("hotkeys.firstTen")}
          />
        </SettingRow>
        <div className="hotkey-grid">
          {hotkeys.firstTenChords.map((chord, index) => (
            <div className="hotkey-grid-item" key={index}>
              <span className="hotkey-grid-label">{t("hotkeys.slot", { slot: index + 1 })}</span>
              <HotkeyRecorder
                compact
                disabled
                value={chord}
                actionLabel={t("hotkeys.slot", { slot: index + 1 })}
                onChange={(value) => updateBinding(`firstTen.${index}`, value)}
              />
            </div>
          ))}
        </div>
      </SettingsCard>

      <SettingsCard title={t("hotkeys.copyBufferGrid")}>
        <div className="buffer-hotkey-list">
          {hotkeys.copyBuffers.map((buffer) => (
            <div className="buffer-hotkey-row" key={buffer.slot}>
              <strong>{t("hotkeys.slot", { slot: buffer.slot })}</strong>
              <div>
                <span>{t("hotkeys.copy")}</span>
                <HotkeyRecorder
                  compact
                  disabled
                  value={buffer.copy}
                  actionLabel={`${t("hotkeys.slot", { slot: buffer.slot })} ${t("hotkeys.copy")}`}
                  onChange={(value) => updateBinding(`copy.${buffer.slot}`, value)}
                />
              </div>
              <div>
                <span>{t("hotkeys.paste")}</span>
                <HotkeyRecorder
                  compact
                  disabled
                  value={buffer.paste}
                  actionLabel={`${t("hotkeys.slot", { slot: buffer.slot })} ${t("hotkeys.paste")}`}
                  onChange={(value) => updateBinding(`paste.${buffer.slot}`, value)}
                />
              </div>
              <div>
                <span>{t("hotkeys.cut")}</span>
                <HotkeyRecorder
                  compact
                  disabled
                  value={buffer.cut}
                  actionLabel={`${t("hotkeys.slot", { slot: buffer.slot })} ${t("hotkeys.cut")}`}
                  onChange={(value) => updateBinding(`cut.${buffer.slot}`, value)}
                />
              </div>
            </div>
          ))}
        </div>
        <div className="card-actions end">
          <Button
            icon={<ArrowResetRegular />}
            onClick={() => {
              setConflict(null);
              patch(defaultHotkeys());
            }}
          >
            {t("hotkeys.resetAll")}
          </Button>
        </div>
      </SettingsCard>
    </div>
  );
}
