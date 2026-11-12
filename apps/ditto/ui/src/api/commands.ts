import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { browserInvoke } from "./browserMock";
import type {
  AppInfo,
  ClipDetail,
  ClipKind,
  ClipPage,
  CopyBuffer,
  DiagnosticsInfo,
  GroupNode,
  MigrationStatus,
  MutationResult,
  OperationResult,
  SettingsEnvelope,
  SettingsState,
} from "../types";

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (isTauriRuntime()) {
    return tauriInvoke<T>(command, args);
  }
  return browserInvoke<T>(command, args);
}

export const commands = {
  getSettings: () => call<SettingsEnvelope>("get_settings"),
  setSettings: (settings: SettingsState) =>
    call<SettingsEnvelope>("set_settings", { settings }),

  listClips: (args: {
    page: number;
    pageSize: number;
    query?: string;
    groupId?: number | null;
    kind?: ClipKind | "";
    pinnedOnly?: boolean;
  }) =>
    call<ClipPage>("list_clips", {
      page: args.page,
      pageSize: args.pageSize,
      query: args.query ?? "",
      groupId: args.groupId ?? null,
      kind: args.kind || null,
      pinnedOnly: args.pinnedOnly ?? false,
    }),
  searchClips: (args: {
    query: string;
    page: number;
    pageSize: number;
    groupId?: number | null;
    kind?: ClipKind | "";
    pinnedOnly?: boolean;
  }) =>
    call<ClipPage>("search_clips", {
      query: args.query,
      page: args.page,
      pageSize: args.pageSize,
      groupId: args.groupId ?? null,
      kind: args.kind || null,
      pinnedOnly: args.pinnedOnly ?? false,
    }),
  getClip: (id: number) => call<ClipDetail>("get_clip", { id }),
  editClip: (id: number, text: string) =>
    call<MutationResult>("edit_clip", { id, text }),
  deleteClips: (ids: number[]) => call<MutationResult>("delete_clips", { ids }),
  setPinned: (ids: number[], pinned: boolean) =>
    call<MutationResult>("set_pinned", { ids, pinned }),
  moveToGroup: (ids: number[], groupId: number | null) =>
    call<MutationResult>("move_to_group", { ids, groupId }),

  getGroups: () => call<GroupNode[]>("get_groups"),
  createGroup: (name: string, parentId: number | null = null) =>
    call<GroupNode>("create_group", { name, parentId }),
  renameGroup: (id: number, name: string) =>
    call<GroupNode>("rename_group", { id, name }),
  deleteGroup: (id: number) => call<MutationResult>("delete_group", { id }),
  moveGroup: (id: number, parentId: number | null) =>
    call<MutationResult>("move_group", { id, parentId }),

  getCopyBuffers: () => call<CopyBuffer[]>("get_copy_buffers"),
  setCopyBuffer: (args: {
    slot: number;
    name?: string;
    enabled?: boolean;
    clipId: number | null;
  }) =>
    call<CopyBuffer>("set_copy_buffer", {
      slot: args.slot,
      name: args.name,
      enabled: args.enabled,
      clipId: args.clipId,
    }),

  backupDb: (destination: string | null = null) =>
    call<OperationResult>("backup_db", { destination }),
  restoreDb: (source: string | null = null) =>
    call<OperationResult>("restore_db", { source }),
  exportData: (destination: string | null = null) =>
    call<OperationResult>("export_data", { destination }),
  importData: (source: string | null = null) =>
    call<OperationResult>("import_data", { source }),
  optimizeStorage: () => call<OperationResult>("optimize_storage"),
  getMigrationStatus: () => call<MigrationStatus>("get_migration_status"),
  openDataFolder: () => call<OperationResult>("open_data_folder"),
  openLogFolder: () => call<OperationResult>("open_log_folder"),
  getDiagnostics: () => call<DiagnosticsInfo>("get_diagnostics"),
  getAppInfo: () => call<AppInfo>("get_app_info"),
};
