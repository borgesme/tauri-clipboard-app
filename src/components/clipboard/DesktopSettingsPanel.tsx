import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Minimize2, Settings } from "lucide-react";

import { validateStorageDir } from "@/api/clipboard";
import { CustomSecretPatternsSetting, MaintenanceAction } from "@/components/clipboard/SettingsAdvancedActions";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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

interface SettingsFormProps {
  settings: DesktopSettings;
  isBusy: boolean;
  onSettingsChange: (settings: DesktopSettings) => void;
  onPurgeDeletedItems: () => void;
  onHideWindow: () => void;
}

export function DesktopSettingsPanel(props: DesktopSettingsPanelProps) {
  return (
    <Card className={cn("gap-4 border-border/70 bg-card/90 shadow-xl backdrop-blur", props.className)}>
      <PanelHeader />
      <CardContent className="grid gap-3 md:grid-cols-2">
        {props.settings ? <SettingsForm {...props} settings={props.settings} /> : <LoadingState />}
      </CardContent>
    </Card>
  );
}

function PanelHeader() {
  return (
    <CardHeader className="pb-0">
      <CardDescription>Settings</CardDescription>
      <CardTitle className="flex items-center gap-2 text-xl">
        <Settings className="size-5 text-primary" />
        设置
      </CardTitle>
    </CardHeader>
  );
}

function LoadingState() {
  return (
    <div className="rounded-lg border border-dashed p-3 text-sm text-muted-foreground md:col-span-2">
      正在加载桌面设置...
    </div>
  );
}

function SettingsForm(props: SettingsFormProps) {
  return (
    <>
      <MonitorToggle {...props} />
      <AutostartToggle {...props} />
      <NumberSetting
        label="保留天数"
        min={1}
        value={props.settings.retentionDays}
        onChange={(retentionDays) => props.onSettingsChange({ ...props.settings, retentionDays })}
      />
      <NumberSetting
        label="最大记录数"
        min={1}
        value={props.settings.maxRecordCount}
        onChange={(maxRecordCount) => props.onSettingsChange({ ...props.settings, maxRecordCount })}
      />
      <NumberSetting
        label="单条文本上限"
        min={1}
        value={props.settings.maxTextLength}
        onChange={(maxTextLength) => props.onSettingsChange({ ...props.settings, maxTextLength })}
      />
      <SecretFilterToggle {...props} />
      <CustomSecretPatternsSetting
        isBusy={props.isBusy}
        patterns={props.settings.customSecretPatterns}
        onChange={(customSecretPatterns) => props.onSettingsChange({ ...props.settings, customSecretPatterns })}
      />
      <StorageDirSetting
        isBusy={props.isBusy}
        storageDir={props.settings.storageDir}
        onChange={(storageDir) => props.onSettingsChange({ ...props.settings, storageDir })}
      />
      <MaintenanceAction isBusy={props.isBusy} onPurgeDeletedItems={props.onPurgeDeletedItems} />
      <Button className="h-auto rounded-xl py-3 md:col-span-2" variant="outline" onClick={props.onHideWindow}>
        <Minimize2 className="size-4" />
        隐藏到托盘
      </Button>
    </>
  );
}

function MonitorToggle({ settings, isBusy, onSettingsChange }: SettingsFormProps) {
  return (
    <SwitchSetting
      checked={settings.monitorEnabled}
      description="开启后自动捕获系统剪贴板文本"
      disabled={isBusy}
      label="剪贴板监听"
      onChange={(monitorEnabled) => onSettingsChange({ ...settings, monitorEnabled })}
    />
  );
}

function SecretFilterToggle({ settings, isBusy, onSettingsChange }: SettingsFormProps) {
  return (
    <SwitchSetting
      checked={settings.ignorePasswordLikeText}
      description="跳过疑似 JWT、API Key、长 token"
      disabled={isBusy}
      label="敏感内容过滤"
      onChange={(ignorePasswordLikeText) => onSettingsChange({ ...settings, ignorePasswordLikeText })}
    />
  );
}

function AutostartToggle({ settings, isBusy, onSettingsChange }: SettingsFormProps) {
  return (
    <SwitchSetting
      checked={settings.autostartEnabled}
      description="随系统启动后台工具"
      disabled={isBusy}
      label="开机启动"
      onChange={(autostartEnabled) => onSettingsChange({ ...settings, autostartEnabled })}
    />
  );
}

function SwitchSetting({
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
    <div className="flex items-center justify-between gap-3 rounded-xl border bg-background/60 p-3">
      <div>
        <div className="text-sm font-medium">{label}</div>
        <div className="text-xs text-muted-foreground">{description}</div>
      </div>
      <Switch checked={checked} disabled={disabled} onCheckedChange={onChange} />
    </div>
  );
}

function NumberSetting({
  label,
  min,
  value,
  onChange,
}: {
  label: string;
  min: number;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-3 rounded-xl border bg-background/60 p-3">
      <span className="flex items-center gap-2 text-sm font-medium">
        {label}
        <Badge variant="outline">{value}</Badge>
      </span>
      <input
        className="h-8 w-20 rounded-md border bg-background px-2 text-right text-sm outline-none focus:ring-2 focus:ring-ring/40"
        min={min}
        type="number"
        value={value}
        onChange={(event) => onChange(normalizeNumber(event.currentTarget.value, min))}
      />
    </label>
  );
}

function StorageDirSetting({
  isBusy,
  storageDir,
  onChange,
}: {
  isBusy: boolean;
  storageDir: string;
  onChange: (storageDir: string) => void;
}) {
  const [draft, setDraft] = useState(storageDir);
  const [errorMessage, setErrorMessage] = useState("");
  const hasChanged = draft.trim() !== storageDir;

  useEffect(() => {
    setDraft(storageDir);
    setErrorMessage("");
  }, [storageDir]);

  return (
    <div className="space-y-2 rounded-xl border bg-background/60 p-3 md:col-span-2">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="text-sm font-medium">本地存储目录</div>
          <div className="text-xs text-muted-foreground">留空使用默认应用数据目录</div>
        </div>
        <div className="flex items-center gap-2">
          <Button disabled={isBusy} size="sm" variant="outline" onClick={() => void selectDirectory(setDraft)}>
            选择目录
          </Button>
          <Button
            disabled={isBusy || !hasChanged}
            size="sm"
            onClick={() => void saveStorageDir(draft, onChange, setErrorMessage)}
          >
            保存路径
          </Button>
        </div>
      </div>
      <input
        className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring/40"
        disabled={isBusy}
        placeholder="例如 D:\\ClipboardData"
        value={draft}
        onChange={(event) => setDraft(event.currentTarget.value)}
      />
      {errorMessage ? <p className="text-xs text-destructive">{errorMessage}</p> : null}
      <p className="text-xs text-muted-foreground">
        应用会在该目录下创建 clipboard.sqlite；切换目录不会自动迁移旧数据。
      </p>
    </div>
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
  onChange: (storageDir: string) => void,
  setErrorMessage: (value: string) => void,
) {
  const storageDir = draft.trim();
  try {
    await validateStorageDir(storageDir);
    setErrorMessage("");
    onChange(storageDir);
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
