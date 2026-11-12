import type {
  AppInfo,
  ClipDetail,
  ClipPage,
  ClipSummary,
  CopyBuffer,
  DiagnosticsInfo,
  GroupNode,
  MigrationRun,
  MigrationStatus,
  MutationResult,
  OperationResult,
  SettingsEnvelope,
  SettingsState,
} from "../types";

const GROUP_NAMES = [
  "Work",
  "Personal",
  "Engineering",
  "Design",
  "Snippets",
  "References",
  "QA fixtures",
  "Archive",
] as const;

const SOURCE_APPS = ["Notepad", "Visual Studio Code", "Microsoft Edge", "Terminal", "Figma"];

const SYNTHETIC_TEXT = [
  "QA sample clipboard entry for the Fluent settings history.",
  "Synthetic release checklist: verify build, locale, and lifecycle evidence.",
  "Ditto settings browser preview never reads the real clipboard history.",
  "A longer synthetic paragraph used to inspect wrapping, row density, and preview layout without exposing user data.",
  "搜索、筛选、预览和分页都在合成测试数据上运行。",
  "复制缓冲区、分组和热键状态来自浏览器预览夹具。",
];

function initialSettings(): SettingsState {
  return {
    general: {
      runAtStartup: false,
      startMinimized: true,
      showTrayIcon: true,
      singleInstance: true,
      language: "en-US",
      startupMessage: "",
      checkForUpdates: false,
      logLevel: "info",
    },
    appearance: {
      theme: "system",
      density: "comfortable",
      popupWidth: 420,
      popupHeight: 480,
      linesPerClip: 2,
      preview: "auto",
      animations: true,
      alwaysOnTop: false,
      fontScale: 100,
    },
    hotkeys: {
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
    },
    clipboard: {
      maxEntries: 500,
      expireDays: 30,
      ignoreFormats: ["CF_DIB"],
      maxClipBytes: 10_485_760,
      captureFiles: true,
      captureImages: true,
      allowDuplicates: false,
      recordSourceApp: true,
    },
    groups: {
      defaultGroupId: null,
      promptForName: true,
      showGroupedInMain: true,
    },
    copyBuffers: {
      playSound: false,
    },
    data: {
      dataRoot: "C:\\Users\\synthetic\\AppData\\Local\\Ditto\\v2",
      portable: false,
      migrationCompleted: false,
      migrationInProgress: false,
      databaseBytes: 12_582_912,
      lastBackupAt: "2026-11-12T10:30:00Z",
    },
  };
}

function seedGroups(): GroupNode[] {
  return GROUP_NAMES.map((name, index) => ({
    id: index + 1,
    name,
    parentId: index === 7 ? 3 : null,
    count: 0,
  }));
}

function seedClips(): ClipDetail[] {
  return Array.from({ length: 620 }, (_, index) => {
    const id = index + 1;
    const text = `${SYNTHETIC_TEXT[index % SYNTHETIC_TEXT.length]} #${String(id).padStart(3, "0")}`;
    return {
      id,
      kind: index % 17 === 0 ? "image" : index % 11 === 0 ? "files" : "text",
      previewText: text,
      text,
      sourceApp: SOURCE_APPS[index % SOURCE_APPS.length],
      createdAtMs: Date.UTC(2026, 10, 12) - index * 61_000,
      lastPastedAtMs: index % 5 === 0 ? Date.UTC(2026, 10, 11) : null,
      pinned: index < 12,
      groupId: index % 4 === 0 ? null : (index % 8) + 1,
      copyBuffer: index < 5 ? index + 1 : null,
      formats: index % 17 === 0 ? ["PNG", "CF_DIB"] : ["CF_UNICODETEXT"],
      sizeBytes: 256 + index * 13,
    };
  });
}

let settings = initialSettings();
let revision = 1;
let groups = seedGroups();
let clips = seedClips();
let buffers: CopyBuffer[] = Array.from({ length: 5 }, (_, index) => {
  const clip = clips[index];
  return {
    slot: index + 1,
    name: "",
    enabled: true,
    clipId: clip?.id ?? null,
    clipPreview: clip?.previewText ?? null,
    updatedAtMs: Date.UTC(2026, 10, 12) - index * 1000,
  };
});

function refreshGroupCounts() {
  groups = groups.map((group) => ({
    ...group,
    count: clips.filter((clip) => clip.groupId === group.id).length,
  }));
}

function toSummary(clip: ClipDetail): ClipSummary {
  const { text: _text, ...summary } = clip;
  return summary;
}

function matchesQuery(clip: ClipSummary, query: string) {
  if (!query.trim()) {
    return true;
  }
  const needle = query.trim().toLocaleLowerCase();
  return [clip.previewText, clip.sourceApp, ...clip.formats].some((value) =>
    value.toLocaleLowerCase().includes(needle),
  );
}

