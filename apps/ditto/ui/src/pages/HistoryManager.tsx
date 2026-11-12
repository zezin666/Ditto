import {
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  MessageBar,
  MessageBarBody,
  Skeleton,
  SkeletonItem,
  Table,
  TableBody,
  TableCell,
  TableHeader,
  TableHeaderCell,
  TableRow,
  Textarea,
} from "@fluentui/react-components";
import {
  AddRegular,
  ArrowLeftRegular,
  ArrowRightRegular,
  DeleteRegular,
  EditRegular,
  PinRegular,
  SearchRegular,
  TagRegular,
  PinOffRegular,
} from "@fluentui/react-icons";
import { useCallback, useEffect, useState } from "react";
import { commands } from "../api/commands";
import {
  ConfirmDialog,
  EmptyState,
  SelectControl,
  SettingsCard,
} from "../components/Controls";
import { useI18n } from "../i18n";
import type {
  ClipDetail,
  ClipKind,
  ClipPage,
  CopyBuffer,
  GroupNode,
  Notify,
} from "../types";

const EMPTY_PAGE: ClipPage = { items: [], total: 0, page: 1, pageSize: 50 };
const KINDS: ClipKind[] = ["text", "html", "rtf", "image", "files", "custom"];

function kindLabel(t: ReturnType<typeof useI18n>["t"], kind: ClipKind) {
  return t(`history.kind${kind.charAt(0).toUpperCase()}${kind.slice(1)}`);
}

