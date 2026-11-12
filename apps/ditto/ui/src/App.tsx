import {
  Badge,
  Button,
  FluentProvider,
  MessageBar,
  MessageBarBody,
  Nav,
  NavItem,
  Skeleton,
  SkeletonItem,
  Toast,
  Toaster,
  ToastTitle,
  useId,
  useToastController,
} from "@fluentui/react-components";
import {
  ArrowUndoRegular,
  ClipboardRegular,
  DatabaseRegular,
  FolderRegular,
  InfoRegular,
  KeyboardRegular,
  LocalLanguageRegular,
  PaintBrushRegular,
  SaveRegular,
  SettingsRegular,
  TagRegular,
} from "@fluentui/react-icons";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useMemo, useState } from "react";
import { commands } from "./api/commands";
import { I18nProvider, useI18n } from "./i18n";
import { resolveTheme, uiFontFamily } from "./theme";
import type {
  CopyBuffer,
  GroupNode,
  Notify,
  SettingsEnvelope,
  SettingsState,
  ToastIntent,
} from "./types";
import { GeneralPage } from "./pages/GeneralPage";
import { AppearancePage } from "./pages/AppearancePage";
import { HotkeysPage } from "./pages/HotkeysPage";
import { ClipboardPage } from "./pages/ClipboardPage";
import { GroupsPage } from "./pages/GroupsPage";
import { CopyBuffersPage } from "./pages/CopyBuffersPage";
import { DataPage } from "./pages/DataPage";
import { LanguagesPage } from "./pages/LanguagesPage";
import { AboutPage } from "./pages/AboutPage";

type Page =
  | "general"
  | "appearance"
  | "hotkeys"
  | "clipboard"
  | "groups"
  | "copyBuffers"
  | "data"
  | "languages"
  | "about";

const pageKeys: Record<Page, { title: string; description: string }> = {
  general: { title: "page.general.title", description: "page.general.description" },
  appearance: { title: "page.appearance.title", description: "page.appearance.description" },
  hotkeys: { title: "page.hotkeys.title", description: "page.hotkeys.description" },
  clipboard: { title: "page.clipboard.title", description: "page.clipboard.description" },
  groups: { title: "page.groups.title", description: "page.groups.description" },
  copyBuffers: { title: "page.copyBuffers.title", description: "page.copyBuffers.description" },
  data: { title: "page.data.title", description: "page.data.description" },
  languages: { title: "page.languages.title", description: "page.languages.description" },
  about: { title: "page.about.title", description: "page.about.description" },
};

export function App() {
  const [envelope, setEnvelope] = useState<SettingsEnvelope | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [reload, setReload] = useState(0);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(false);
    commands
      .getSettings()
      .then((result) => {
        if (active) {
          setEnvelope(result);
        }
      })
      .catch(() => {
        if (active) {
          setError(true);
        }
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [reload]);

  if (loading || error || !envelope) {
    const theme = resolveTheme("system");
    return (
      <I18nProvider initialLocale="en-US">
        <FluentProvider theme={theme.theme}>
          <BootState
            loading={loading}
            error={error}
            onRetry={() => setReload((current) => current + 1)}
          />
        </FluentProvider>
      </I18nProvider>
    );
  }

  return <SettingsSurface initial={envelope} />;
}

function BootState({
  loading,
  error,
  onRetry,
}: {
  loading: boolean;
  error: boolean;
  onRetry: () => void;
}) {
  const { t } = useI18n();
  return (
    <main className="boot-shell" aria-live="polite">
      <div className="boot-card">
        {error ? (
          <>
            <h1>{t("state.errorTitle")}</h1>
            <p>{t("state.errorBody")}</p>
            <Button appearance="primary" onClick={onRetry}>
              {t("common.retry")}
            </Button>
          </>
        ) : (
          <>
            <h1>{loading ? t("state.loadingTitle") : t("state.errorTitle")}</h1>
            <p>{t("state.loadingBody")}</p>
            <div className="boot-skeleton">
              {Array.from({ length: 4 }, (_, index) => (
                <Skeleton key={index}>
                  <SkeletonItem size={32} />
                </Skeleton>
              ))}
            </div>
          </>
        )}
      </div>
    </main>
  );
}

function SettingsSurface({ initial }: { initial: SettingsEnvelope }) {
  const [saved, setSaved] = useState(initial.values);
  const [draft, setDraft] = useState(initial.values);
  const [revision, setRevision] = useState(initial.revision);
  const [restartRequired, setRestartRequired] = useState(initial.restartRequired);
  const [themeTick, setThemeTick] = useState(0);
  const resolvedTheme = useMemo(() => resolveTheme(draft.appearance.theme), [draft.appearance.theme, themeTick]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const contrast = window.matchMedia("(forced-colors: active)");
    const update = () => setThemeTick((current) => current + 1);
    media.addEventListener("change", update);
    contrast.addEventListener("change", update);
    return () => {
      media.removeEventListener("change", update);
      contrast.removeEventListener("change", update);
    };
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedTheme.mode === "hc" ? "hc" : resolvedTheme.mode;
    document.documentElement.dataset.highContrast = String(resolvedTheme.mode === "hc");
    document.documentElement.style.setProperty("--ditto-font-family-ui", uiFontFamily);
  }, [resolvedTheme.mode]);

  useEffect(() => {
    document.documentElement.dataset.animations = String(draft.appearance.animations);
    document.documentElement.dataset.density = draft.appearance.density;
    document.documentElement.style.setProperty(
      "--t5-user-font-scale",
      String(draft.appearance.fontScale / 100),
    );
  }, [
    draft.appearance.animations,
    draft.appearance.density,
    draft.appearance.fontScale,
  ]);

  return (
    <I18nProvider initialLocale={draft.general.language}>
      <FluentProvider theme={resolvedTheme.theme}>
        <SettingsShell
          saved={saved}
          setSaved={setSaved}
          draft={draft}
          setDraft={setDraft}
          revision={revision}
          setRevision={setRevision}
          restartRequired={restartRequired}
          setRestartRequired={setRestartRequired}
        />
      </FluentProvider>
    </I18nProvider>
  );
}