function pagedClips(args: Record<string, unknown>): ClipPage {
  const page = Math.max(1, Number(args.page ?? 1));
  const pageSize = Math.min(500, Math.max(1, Number(args.pageSize ?? 50)));
  const query = String(args.query ?? "");
  const groupId =
    args.groupId === undefined || args.groupId === null ? undefined : Number(args.groupId);
  const kind = args.kind ? String(args.kind) : "";
  const pinnedOnly = Boolean(args.pinnedOnly);

  const filtered = clips
    .map(toSummary)
    .filter((clip) => matchesQuery(clip, query))
    .filter((clip) => groupId === undefined || clip.groupId === groupId)
    .filter((clip) => !kind || clip.kind === kind)
    .filter((clip) => !pinnedOnly || clip.pinned)
    .sort((left, right) => {
      if (left.pinned !== right.pinned) {
        return left.pinned ? -1 : 1;
      }
      return right.createdAtMs - left.createdAtMs;
    });

  return {
    items: filtered.slice((page - 1) * pageSize, page * pageSize),
    total: filtered.length,
    page,
    pageSize,
  };
}

function updateClip(id: number, updater: (clip: ClipDetail) => ClipDetail) {
  const index = clips.findIndex((clip) => clip.id === id);
  if (index < 0) {
    throw { code: "DITTO_E_NOT_FOUND", message: "Clip not found", retryable: false };
  }
  clips[index] = updater(clips[index]);
  refreshGroupCounts();
}

function refreshBufferPreviews() {
  buffers = buffers.map((buffer) => {
    const clip = clips.find((candidate) => candidate.id === buffer.clipId);
    return { ...buffer, clipPreview: clip?.previewText ?? null };
  });
  clips = clips.map((clip) => {
    const buffer = buffers.find((candidate) => candidate.clipId === clip.id);
    return { ...clip, copyBuffer: buffer?.slot ?? null };
  });
}

