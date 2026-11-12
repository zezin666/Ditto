import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Dropdown,
  Field,
  Option,
  Text,
  type DropdownProps,
} from "@fluentui/react-components";
import { DismissRegular, KeyboardRegular } from "@fluentui/react-icons";
import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { useI18n } from "../i18n";

export function SettingsCard({
  title,
  children,
  className = "",
}: {
  title?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`settings-card ${className}`.trim()}>
      {title ? <h2 className="card-title">{title}</h2> : null}
      <div className="card-content">{children}</div>
    </section>
  );
}

export function SettingRow({
  label,
  help,
  children,
  align = "center",
}: {
  label: string;
  help?: string;
  children: ReactNode;
  align?: "center" | "start";
}) {
  return (
    <div className={`setting-row setting-row-${align}`}>
      <div className="setting-copy">
        <span className="setting-label">{label}</span>
        {help ? <span className="setting-help">{help}</span> : null}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

export interface SelectOption<T extends string> {
  value: T;
  label: string;
}

export function SelectControl<T extends string>({
  label,
  value,
  options,
  onChange,
  disabled = false,
  ariaLabel,
}: {
  label?: string;
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
  ariaLabel?: string;
}) {
  const selected = options.find((option) => option.value === value);
  const handleOptionSelect: DropdownProps["onOptionSelect"] = (_, data) => {
    if (data.optionValue) {
      onChange(data.optionValue as T);
    }
  };
  const dropdown = (
    <Dropdown
      aria-label={ariaLabel ?? label}
      className="select-control"
      value={selected?.label ?? value}
      selectedOptions={[value]}
      onOptionSelect={handleOptionSelect}
      disabled={disabled}
    >
      {options.map((option) => (
        <Option key={option.value} value={option.value}>
          {option.label}
        </Option>
      ))}
    </Dropdown>
  );
  return label ? <Field label={label}>{dropdown}</Field> : dropdown;
}

export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  cancelLabel,
  danger = false,
  busy = false,
  onConfirm,
  onOpenChange,
}: {
  open: boolean;
  title: string;
  body: string;
  confirmLabel: string;
  cancelLabel: string;
  danger?: boolean;
  busy?: boolean;
  onConfirm: () => void;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={(_, data) => onOpenChange(data.open)}>
      <DialogSurface className="confirm-surface">
        <DialogBody>
          <DialogTitle>{title}</DialogTitle>
          <DialogContent>
            <Text>{body}</Text>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => onOpenChange(false)} disabled={busy}>
              {cancelLabel}
            </Button>
            <Button
              appearance="primary"
              className={danger ? "danger-button" : ""}
              onClick={onConfirm}
              disabled={busy}
            >
              {confirmLabel}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

function formatKeyEvent(event: globalThis.KeyboardEvent | KeyboardEvent<HTMLElement>) {
  const modifierKeys = new Set(["Control", "Alt", "Shift", "Meta"]);
  if (modifierKeys.has(event.key)) {
    return null;
  }
  const parts: string[] = [];
  if (event.ctrlKey) {
    parts.push("Ctrl");
  }
  if (event.altKey) {
    parts.push("Alt");
  }
  if (event.shiftKey) {
    parts.push("Shift");
  }
  if (event.metaKey) {
    parts.push("Win");
  }
  let key = event.key;
  if (key === " ") {
    key = "Space";
  } else if (key.length === 1) {
    key = key.toUpperCase();
  }
  parts.push(key);
  return parts.join("+");
}

export function HotkeyRecorder({
  value,
  actionLabel,
  onChange,
  compact = false,
  disabled = false,
}: {
  value: string;
  actionLabel: string;
  onChange: (value: string) => void;
  compact?: boolean;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  const [recording, setRecording] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!recording || disabled) {
      return;
    }
    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        setRecording(false);
        return;
      }
      if (event.key === "Backspace" || event.key === "Delete") {
        onChange("");
        setRecording(false);
        return;
      }
      const formatted = formatKeyEvent(event);
      if (formatted) {
        onChange(formatted);
        setRecording(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [onChange, recording]);

  return (
    <div className={`hotkey-recorder ${compact ? "hotkey-recorder-compact" : ""}`}>
      <span className={`hotkey-chip ${value ? "" : "hotkey-chip-empty"}`}>
        {value || t("hotkeys.emptyBinding")}
      </span>
      <Button
        ref={buttonRef}
        appearance="subtle"
        icon={<KeyboardRegular />}
        aria-label={t("hotkeys.record")}
        title={recording ? t("hotkeys.recording") : t("hotkeys.recordHint", { action: actionLabel })}
        className={recording ? "recording-button" : ""}
        disabled={disabled}
        onClick={() => setRecording((current) => !current)}
      >
        {recording ? t("hotkeys.recording") : compact ? "" : t("hotkeys.record")}
      </Button>
      {value ? (
        <Button
          appearance="subtle"
          icon={<DismissRegular />}
          aria-label={t("hotkeys.clearBinding")}
          title={t("hotkeys.clearBinding")}
          disabled={disabled}
          onClick={() => onChange("")}
        />
      ) : null}
    </div>
  );
}

export function EmptyState({ title, body }: { title: string; body?: string }) {
  const id = useId();
  return (
    <div className="empty-state" role="status" aria-labelledby={id}>
      <h3 id={id}>{title}</h3>
      {body ? <p>{body}</p> : null}
    </div>
  );
}

export function formatBytes(value: number, t: ReturnType<typeof useI18n>["t"]) {
  if (value < 1024) {
    return t("units.bytes", { value });
  }
  if (value < 1024 * 1024) {
    return t("units.kib", { value: (value / 1024).toFixed(1) });
  }
  if (value < 1024 * 1024 * 1024) {
    return t("units.mib", { value: (value / 1024 / 1024).toFixed(1) });
  }
  return t("units.gib", { value: (value / 1024 / 1024 / 1024).toFixed(1) });
}