export function HistoryManager({
  groups,
  buffers,
  selectedClipId,
  onSelectedClipChange,
  onDataChanged,
  notify,
}: {
  groups: GroupNode[];
  buffers: CopyBuffer[];
  selectedClipId: number | null;
  onSelectedClipChange: (id: number | null) => void;
  onDataChanged: () => Promise<void>;
  notify: Notify;
}) {
  const { t, locale } = useI18n();
  const [query, setQuery] = useState("");
  const [groupFilter, setGroupFilter] = useState("all");
  const [kindFilter, setKindFilter] = useState<ClipKind | "">("");
  const [pinnedOnly, setPinnedOnly] = useState(false);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(50);
  const [data, setData] = useState<ClipPage>(EMPTY_PAGE);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reload, setReload] = useState(0);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [detail, setDetail] = useState<ClipDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [editText, setEditText] = useState("");
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [moveGroup, setMoveGroup] = useState("none");
  const [bufferSlot, setBufferSlot] = useState("1");
  const [busy, setBusy] = useState(false);

  const loadPage = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const next = query.trim()
        ? await commands.searchClips({
            query,
            page,
            pageSize,
            groupId: groupFilter === "all" ? undefined : Number(groupFilter),
            kind: kindFilter,
            pinnedOnly,
          })
        : await commands.listClips({
            page,
            pageSize,
            groupId: groupFilter === "all" ? undefined : Number(groupFilter),
            kind: kindFilter,
            pinnedOnly,
          });
      setData(next);
    } catch {
      setError(t("state.errorBody"));
    } finally {
      setLoading(false);
    }
  }, [groupFilter, kindFilter, page, pageSize, pinnedOnly, query, t]);

  useEffect(() => {
    const timeout = window.setTimeout(() => void loadPage(), query ? 180 : 0);
    return () => window.clearTimeout(timeout);
  }, [loadPage, query, reload]);

  useEffect(() => {
    setPage(1);
  }, [groupFilter, kindFilter, pageSize, pinnedOnly, query]);

  useEffect(() => {
    if (!selectedClipId) {
      setDetail(null);
      return;
    }
    let active = true;
    setDetailLoading(true);
    commands
      .getClip(selectedClipId)
      .then((clip) => {
        if (active) {
          setDetail(clip);
          setEditText(clip.text);
        }
      })
      .catch(() => {
        if (active) {
          setDetail(null);
        }
      })
      .finally(() => {
        if (active) {
          setDetailLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [selectedClipId]);

  const refreshAll = async () => {
    setReload((current) => current + 1);
    await onDataChanged();
  };

  const selectOnly = (id: number) => {
    setSelectedIds(new Set([id]));
    onSelectedClipChange(id);
  };

  const toggleSelection = (id: number) => {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      onSelectedClipChange(next.has(id) ? id : next.values().next().value ?? null);
      return next;
    });
  };

  const selectedArray = [...selectedIds];
  const pages = Math.max(1, Math.ceil(data.total / data.pageSize));

  const deleteSelected = async () => {
    if (!selectedArray.length) {
      return;
    }
    setBusy(true);
    try {
      await commands.deleteClips(selectedArray);
      notify("success", t("toast.clipDeleted"));
      setDeleteOpen(false);
      setSelectedIds(new Set());
      onSelectedClipChange(null);
      await refreshAll();
    } catch {
      notify("error", t("toast.clipDeleteFailed"));
    } finally {
      setBusy(false);
    }
  };

  const setPinned = async (pinned: boolean) => {
    if (!selectedArray.length) {
      return;
    }
    setBusy(true);
    try {
      await commands.setPinned(selectedArray, pinned);
      notify("success", t("toast.pinUpdated"));
      await refreshAll();
    } catch {
      notify("error", t("toast.pinFailed"));
    } finally {
      setBusy(false);
    }
  };

  const moveSelected = async () => {
    if (!selectedArray.length) {
      return;
    }
    setBusy(true);
    try {
      await commands.moveToGroup(
        selectedArray,
        moveGroup === "none" ? null : Number(moveGroup),
      );
      notify("success", t("toast.clipMoved"));
      await refreshAll();
    } catch {
      notify("error", t("toast.clipMoveFailed"));
    } finally {
      setBusy(false);
    }
  };

  const assignBuffer = async () => {
    const clipId = selectedClipId ?? selectedArray[0] ?? null;
    if (clipId === null) {
      return;
    }
    setBusy(true);
    try {
      await commands.setCopyBuffer({
        slot: Number(bufferSlot),
        clipId,
      });
      notify("success", t("toast.bufferUpdated"));
      await refreshAll();
    } catch {
      notify("error", t("toast.bufferUpdateFailed"));
    } finally {
      setBusy(false);
    }
  };

  const saveEdit = async () => {
    if (!detail) {
      return;
    }
    setBusy(true);
    try {
      await commands.editClip(detail.id, editText);
      notify("success", t("toast.clipUpdated"));
      setEditOpen(false);
      await refreshAll();
      setDetail({ ...detail, text: editText, previewText: editText.slice(0, 512) });
    } catch {
      notify("error", t("toast.clipUpdateFailed"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="history-layout">
      <SettingsCard className="history-list-card">
        <div className="history-filters">
          <Field label={t("common.search")}>
            <div className="search-field">
              <SearchRegular />
              <input
                className="native-search"
                value={query}
                placeholder={t("history.searchPlaceholder")}
                aria-label={t("history.searchPlaceholder")}
                onChange={(event) => setQuery(event.target.value)}
              />
              {query ? (
                <Button
                  appearance="subtle"
                  size="small"
                  onClick={() => setQuery("")}
                  aria-label={t("common.clearSearch")}
                >
                  {t("common.clear")}
                </Button>
              ) : null}
            </div>
          </Field>
          <SelectControl
            label={t("history.groupFilter")}
            value={groupFilter}
            options={[
              { value: "all", label: t("history.allGroups") },
              ...groups.map((group) => ({ value: String(group.id), label: group.name })),
            ]}
            onChange={setGroupFilter}
          />
          <SelectControl
            label={t("history.kindFilter")}
            value={kindFilter}
            options={[
              { value: "", label: t("history.allKinds") },
              ...KINDS.map((kind) => ({ value: kind, label: kindLabel(t, kind) })),
            ]}
            onChange={setKindFilter}
          />
          <Field label={t("history.pageSize")}>
            <SelectControl
              value={String(pageSize)}
              options={["50", "100", "250", "500"].map((value) => ({
                value,
                label: value,
              }))}
              onChange={(value) => setPageSize(Number(value))}
              ariaLabel={t("history.pageSize")}
            />
          </Field>
          <Checkbox
            checked={pinnedOnly}
            label={t("history.pinnedOnly")}
            onChange={(_, data) => setPinnedOnly(Boolean(data.checked))}
          />
        </div>

        <div className="history-actions">
          <Button
            icon={<EditRegular />}
            disabled={selectedIds.size !== 1 || busy}
            onClick={() => setEditOpen(true)}
          >
            {t("history.edit")}
          </Button>
          <Button
            icon={<PinRegular />}
            disabled={!selectedIds.size || busy}
            onClick={() => void setPinned(true)}
          >
            {t("history.pin")}
          </Button>
          <Button
            icon={<PinOffRegular />}
            disabled={!selectedIds.size || busy}
            onClick={() => void setPinned(false)}
          >
            {t("history.unpin")}
          </Button>
          <Button
            icon={<DeleteRegular />}
            disabled={!selectedIds.size || busy}
            onClick={() => setDeleteOpen(true)}
          >
            {t("history.delete")}
          </Button>
          <span className="selection-count">
            {t("history.selectedCount", { count: selectedIds.size })}
          </span>
        </div>

        {error ? (
          <MessageBar intent="error">
            <MessageBarBody>{error}</MessageBarBody>
            <Button appearance="subtle" onClick={() => void loadPage()}>
              {t("common.retry")}
            </Button>
          </MessageBar>
        ) : null}

        {loading ? (
          <div className="history-skeleton" aria-label={t("history.loading")}>
            {Array.from({ length: 6 }, (_, index) => (
              <Skeleton key={index}>
                <SkeletonItem size={20} />
              </Skeleton>
            ))}
          </div>
        ) : data.items.length === 0 ? (
          <EmptyState title={t("history.empty")} />
        ) : (
          <div className="history-table-wrap">
            <Table size="small" aria-label={t("data.tabHistory")}>
              <TableHeader>
                <TableRow>
                  <TableHeaderCell className="col-check" aria-label={t("common.selected")} />
                  <TableHeaderCell className="col-kind">{t("history.columnKind")}</TableHeaderCell>
                  <TableHeaderCell className="col-preview">{t("history.columnPreview")}</TableHeaderCell>
                  <TableHeaderCell className="col-source">{t("history.columnSource")}</TableHeaderCell>
                  <TableHeaderCell className="col-created">{t("history.columnCreated")}</TableHeaderCell>
                  <TableHeaderCell className="col-state">{t("history.columnState")}</TableHeaderCell>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.items.map((clip) => (
                  <TableRow
                    key={clip.id}
                    aria-selected={selectedClipId === clip.id}
                    className={selectedClipId === clip.id ? "history-row-selected" : ""}
                    tabIndex={0}
                    onClick={() => selectOnly(clip.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        selectOnly(clip.id);
                      }
                      if (event.key === " ") {
                        event.preventDefault();
                        toggleSelection(clip.id);
                      }
                    }}
                  >
                    <TableCell className="col-check">
                      <Checkbox
                        checked={selectedIds.has(clip.id)}
                        aria-label={t("common.selected")}
                        onClick={(event) => event.stopPropagation()}
                        onChange={() => toggleSelection(clip.id)}
                      />
                    </TableCell>
                    <TableCell className="col-kind">
                      <span className="kind-chip">{kindLabel(t, clip.kind)}</span>
                    </TableCell>
                    <TableCell className="col-preview">
                      <span className="clip-preview-cell">{clip.previewText}</span>
                    </TableCell>
                    <TableCell className="col-source">{clip.sourceApp}</TableCell>
                    <TableCell className="col-created">
                      {new Intl.DateTimeFormat(locale, {
                        dateStyle: "short",
                        timeStyle: "short",
                      }).format(clip.createdAtMs)}
                    </TableCell>
                    <TableCell className="col-state">
                      {clip.pinned ? <PinRegular aria-label={t("history.pinned")} /> : null}
                      {clip.copyBuffer ? (
                        <span className="buffer-badge">
                          {t("history.bufferBadge", { slot: clip.copyBuffer })}
                        </span>
                      ) : null}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}

        <div className="pagination-row">
          <span>{t("common.page", { page: data.page, pages })}</span>
          <Button
            icon={<ArrowLeftRegular />}
            disabled={data.page <= 1 || loading}
            onClick={() => setPage((current) => Math.max(1, current - 1))}
          >
            {t("common.previous")}
          </Button>
          <Button
            icon={<ArrowRightRegular />}
            iconPosition="after"
            disabled={data.page >= pages || loading}
            onClick={() => setPage((current) => Math.min(pages, current + 1))}
          >
            {t("common.next")}
          </Button>
        </div>
      </SettingsCard>

      <aside className="history-preview" aria-label={t("history.preview")}>
        <h2 className="card-title">{t("history.preview")}</h2>
        {detailLoading ? (
          <Skeleton>
            <SkeletonItem size={72} />
          </Skeleton>
        ) : detail ? (
          <>
            <div className="preview-meta">
              <span className="kind-chip">{kindLabel(t, detail.kind)}</span>
              <span>{detail.sourceApp}</span>
            </div>
            <p className="preview-text">{detail.text}</p>
            <dl className="preview-details">
              <div>
                <dt>{t("history.columnKind")}</dt>
                <dd>{kindLabel(t, detail.kind)}</dd>
              </div>
              <div>
                <dt>{t("history.columnSource")}</dt>
                <dd>{detail.sourceApp}</dd>
              </div>
            </dl>
            <Button icon={<EditRegular />} onClick={() => setEditOpen(true)}>
              {t("common.edit")}
            </Button>
          </>
        ) : (
          <EmptyState title={t("common.notSelected")} />
        )}
        {selectedIds.size ? (
          <div className="preview-actions">
            <SelectControl
              label={t("history.moveToGroup")}
              value={moveGroup}
              options={[
                { value: "none", label: t("history.ungrouped") },
                ...groups.map((group) => ({ value: String(group.id), label: group.name })),
              ]}
              onChange={setMoveGroup}
            />
            <Button icon={<TagRegular />} onClick={() => void moveSelected()} disabled={busy}>
              {t("common.move")}
            </Button>
            <SelectControl
              label={t("history.bufferSlot")}
              value={bufferSlot}
              options={buffers.map((buffer) => ({
                value: String(buffer.slot),
                label: buffer.name || t("copyBuffers.slot", { slot: buffer.slot }),
              }))}
              onChange={setBufferSlot}
            />
            <Button icon={<AddRegular />} onClick={() => void assignBuffer()} disabled={busy}>
              {t("common.assign")}
            </Button>
          </div>
        ) : null}
      </aside>

      <Dialog open={editOpen} onOpenChange={(_, value) => setEditOpen(value.open)}>
        <DialogSurface className="edit-dialog">
          <DialogBody>
            <DialogTitle>{t("history.editTitle")}</DialogTitle>
            <DialogContent>
              <p className="setting-help">{t("history.editHelp")}</p>
              <Field label={t("history.clipText")}>
                <Textarea
                  value={editText}
                  resize="vertical"
                  rows={10}
                  onChange={(_, value) => setEditText(value.value)}
                />
              </Field>
            </DialogContent>
            <DialogActions>
              <Button appearance="secondary" onClick={() => setEditOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button appearance="primary" disabled={busy} onClick={() => void saveEdit()}>
                {t("common.apply")}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <ConfirmDialog
        open={deleteOpen}
        title={t("history.deleteTitle")}
        body={t("history.deleteBody")}
        confirmLabel={t("common.delete")}
        cancelLabel={t("common.cancel")}
        danger
        busy={busy}
        onOpenChange={setDeleteOpen}
        onConfirm={() => void deleteSelected()}
      />
    </div>
  );
}
