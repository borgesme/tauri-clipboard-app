import { useEffect, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { validateStorageDir } from "@/api/clipboard";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import type { DesktopSettings } from "@/types/clipboard";

interface DesktopSettingsPanelProps {
  settings: DesktopSettings | null;
  isBusy: boolean;
  className?: string;
  onSettingsChange: (settings: DesktopSettings) => void;
  onPurgeDeletedItems: () => void;
  onHideWindow: () => void;
}

interface SettingsFormProps extends Omit<DesktopSettingsPanelProps, "className" | "settings"> {
  settings: DesktopSettings;
}

export function DesktopSettingsPanel(props: DesktopSettingsPanelProps) {
  return (
    <div className={cn("settings-card", props.className)}>
      {props.settings ? <SettingsForm {...props} settings={props.settings} /> : <LoadingState />}
    </div>
  );
}

function LoadingState() {
  return <div className="settings-empty">正在加载桌面设置...</div>;
}

function SettingsForm(props: SettingsFormProps) {
  return (
    <>
      <SwitchRow
        checked={props.settings.monitorEnabled}
        description="开启后自动捕获系统剪贴板文本。"
        disabled={props.isBusy}
        label="剪贴板监听"
        onChange={(monitorEnabled) => props.onSettingsChange({ ...props.settings, monitorEnabled })}
      />
      <SwitchRow
        checked={props.settings.ignorePasswordLikeText}
        description="疑似 JWT、API Key、长 token 会按敏感内容跳过。"
        disabled={props.isBusy}
        label="敏感内容过滤"
        onChange={(ignorePasswordLikeText) => props.onSettingsChange({ ...props.settings, ignorePasswordLikeText })}
      />
      <SwitchRow
        checked={props.settings.autostartEnabled}
        description="随系统启动后在后台运行。"
        disabled={props.isBusy}
        label="开机启动"
        onChange={(autostartEnabled) => props.onSettingsChange({ ...props.settings, autostartEnabled })}
      />
      <AdvancedSettingsSection {...props} />
      <StorageDirRow {...props} />
      <ActionRow label="数据维护" description="物理删除已移入回收状态的记录并压缩数据库。">
        <Button className="settings-button" disabled={props.isBusy} size="sm" variant="outline" onClick={props.onPurgeDeletedItems}>
          清理
        </Button>
      </ActionRow>
      <ActionRow label="托盘运行" description="隐藏主窗口，继续在后台监听剪贴板。">
        <Button className="settings-button" size="sm" variant="outline" onClick={props.onHideWindow}>
          隐藏
        </Button>
      </ActionRow>
    </>
  );
}

function AdvancedSettingsSection({ settings, isBusy, onSettingsChange }: SettingsFormProps) {
  const [draft, setDraft] = useState({
    retentionDays: settings.retentionDays,
    maxRecordCount: settings.maxRecordCount,
    maxTextLength: settings.maxTextLength,
    customSecretPatterns: settings.customSecretPatterns,
  });

  useEffect(() => {
    setDraft({
      retentionDays: settings.retentionDays,
      maxRecordCount: settings.maxRecordCount,
      maxTextLength: settings.maxTextLength,
      customSecretPatterns: settings.customSecretPatterns,
    });
  }, [
    settings.retentionDays,
    settings.maxRecordCount,
    settings.maxTextLength,
    settings.customSecretPatterns,
  ]);

  const hasChanged =
    draft.retentionDays !== settings.retentionDays ||
    draft.maxRecordCount !== settings.maxRecordCount ||
    draft.maxTextLength !== settings.maxTextLength ||
    draft.customSecretPatterns !== settings.customSecretPatterns;

  return (
    <>
      <ActionRow label="默认保留时长" description="超过期限的非固定记录自动清理。">
        <NumberInput
          min={1}
          suffix="天"
          value={draft.retentionDays}
          onChange={(retentionDays) => setDraft((current) => ({ ...current, retentionDays }))}
        />
      </ActionRow>
      <ActionRow label="记录容量" description="控制最大记录数和单条文本长度。">
        <div className="settings-inline-inputs">
          <NumberInput
            min={1}
            suffix="条"
            value={draft.maxRecordCount}
            onChange={(maxRecordCount) => setDraft((current) => ({ ...current, maxRecordCount }))}
          />
          <NumberInput
            min={1}
            suffix="字"
            value={draft.maxTextLength}
            onChange={(maxTextLength) => setDraft((current) => ({ ...current, maxTextLength }))}
          />
        </div>
      </ActionRow>
      <div className="setting vertical">
        <SettingText label="自定义敏感正则" description="每行一条正则；匹配内容会按敏感内容跳过。" />
        <textarea
          className="settings-pattern-input"
          disabled={isBusy}
          placeholder="例如 ^corp_[A-Za-z0-9]{24}$"
          value={draft.customSecretPatterns}
          onChange={(event) => {
            const value = event.currentTarget.value;
            setDraft((current) => ({ ...current, customSecretPatterns: value }));
          }}
        />
      </div>
      <ActionRow label="高级设置" description="保留时长、容量与正则改动后点击保存生效。">
        <Button
          className="settings-button"
          disabled={isBusy || !hasChanged}
          size="sm"
          onClick={() => onSettingsChange({ ...settings, ...draft })}
        >
          保存设置
        </Button>
      </ActionRow>
    </>
  );
}

function StorageDirRow({ settings, isBusy, onSettingsChange }: SettingsFormProps) {
  const [draft, setDraft] = useState(settings.storageDir);
  const [errorMessage, setErrorMessage] = useState("");
  const hasChanged = draft.trim() !== settings.storageDir;

  useEffect(() => {
    setDraft(settings.storageDir);
    setErrorMessage("");
  }, [settings.storageDir]);

  return (
    <div className="setting vertical">
      <SettingText label="本地存储目录" description="留空使用默认应用数据目录；切换目录不会自动迁移旧数据。" />
      <div className="settings-storage-row">
        <input
          className="settings-text-input"
          disabled={isBusy}
          placeholder="例如 D:\\ClipboardData"
          value={draft}
          onChange={(event) => setDraft(event.currentTarget.value)}
        />
        <Button className="settings-button" disabled={isBusy} size="sm" variant="outline" onClick={() => void selectDirectory(setDraft)}>
          选择
        </Button>
        <Button
          className="settings-button"
          disabled={isBusy || !hasChanged}
          size="sm"
          onClick={() => void saveStorageDir(draft, settings, onSettingsChange, setErrorMessage)}
        >
          保存
        </Button>
      </div>
      {errorMessage ? <p className="settings-error">{errorMessage}</p> : null}
    </div>
  );
}

function SwitchRow({
  checked,
  description,
  disabled,
  label,
  onChange,
}: {
  checked: boolean;
  description: string;
  disabled: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <ActionRow label={label} description={description}>
      <Switch className="settings-switch" aria-label={label} checked={checked} disabled={disabled} onCheckedChange={onChange} />
    </ActionRow>
  );
}

function ActionRow({ children, description, label }: { children: ReactNode; description: string; label: string }) {
  return (
    <div className="setting">
      <SettingText label={label} description={description} />
      <div className="setting-control">{children}</div>
    </div>
  );
}

function SettingText({ description, label }: { description: string; label: string }) {
  return (
    <div className="setting-text">
      <b>{label}</b>
      <small>{description}</small>
    </div>
  );
}

function NumberInput({
  min,
  suffix,
  value,
  onChange,
}: {
  min: number;
  suffix: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="settings-number">
      <input min={min} type="number" value={value} onChange={(event) => onChange(normalizeNumber(event.currentTarget.value, min))} />
      <span>{suffix}</span>
    </label>
  );
}

async function selectDirectory(setDraft: (value: string) => void) {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") {
    setDraft(selected);
  }
}

async function saveStorageDir(
  draft: string,
  settings: DesktopSettings,
  onChange: (settings: DesktopSettings) => void,
  setErrorMessage: (value: string) => void,
) {
  const storageDir = draft.trim();
  try {
    await validateStorageDir(storageDir);
    setErrorMessage("");
    onChange({ ...settings, storageDir });
  } catch (error) {
    setErrorMessage(String(error));
  }
}

function normalizeNumber(rawValue: string, min: number) {
  const parsed = Number.parseInt(rawValue, 10);
  if (Number.isNaN(parsed)) {
    return min;
  }
  return Math.max(min, parsed);
}
