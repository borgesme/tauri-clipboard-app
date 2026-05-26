import { Minimize2, Settings } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { DesktopSettings } from "@/types/clipboard";

interface DesktopSettingsPanelProps {
  settings: DesktopSettings | null;
  isBusy: boolean;
  onSettingsChange: (settings: DesktopSettings) => void;
  onHideWindow: () => void;
}

interface SettingsFormProps {
  settings: DesktopSettings;
  isBusy: boolean;
  onSettingsChange: (settings: DesktopSettings) => void;
  onHideWindow: () => void;
}

export function DesktopSettingsPanel(props: DesktopSettingsPanelProps) {
  return (
    <Card className="gap-4 border-border/70 bg-card/90 shadow-xl backdrop-blur">
      <PanelHeader />
      <CardContent className="space-y-3">
        {props.settings ? <SettingsForm {...props} settings={props.settings} /> : <LoadingState />}
      </CardContent>
    </Card>
  );
}

function PanelHeader() {
  return (
    <CardHeader className="pb-0">
      <CardDescription>Desktop</CardDescription>
      <CardTitle className="flex items-center gap-2 text-lg">
        <Settings className="size-5 text-primary" />
        桌面增强
      </CardTitle>
    </CardHeader>
  );
}

function LoadingState() {
  return (
    <div className="rounded-lg border border-dashed p-3 text-sm text-muted-foreground">
      正在加载桌面设置...
    </div>
  );
}

function SettingsForm(props: SettingsFormProps) {
  return (
    <>
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
      <Button className="w-full" variant="outline" onClick={props.onHideWindow}>
        <Minimize2 className="size-4" />
        隐藏到托盘
      </Button>
    </>
  );
}

function AutostartToggle({ settings, isBusy, onSettingsChange }: SettingsFormProps) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border bg-background/60 p-3">
      <div>
        <div className="text-sm font-medium">开机启动</div>
        <div className="text-xs text-muted-foreground">随系统启动后台工具</div>
      </div>
      <Button
        size="sm"
        variant={settings.autostartEnabled ? "default" : "outline"}
        disabled={isBusy}
        onClick={() => onSettingsChange({ ...settings, autostartEnabled: !settings.autostartEnabled })}
      >
        {settings.autostartEnabled ? "已开启" : "已关闭"}
      </Button>
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
    <label className="flex items-center justify-between gap-3 rounded-lg border bg-background/60 p-3">
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

function normalizeNumber(rawValue: string, min: number) {
  const parsed = Number.parseInt(rawValue, 10);
  if (Number.isNaN(parsed)) {
    return min;
  }
  return Math.max(min, parsed);
}
