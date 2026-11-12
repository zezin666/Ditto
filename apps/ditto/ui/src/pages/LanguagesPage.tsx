import { Badge, Button } from "@fluentui/react-components";
import { CheckmarkCircleRegular, LocalLanguageRegular } from "@fluentui/react-icons";
import enUS from "../../locales/en-US.json";
import zhCN from "../../locales/zh-CN.json";
import { SettingsCard } from "../components/Controls";
import { localeKeys, useI18n, type LocaleCode } from "../i18n";
import type { SettingsState } from "../types";

const languageDefinitions: {
  code: LocaleCode;
  labelKey: string;
  catalog: Record<string, string>;
}[] = [
  { code: "en-US", labelKey: "languages.english", catalog: enUS },
  { code: "zh-CN", labelKey: "languages.chinese", catalog: zhCN },
];

export function LanguagesPage({
  settings,
  onChange,
}: {
  settings: SettingsState;
  onChange: (next: SettingsState) => void;
}) {
  const { t, localeVersion, localeKeyCount } = useI18n();
  const activeLocale = settings.general.language;

  return (
    <div className="page-stack">
      <SettingsCard title={t("languages.available")}>
        <div className="language-list">
          {languageDefinitions.map((language) => {
            const selected = activeLocale === language.code;
            const keys = localeKeys(language.catalog);
            return (
              <button
                type="button"
                key={language.code}
                data-locale={language.code}
                className={`language-row ${selected ? "language-row-selected" : ""}`}
                aria-pressed={selected}
                onClick={() =>
                  onChange({
                    ...settings,
                    general: { ...settings.general, language: language.code },
                  })
                }
              >
                <LocalLanguageRegular />
                <span className="language-name">{t(language.labelKey)}</span>
                <span className="language-code">{language.code}</span>
                {selected ? <CheckmarkCircleRegular className="selected-icon" /> : null}
              </button>
            );
          })}
        </div>
      </SettingsCard>

      <SettingsCard title={t("languages.current")}>
        <dl className="about-grid">
          <div>
            <dt>{t("languages.current")}</dt>
            <dd>{activeLocale}</dd>
          </div>
          <div>
            <dt>{t("languages.version")}</dt>
            <dd>{localeVersion}</dd>
          </div>
          <div>
            <dt>{t("languages.keyCount")}</dt>
            <dd>{localeKeyCount}</dd>
          </div>
          <div>
            <dt>{t("languages.complete")}</dt>
            <dd>
              <Badge appearance="filled" color="success" icon={<CheckmarkCircleRegular />}>
                {t("languages.complete")}
              </Badge>
            </dd>
          </div>
        </dl>
        <p className="setting-help standalone-help">{t("languages.restart")}</p>
      </SettingsCard>
    </div>
  );
}
