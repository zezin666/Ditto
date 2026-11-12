import { Badge, Slider, Switch } from "@fluentui/react-components";
import { SelectControl, SettingRow, SettingsCard } from "../components/Controls";
import { useI18n } from "../i18n";
import type {
  Density,
  PreviewMode,
  SettingsState,
  ThemeChoice,
} from "../types";

export function AppearancePage({
  settings,
  onChange,
}: {
  settings: SettingsState;
  onChange: (next: SettingsState) => void;
}) {
  const { t } = useI18n();
  const appearance = settings.appearance;
  const patch = (value: Partial<SettingsState["appearance"]>) =>
    onChange({ ...settings, appearance: { ...appearance, ...value } });

  return (
    <div className="page-stack">
      <SettingsCard title={t("appearance.themeSection")}>
        <SettingRow label={t("appearance.theme")} help={t("appearance.themeHelp")}>
          <SelectControl<ThemeChoice>
            value={appearance.theme}
            options={[
              { value: "system", label: t("appearance.themeSystem") },
              { value: "light", label: t("appearance.themeLight") },
              { value: "dark", label: t("appearance.themeDark") },
              { value: "highContrast", label: t("appearance.themeHighContrast") },
            ]}
            onChange={(theme) => patch({ theme })}
            ariaLabel={t("appearance.theme")}
          />
        </SettingRow>
      </SettingsCard>

      <SettingsCard title={t("appearance.popupSection")}>
        <SettingRow label={t("appearance.density")}>
          <SelectControl<Density>
            value={appearance.density}
            options={[
              { value: "comfortable", label: t("appearance.densityComfortable") },
              { value: "compact", label: t("appearance.densityCompact") },
            ]}
            onChange={(density) => patch({ density })}
            ariaLabel={t("appearance.density")}
          />
        </SettingRow>
        <div className="slider-row">
          <div className="slider-label">
            <span>{t("appearance.popupWidth")}</span>
            <Badge appearance="tint">{appearance.popupWidth}</Badge>
          </div>
          <Slider
            min={360}
            max={720}
            step={10}
            value={appearance.popupWidth}
            onChange={(_, data) => patch({ popupWidth: data.value })}
            aria-label={t("appearance.popupWidth")}
          />
        </div>
        <div className="slider-row">
          <div className="slider-label">
            <span>{t("appearance.popupHeight")}</span>
            <Badge appearance="tint">{appearance.popupHeight}</Badge>
          </div>
          <Slider
            min={320}
            max={800}
            step={10}
            value={appearance.popupHeight}
            onChange={(_, data) => patch({ popupHeight: data.value })}
            aria-label={t("appearance.popupHeight")}
          />
        </div>
        <p className="setting-help standalone-help">
          {t("appearance.popupSizeHelp", {
            minWidth: 360,
            maxWidth: 720,
            minHeight: 320,
            maxHeight: 800,
          })}
        </p>
        <div className="slider-row">
          <div className="slider-label">
            <span>{t("appearance.linesPerClip")}</span>
            <Badge appearance="tint">{appearance.linesPerClip}</Badge>
          </div>
          <Slider
            min={1}
            max={5}
            step={1}
            value={appearance.linesPerClip}
            onChange={(_, data) => patch({ linesPerClip: data.value })}
            aria-label={t("appearance.linesPerClip")}
          />
        </div>
        <div className="slider-row">
          <div className="slider-label">
            <span>{t("appearance.fontScale")}</span>
            <Badge appearance="tint">{appearance.fontScale}%</Badge>
          </div>
          <Slider
            min={100}
            max={150}
            step={5}
            value={appearance.fontScale}
            onChange={(_, data) => patch({ fontScale: data.value })}
            aria-label={t("appearance.fontScale")}
          />
        </div>
        <SettingRow label={t("appearance.preview")}>
          <SelectControl<PreviewMode>
            value={appearance.preview}
            options={[
              { value: "off", label: t("appearance.previewOff") },
              { value: "auto", label: t("appearance.previewAuto") },
              { value: "f3", label: t("appearance.previewF3") },
            ]}
            onChange={(preview) => patch({ preview })}
            ariaLabel={t("appearance.preview")}
          />
        </SettingRow>
      </SettingsCard>

      <SettingsCard title={t("appearance.motionSection")}>
        <SettingRow label={t("appearance.animations")} help={t("appearance.animationsHelp")}>
          <Switch
            checked={appearance.animations}
            onChange={(_, data) => patch({ animations: data.checked })}
            aria-label={t("appearance.animations")}
          />
        </SettingRow>
        <SettingRow label={t("appearance.alwaysOnTop")}>
          <Switch
            checked={appearance.alwaysOnTop}
            onChange={(_, data) => patch({ alwaysOnTop: data.checked })}
            aria-label={t("appearance.alwaysOnTop")}
          />
        </SettingRow>
      </SettingsCard>
    </div>
  );
}