function SettingsShell({
  saved,
  setSaved,
  draft,
  setDraft,
  revision,
  setRevision,
  restartRequired,
  setRestartRequired,
}: {
  saved: SettingsState;
  setSaved: (settings: SettingsState) => void;
  draft: SettingsState;
  setDraft: (settings: SettingsState) => void;
  revision: number;
  setRevision: (revision: number) => void;
  restartRequired: boolean;
  setRestartRequired: (required: boolean) => void;
}) {
  const { t, missingKeys, fallbackKeys } = useI18n();
  const [page, setPage] = useState<Page>("general");
  const [groups, setGroups] = useState<GroupNode[]>([]);
  const [buffers, setBuffers] = useState<CopyBuffer[]>([]);
  const [selectedClipId, setSelectedClipId] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const toasterId = useId("ditto-toaster");
  const { dispatchToast } = useToastController(toasterId);
  const dirty = JSON.stringify(saved) !== JSON.stringify(draft);
  const meta = pageKeys[page];

  const notify: Notify = (intent, message) => {
    dispatchToast(
      <Toast>
        <ToastTitle>{message}</ToastTitle>
      </Toast>,
      { intent: intent as ToastIntent, timeout: 3800 },
    );
  };

  const refreshData = async () => {
    const [nextGroups, nextBuffers] = await Promise.all([
      commands.getGroups(),
      commands.getCopyBuffers(),
    ]);
    setGroups(nextGroups);
    setBuffers(nextBuffers);
  };

  useEffect(() => {
    void refreshData();
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      if (document.querySelector('[role="dialog"]')) {
        return;
      }
      if ("__TAURI_INTERNALS__" in window) {
        void getCurrentWindow().close();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const applySettings = async () => {
    setBusy(true);
    try {
      const result = await commands.setSettings(draft);
      setSaved(result.values);
      setDraft(result.values);
      setRevision(result.revision);
      setRestartRequired(result.restartRequired);
      notify("success", t("toast.settingsSaved"));
      await refreshData();
    } catch {
      notify("error", t("toast.settingsSaveFailed"));
    } finally {
      setBusy(false);
    }
  };

  const discardSettings = () => {
    setDraft(saved);
    setRestartRequired(false);
  };

  const openLogs = async () => {
    try {
      await commands.openLogFolder();
      notify("success", t("toast.actionComplete"));
    } catch {
      notify("error", t("toast.openFolderFailed"));
    }
  };

  const renderPage = () => {
    switch (page) {
      case "general":
        return <GeneralPage settings={draft} onChange={setDraft} onOpenLogs={() => void openLogs()} />;
      case "appearance":
        return <AppearancePage settings={draft} onChange={setDraft} />;
      case "hotkeys":
        return <HotkeysPage settings={draft} onChange={setDraft} />;
      case "clipboard":
        return <ClipboardPage settings={draft} onChange={setDraft} />;
      case "groups":
        return (
          <GroupsPage
            settings={draft}
            groups={groups}
            onChange={setDraft}
            onRefresh={refreshData}
            notify={notify}
          />
        );
      case "copyBuffers":
        return (
          <CopyBuffersPage
            settings={draft}
            buffers={buffers}
            selectedClipId={selectedClipId}
            onChange={setDraft}
            onRefresh={refreshData}
            notify={notify}
          />
        );
      case "data":
        return (
          <DataPage
            settings={draft}
            groups={groups}
            buffers={buffers}
            selectedClipId={selectedClipId}
            onSelectedClipChange={setSelectedClipId}
            onDataChanged={refreshData}
            notify={notify}
          />
        );
      case "languages":
        return <LanguagesPage settings={draft} onChange={setDraft} />;
      case "about":
        return <AboutPage onOpenLogs={() => void openLogs()} />;
      default:
        return null;
    }
  };

  return (
    <>
      <Toaster toasterId={toasterId} position="top-end" />
      <div className="settings-app">
        <header className="app-header">
          <div className="app-title">
            <div className="app-mark" aria-hidden="true">
              D
            </div>
            <div>
              <h1>{t("settings.title")}</h1>
              <p>{t("settings.subtitle")}</p>
            </div>
          </div>
          <div className="header-actions">
            <Badge appearance="outline" className="mock-badge" title={t("mock.note")}>
              {t("mock.badge")}
            </Badge>
            <Button
              icon={<ArrowUndoRegular />}
              appearance="secondary"
              disabled={!dirty || busy}
              onClick={discardSettings}
            >
              {t("common.discard")}
            </Button>
            <Button
              icon={<SaveRegular />}
              appearance="primary"
              disabled={!dirty || busy}
              onClick={() => void applySettings()}
            >
              {busy ? t("common.saving") : t("common.apply")}
            </Button>
          </div>
        </header>

        <div className="app-body">
          <aside className="nav-pane" aria-label={t("settings.title")}>
            <Nav
              selectedValue={page}
              onNavItemSelect={(_, data) => {
                if (data.value) {
                  setPage(data.value as Page);
                }
              }}
            >
              <NavItem value="general" data-page="general" icon={<SettingsRegular />}>
                {t("nav.general")}
              </NavItem>
              <NavItem value="appearance" data-page="appearance" icon={<PaintBrushRegular />}>
                {t("nav.appearance")}
              </NavItem>
              <NavItem value="hotkeys" data-page="hotkeys" icon={<KeyboardRegular />}>
                {t("nav.hotkeys")}
              </NavItem>
              <NavItem value="clipboard" data-page="clipboard" icon={<ClipboardRegular />}>
                {t("nav.clipboard")}
              </NavItem>
              <NavItem value="groups" data-page="groups" icon={<FolderRegular />}>
                {t("nav.groups")}
              </NavItem>
              <NavItem value="copyBuffers" data-page="copyBuffers" icon={<TagRegular />}>
                {t("nav.copyBuffers")}
              </NavItem>
              <NavItem value="data" data-page="data" icon={<DatabaseRegular />}>
                {t("nav.data")}
              </NavItem>
              <NavItem value="languages" data-page="languages" icon={<LocalLanguageRegular />}>
                {t("nav.languages")}
              </NavItem>
              <NavItem value="about" data-page="about" icon={<InfoRegular />}>
                {t("nav.about")}
              </NavItem>
            </Nav>
            <div className="nav-footer" title={t("mock.note")}>
              <Badge appearance="tint" size="small">
                {t("mock.badge")}
              </Badge>
              <span>{t("mock.note")}</span>
            </div>
          </aside>

          <main className="main-pane" id="main-content">
            <div className="page-header">
              <div>
                <h2>{t(meta.title)}</h2>
                <p>{t(meta.description)}</p>
              </div>
              <span className="revision-label">
                {t("common.revision", { revision })}
              </span>
            </div>

            {missingKeys.length ? (
              <MessageBar intent="error" className="page-message">
                <MessageBarBody>
                  {t("i18n.missing", { keys: missingKeys.join(", ") })}
                </MessageBarBody>
              </MessageBar>
            ) : null}
            {fallbackKeys.length ? (
              <MessageBar intent="warning" className="page-message">
                <MessageBarBody>
                  {t("i18n.fallback", { keys: fallbackKeys.join(", ") })}
                </MessageBarBody>
              </MessageBar>
            ) : null}
            {restartRequired ? (
              <MessageBar intent="info" className="page-message">
                <MessageBarBody>{t("general.languageHelp")}</MessageBarBody>
              </MessageBar>
            ) : null}
            <div className="page-scroll">{renderPage()}</div>
          </main>
        </div>
      </div>
    </>
  );
}