export async function browserInvoke<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  await new Promise((resolve) => window.setTimeout(resolve, 45));
  switch (command) {
    case "get_settings":
      return structuredClone({
        values: settings,
        revision,
        restartRequired: false,
      }) as SettingsEnvelope as T;
    case "set_settings": {
      const next = args.settings as SettingsState;
      const restartRequired =
        next.general.language !== settings.general.language ||
        next.data.dataRoot !== settings.data.dataRoot;
      settings = structuredClone(next);
      revision += 1;
      return structuredClone({
        values: settings,
        revision,
        restartRequired,
      }) as SettingsEnvelope as T;
    }
    case "list_clips":
    case "search_clips":
      return structuredClone(pagedClips(args)) as T;
    case "get_clip": {
      const clip = clips.find((candidate) => candidate.id === Number(args.id));
      if (!clip) {
        throw { code: "DITTO_E_NOT_FOUND", message: "Clip not found", retryable: false };
      }
      return structuredClone(clip) as T;
    }
    case "delete_clips": {
      const ids = new Set((args.ids as number[]).map(Number));
      clips = clips.filter((clip) => !ids.has(clip.id));
      refreshBufferPreviews();
      refreshGroupCounts();
      return { ok: true, affected: ids.size, message: "deleted" } as MutationResult as T;
    }
    case "set_pinned": {
      const ids = new Set((args.ids as number[]).map(Number));
      clips = clips.map((clip) => (ids.has(clip.id) ? { ...clip, pinned: Boolean(args.pinned) } : clip));
      return { ok: true, affected: ids.size, message: "updated" } as MutationResult as T;
    }
    case "move_to_group": {
      const ids = new Set((args.ids as number[]).map(Number));
      const groupId = args.groupId === null || args.groupId === undefined ? null : Number(args.groupId);
      clips = clips.map((clip) => (ids.has(clip.id) ? { ...clip, groupId } : clip));
      refreshGroupCounts();
      return { ok: true, affected: ids.size, message: "moved" } as MutationResult as T;
    }
    case "edit_clip": {
      const id = Number(args.id);
      const text = String(args.text ?? "");
      updateClip(id, (clip) => ({
        ...clip,
        text,
        previewText: text.slice(0, 512),
        sizeBytes: new TextEncoder().encode(text).length,
      }));
      refreshBufferPreviews();
      return { ok: true, affected: 1, message: "updated" } as MutationResult as T;
    }
    case "get_groups":
      refreshGroupCounts();
      return structuredClone(groups) as T;
    case "create_group": {
      const id = Math.max(0, ...groups.map((group) => group.id)) + 1;
      const group = {
        id,
        name: String(args.name),
        parentId: args.parentId === null || args.parentId === undefined ? null : Number(args.parentId),
        count: 0,
      };
      groups = [...groups, group];
      return structuredClone(group) as T;
    }
    case "rename_group": {
      const id = Number(args.id);
      groups = groups.map((group) => (group.id === id ? { ...group, name: String(args.name) } : group));
      return structuredClone(groups.find((group) => group.id === id)) as T;
    }
    case "delete_group": {
      const id = Number(args.id);
      const removed = new Set<number>([id]);
      let changed = true;
      while (changed) {
        changed = false;
        for (const group of groups) {
          if (group.parentId !== null && removed.has(group.parentId) && !removed.has(group.id)) {
            removed.add(group.id);
            changed = true;
          }
        }
      }
      clips = clips.map((clip) => (clip.groupId !== null && removed.has(clip.groupId) ? { ...clip, groupId: null } : clip));
      groups = groups.filter((group) => !removed.has(group.id));
      return { ok: true, affected: removed.size, message: "deleted" } as MutationResult as T;
    }
    case "move_group": {
      const id = Number(args.id);
      groups = groups.map((group) =>
        group.id === id
          ? {
              ...group,
              parentId: args.parentId === null || args.parentId === undefined ? null : Number(args.parentId),
            }
          : group,
      );
      return { ok: true, affected: 1, message: "moved" } as MutationResult as T;
    }
    case "get_copy_buffers":
      refreshBufferPreviews();
      return structuredClone(buffers) as T;
    case "set_copy_buffer": {
      const slot = Number(args.slot);
      const clipId = args.clipId === null || args.clipId === undefined ? null : Number(args.clipId);
      buffers = buffers.map((buffer) =>
        buffer.slot === slot
          ? {
              ...buffer,
              name: args.name === undefined ? buffer.name : String(args.name),
              enabled: args.enabled === undefined ? buffer.enabled : Boolean(args.enabled),
              clipId,
              updatedAtMs: Date.now(),
            }
          : buffer,
      );
      refreshBufferPreviews();
      return structuredClone(buffers.find((buffer) => buffer.slot === slot)) as T;
    }
    case "backup_db":
      settings.data.lastBackupAt = new Date().toISOString();
      return {
        ok: true,
        message: "backup",
        path: "C:\\Users\\synthetic\\AppData\\Local\\Ditto\\v2\\backups\\mock-backup.db",
      } as OperationResult as T;
    case "restore_db":
      return { ok: true, message: "restore", path: null } as OperationResult as T;
    case "export_data":
      return { ok: true, message: "export", path: null } as OperationResult as T;
    case "import_data":
      return { ok: true, message: "import", path: null } as OperationResult as T;
    case "optimize_storage":
      settings.data.databaseBytes = Math.max(1024, settings.data.databaseBytes - 1_048_576);
      return { ok: true, message: "optimize", path: null } as OperationResult as T;
    case "get_migration_status":
      return {
        completed: settings.data.migrationCompleted,
        inProgress: settings.data.migrationInProgress,
        phase: "browser-preview",
        processed: 0,
        total: 0,
        sourcePath: null,
        startedAt: null,
        completedAt: null,
        warnings: ["Browser preview: no real legacy data was read."],
      } as MigrationStatus as T;
    case "migration_dry_run":
      return {
        status: "dry_run",
        dryRun: true,
        sourcePath: String(args.source ?? ""),
        clips: 120,
        groups: 8,
        formats: 260,
        copyBuffers: 5,
        pinned: 12,
        warnings: ["Browser preview: synthetic migration counts."],
        backupPaths: [],
        restartRequired: false,
      } as MigrationRun as T;
    case "migration_apply":
      return {
        status: "dry_run",
        dryRun: false,
        sourcePath: String(args.source ?? ""),
        clips: 120,
        groups: 8,
        formats: 260,
        copyBuffers: 5,
        pinned: 12,
        warnings: ["Browser preview: no migration was staged."],
        backupPaths: [],
        restartRequired: true,
      } as MigrationRun as T;
    case "open_data_folder":
    case "open_log_folder":
      return { ok: true, message: "mock-open", path: null } as OperationResult as T;
    case "get_diagnostics":
      return {
        os: "Windows 11",
        architecture: "x86_64",
        corePid: 4242,
        webviewPidCount: 6,
        databaseBytes: settings.data.databaseBytes,
        logPath: "C:\\Users\\synthetic\\AppData\\Local\\Ditto\\v2\\logs",
        settingsRevision: revision,
        contentIncluded: false,
      } as DiagnosticsInfo as T;
    case "get_app_info":
      return {
        version: "0.1.0",
        buildSha: "browser-preview",
        protocolVersion: "1",
        schemaVersion: "mock",
        sourceUrl: "https://github.com/sabrogden/Ditto",
        license: "GPL-3.0-only",
      } as AppInfo as T;
    default:
      throw {
        code: "DITTO_E_UNKNOWN_COMMAND",
        message: `Unknown browser preview command: ${command}`,
        retryable: false,
      };
  }
}
