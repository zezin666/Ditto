import { Button, Input, Switch } from "@fluentui/react-components";
import { AddRegular, DismissRegular } from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { commands } from "../api/commands";
import { SettingRow, SettingsCard } from "../components/Controls";
import { useI18n } from "../i18n";
import type { CopyBuffer, Notify, SettingsState } from "../types";

function BufferSlot({
  buffer,
  selectedClipId,
  onRefresh,
  notify,
}: {
  buffer: CopyBuffer;
  selectedClipId: number | null;
  onRefresh: () => Promise<void>;
  notify: Notify;
}) {
  const { t } = useI18n();
  const [name, setName] = useState(buffer.name);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setName(buffer.name);
  }, [buffer.name]);

  const save = async (changes: Partial<Pick<CopyBuffer, "name" | "enabled" | "clipId">>) => {
    setBusy(true);
    try {
      await commands.setCopyBuffer({
        slot: buffer.slot,
        name: changes.name ?? buffer.name,
        enabled: changes.enabled ?? buffer.enabled,
        clipId: Object.hasOwn(changes, "clipId") ? (changes.clipId ?? null) : buffer.clipId,
      });
      notify("success", t("toast.bufferUpdated"));
      await onRefresh();
    } catch {
      notify("error", t("toast.bufferUpdateFailed"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="buffer-slot">
      <div className="buffer-slot-heading">
        <h3>{t("copyBuffers.slot", { slot: buffer.slot })}</h3>
        <Switch
          checked={buffer.enabled}
          label={t("copyBuffers.enabled")}
          onChange={(_, data) => void save({ enabled: data.checked })}
          disabled={busy}
        />
      </div>
      <SettingRow label={t("copyBuffers.slotName")}>
        <Input
          value={name}
          onChange={(_, data) => setName(data.value)}
          onBlur={() => {
            if (name !== buffer.name) {
              void save({ name });
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && name !== buffer.name) {
              void save({ name });
            }
          }}
          aria-label={t("copyBuffers.slotName")}
        />
      </SettingRow>
      <div className="buffer-clip">
        <span className="setting-label">{t("copyBuffers.currentClip")}</span>
        <p>{buffer.clipPreview || t("copyBuffers.noClip")}</p>
      </div>
      <div className="card-actions">
        <Button
          icon={<AddRegular />}
          disabled={busy || selectedClipId === null}
          onClick={() => void save({ clipId: selectedClipId })}
        >
          {t("copyBuffers.assignSelected")}
        </Button>
        <Button
          icon={<DismissRegular />}
          disabled={busy || buffer.clipId === null}
          onClick={() => void save({ clipId: null })}
        >
          {t("copyBuffers.clearClip")}
        </Button>
      </div>
      {selectedClipId === null ? <p className="setting-help">{t("copyBuffers.noSelection")}</p> : null}
    </section>
  );
}

export function CopyBuffersPage({
  settings,
  buffers,
  selectedClipId,
  onChange,
  onRefresh,
  notify,
}: {
  settings: SettingsState;
  buffers: CopyBuffer[];
  selectedClipId: number | null;
  onChange: (next: SettingsState) => void;
  onRefresh: () => Promise<void>;
  notify: Notify;
}) {
  const { t } = useI18n();
  return (
    <div className="page-stack">
      <SettingsCard title={t("copyBuffers.hotkeys")}>
        <SettingRow label={t("copyBuffers.playSound")} help={t("common.comingLater")}>
          <Switch
            checked={settings.copyBuffers.playSound}
            disabled
            onChange={(_, data) =>
              onChange({
                ...settings,
                copyBuffers: { ...settings.copyBuffers, playSound: data.checked },
              })
            }
            aria-label={t("copyBuffers.playSound")}
          />
        </SettingRow>
        <p className="setting-help standalone-help">{t("copyBuffers.hotkeyHelp")}</p>
      </SettingsCard>
      <div className="buffer-grid">
        {buffers.map((buffer) => (
          <BufferSlot
            key={buffer.slot}
            buffer={buffer}
            selectedClipId={selectedClipId}
            onRefresh={onRefresh}
            notify={notify}
          />
        ))}
      </div>
    </div>
  );
}
