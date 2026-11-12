export type ThemeChoice = "system" | "light" | "dark" | "highContrast";
export type Density = "comfortable" | "compact";
export type PreviewMode = "off" | "auto" | "f3";
export type LogLevel = "error" | "warn" | "info" | "debug" | "trace";
export type ClipKind = "text" | "html" | "rtf" | "image" | "files" | "custom";

export interface GeneralSettings {
  runAtStartup: boolean;
  startMinimized: boolean;
  showTrayIcon: boolean;
  singleInstance: boolean;
  language: "en-US" | "zh-CN";
  startupMessage: string;
  checkForUpdates: boolean;
  logLevel: LogLevel;
}

export interface AppearanceSettings {
  theme: ThemeChoice;
  density: Density;
  popupWidth: number;
  popupHeight: number;
  linesPerClip: number;
  preview: PreviewMode;
  animations: boolean;
  alwaysOnTop: boolean;
  fontScale: number;
}

export interface CopyBufferHotkeys {
  slot: number;
  copy: string;
  paste: string;
  cut: string;
}

export interface HotkeyConflict {
  action: string;
  chord: string;
  reason: "in_use" | "invalid" | "duplicate" | "fallback";
  effective: string | null;
}

export interface HotkeySettings {
  open: string;
  openAlt: string;
  plainTextPaste: string;
  firstTenEnabled: boolean;
  firstTenChords: string[];
  copyBuffers: CopyBufferHotkeys[];
  startupConflicts: HotkeyConflict[];
}

export interface ClipboardSettings {
  maxEntries: number;
  expireDays: number;
  ignoreFormats: string[];
  maxClipBytes: number;
  captureFiles: boolean;
  captureImages: boolean;
  allowDuplicates: boolean;
  recordSourceApp: boolean;
}

export interface GroupSettings {
  defaultGroupId: number | null;
  promptForName: boolean;
  showGroupedInMain: boolean;
}

export interface CopyBufferSettings {
  playSound: boolean;
}

export interface DataSettings {
  dataRoot: string;
  portable: boolean;
  migrationCompleted: boolean;
  migrationInProgress: boolean;
  databaseBytes: number;
  lastBackupAt: string | null;
}

export interface SettingsState {
  general: GeneralSettings;
  appearance: AppearanceSettings;
  hotkeys: HotkeySettings;
  clipboard: ClipboardSettings;
  groups: GroupSettings;
  copyBuffers: CopyBufferSettings;
  data: DataSettings;
}

export interface SettingsEnvelope {
  values: SettingsState;
  revision: number;
  restartRequired: boolean;
}

export interface ClipSummary {
  id: number;
  kind: ClipKind;
  previewText: string;
  sourceApp: string;
  createdAtMs: number;
  lastPastedAtMs: number | null;
  pinned: boolean;
  groupId: number | null;
  copyBuffer: number | null;
  formats: string[];
  sizeBytes: number;
}

export interface ClipDetail extends ClipSummary {
  text: string;
}

export interface ClipPage {
  items: ClipSummary[];
  total: number;
  page: number;
  pageSize: number;
}

export interface GroupNode {
  id: number;
  name: string;
  parentId: number | null;
  count: number;
}

export interface CopyBuffer {
  slot: number;
  name: string;
  enabled: boolean;
  clipId: number | null;
  clipPreview: string | null;
  updatedAtMs: number | null;
}

export interface MutationResult {
  ok: boolean;
  affected: number;
  message: string;
}

export interface OperationResult {
  ok: boolean;
  message: string;
  path: string | null;
}

export interface MigrationStatus {
  completed: boolean;
  inProgress: boolean;
  phase: string;
  processed: number;
  total: number;
  sourcePath: string | null;
  startedAt: string | null;
  completedAt: string | null;
  warnings: string[];
}

export interface MigrationRun {
  status: string;
  dryRun: boolean;
  sourcePath: string;
  clips: number;
  groups: number;
  formats: number;
  copyBuffers: number;
  pinned: number;
  warnings: string[];
  backupPaths: string[];
  restartRequired: boolean;
}

export interface AppInfo {
  version: string;
  buildSha: string;
  protocolVersion: string;
  schemaVersion: string;
  sourceUrl: string;
  license: string;
}

export interface DiagnosticsInfo {
  os: string;
  architecture: string;
  corePid: number;
  webviewPidCount: number;
  databaseBytes: number;
  logPath: string;
  settingsRevision: number;
  contentIncluded: boolean;
}

export interface CommandError {
  code: string;
  message: string;
  retryable: boolean;
}

export type ToastIntent = "success" | "error" | "warning" | "info";
export type Notify = (intent: ToastIntent, message: string) => void;
