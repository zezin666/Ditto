import { Badge, Button, Skeleton, SkeletonItem } from "@fluentui/react-components";
import { FolderOpenRegular, InfoRegular, ShieldCheckmarkRegular } from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { commands } from "../api/commands";
import { formatBytes, SettingsCard } from "../components/Controls";
import { useI18n } from "../i18n";
import type { AppInfo, DiagnosticsInfo } from "../types";

export function AboutPage({ onOpenLogs }: { onOpenLogs: () => void }) {
  const { t } = useI18n();
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticsInfo | null>(null);

  useEffect(() => {
    void Promise.all([commands.getAppInfo(), commands.getDiagnostics()]).then(
      ([appInfo, diagnosticInfo]) => {
        setInfo(appInfo);
        setDiagnostics(diagnosticInfo);
      },
    );
  }, []);

  if (!info || !diagnostics) {
    return (
      <SettingsCard>
        <Skeleton>
          <SkeletonItem size={72} />
        </Skeleton>
      </SettingsCard>
    );
  }

  return (
    <div className="page-stack">
      <SettingsCard>
        <div className="about-hero">
          <div className="app-mark" aria-hidden="true">
            D
          </div>
          <div>
            <h2>{t("settings.title")}</h2>
            <p>{t("settings.subtitle")}</p>
          </div>
          <Badge appearance="tint">{info.version}</Badge>
        </div>
        <dl className="about-grid">
          <div>
            <dt>{t("about.version")}</dt>
            <dd>{info.version}</dd>
          </div>
          <div>
            <dt>{t("about.build")}</dt>
            <dd>{info.buildSha}</dd>
          </div>
          <div>
            <dt>{t("about.protocol")}</dt>
            <dd>{info.protocolVersion}</dd>
          </div>
          <div>
            <dt>{t("about.schema")}</dt>
            <dd>{info.schemaVersion}</dd>
          </div>
          <div>
            <dt>{t("about.license")}</dt>
            <dd>{info.license}</dd>
          </div>
          <div>
            <dt>{t("about.source")}</dt>
            <dd>
              <code>{info.sourceUrl}</code>
            </dd>
          </div>
        </dl>
      </SettingsCard>

      <SettingsCard title={t("about.diagnostics")}>
        <p className="setting-help standalone-help">{t("about.diagnosticsHelp")}</p>
        <dl className="about-grid">
          <div>
            <dt>{t("about.os")}</dt>
            <dd>
              {diagnostics.os} ({diagnostics.architecture})
            </dd>
          </div>
          <div>
            <dt>{t("about.pid")}</dt>
            <dd>{diagnostics.corePid}</dd>
          </div>
          <div>
            <dt>{t("about.webviewProcesses")}</dt>
            <dd>{diagnostics.webviewPidCount}</dd>
          </div>
          <div>
            <dt>{t("data.databaseSize")}</dt>
            <dd>{formatBytes(diagnostics.databaseBytes, t)}</dd>
          </div>
          <div>
            <dt>{t("about.diagnostics")}</dt>
            <dd>
              <Badge
                color={diagnostics.contentIncluded ? "danger" : "success"}
                icon={
                  diagnostics.contentIncluded ? <InfoRegular /> : <ShieldCheckmarkRegular />
                }
              >
                {diagnostics.contentIncluded
                  ? t("about.contentIncluded")
                  : t("about.contentExcluded")}
              </Badge>
            </dd>
          </div>
          <div>
            <dt>{t("general.logLevel")}</dt>
            <dd>
              <code>{diagnostics.logPath}</code>
            </dd>
          </div>
        </dl>
        <div className="card-actions">
          <Button icon={<FolderOpenRegular />} onClick={onOpenLogs}>
            {t("about.openLogs")}
          </Button>
        </div>
      </SettingsCard>

      <SettingsCard title={t("about.thirdParty")}>
        <p>{t("about.thirdPartyBody")}</p>
      </SettingsCard>
    </div>
  );
}
