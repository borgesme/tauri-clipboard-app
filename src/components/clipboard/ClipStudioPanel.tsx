import { useState } from "react";
import { Copy, Send, Settings, WandSparkles, X } from "lucide-react";

import { ClipStudioCalendarPanel } from "@/components/clipboard/ClipStudioCalendarPanel";
import { CodeBlock } from "@/components/clipboard/CodeBlock";
import { DesktopSettingsPanel } from "@/components/clipboard/DesktopSettingsPanel";
import { Kbd } from "@/components/clipboard/ClipStudioLayout";
import {
  type PanelTab,
  type ToolboxAction,
  createToolboxResult,
  getClipKindFromContent,
} from "@/components/clipboard/clipStudioHelpers";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ClipboardDateGroup, ClipboardItem, DesktopSettings } from "@/types/clipboard";

export interface ClipStudioPanelProps {
  activeTab: PanelTab;
  dates: ClipboardDateGroup[];
  selectedDate: string;
  today: string;
  frequentCount: number;
  selectedItem: ClipboardItem | null;
  toolboxText: string;
  toolboxResult: string;
  desktopSettings: DesktopSettings | null;
  isBusy: boolean;
  drawerOpen: boolean;
  onTabChange: (tab: PanelTab) => void;
  onCloseDrawer: () => void;
  onDateSelect: (date: string) => void;
  onToolboxTextChange: (value: string) => void;
  onToolboxResultChange: (value: string) => void;
  onSendSelectedToToolbox: () => void;
  onCopyToolboxResult: () => void;
  onSettingsChange: (settings: DesktopSettings) => void;
  onPurgeDeletedItems: () => void;
  onHideWindow: () => void;
}

const tabs: Array<{ value: PanelTab; label: string }> = [
  { value: "calendar", label: "日期" },
  { value: "toolbox", label: "工具箱" },
  { value: "settings", label: "设置" },
];

const toolActions: Array<{ value: ToolboxAction; label: string }> = [
  { value: "trim", label: "清理空格" },
  { value: "upper", label: "转大写" },
  { value: "lower", label: "转小写" },
  { value: "markdown", label: "生成 Markdown 链接" },
];

export function ClipStudioPanel(props: ClipStudioPanelProps) {
  return (
    <>
      <button
        aria-label={`打开${getTabLabel(props.activeTab)}抽屉`}
        className="clip-panel-handle"
        type="button"
        onClick={() => props.onTabChange(props.activeTab)}
      >
        {getTabLabel(props.activeTab)}
      </button>
      <button
        aria-label="关闭右侧抽屉"
        className={cn("clip-panel-scrim", props.drawerOpen && "open")}
        type="button"
        onClick={props.onCloseDrawer}
      />
      <aside className={cn("clip-panel", props.drawerOpen && "open")}>
        <TabBar activeTab={props.activeTab} onCloseDrawer={props.onCloseDrawer} onTabChange={props.onTabChange} />
        <div className="clip-panel-body">
          {props.activeTab === "calendar" ? <CalendarPanel {...props} /> : null}
          {props.activeTab === "toolbox" ? <ToolboxPanel {...props} /> : null}
          {props.activeTab === "settings" ? <SettingsPanel {...props} /> : null}
        </div>
      </aside>
    </>
  );
}

function TabBar({ activeTab, onCloseDrawer, onTabChange }: Pick<ClipStudioPanelProps, "activeTab" | "onCloseDrawer" | "onTabChange">) {
  return (
    <div className="clip-tabs" aria-label="右侧面板">
      {tabs.map((tab) => (
        <button
          className={cn("clip-tab", activeTab === tab.value && "active")}
          key={tab.value}
          type="button"
          onClick={() => onTabChange(tab.value)}
        >
          {tab.label}
        </button>
      ))}
      <button className="clip-panel-close" type="button" onClick={onCloseDrawer}>
        <X className="size-4" />
      </button>
    </div>
  );
}

function getTabLabel(tab: PanelTab) {
  return tabs.find((item) => item.value === tab)?.label ?? "面板";
}

function CalendarPanel(props: ClipStudioPanelProps) {
  return <ClipStudioCalendarPanel {...props} />;
}

function ToolboxPanel(props: ClipStudioPanelProps) {
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const isCode = getClipKindFromContent(props.toolboxText) === "code";
  const showPreview = mode === "preview" && isCode;
  return (
    <section className="clip-panel-view">
      <InfoCard icon={<WandSparkles className="size-4" />} title="文本处理工具箱">
        按 <Kbd>T</Kbd> 可把当前选中的剪贴板内容送入这里，减少鼠标操作。
      </InfoCard>
      <div className="panel-card">
        <div className="toolbox-head">
          <Button className="clip-primary-button" size="sm" onClick={props.onSendSelectedToToolbox} disabled={!props.selectedItem}>
            <Send className="size-4" />
            送入工具箱
          </Button>
          {isCode ? (
            <div className="toolbox-mode-switch">
              <button
                type="button"
                className={cn("toolbox-mode-button", mode === "edit" && "active")}
                onClick={() => setMode("edit")}
              >
                编辑
              </button>
              <button
                type="button"
                className={cn("toolbox-mode-button", mode === "preview" && "active")}
                onClick={() => setMode("preview")}
              >
                预览
              </button>
            </div>
          ) : null}
        </div>
        {showPreview ? (
          <CodeBlock content={props.toolboxText} className="toolbox-preview" />
        ) : (
          <textarea
            className="toolbox-input"
            value={props.toolboxText}
            onChange={(event) => props.onToolboxTextChange(event.currentTarget.value)}
          />
        )}
        <div className="tool-grid">
          {toolActions.map((action) => (
            <button
              className="tool-button"
              key={action.value}
              type="button"
              onClick={() => props.onToolboxResultChange(createToolboxResult(action.value, props.toolboxText))}
            >
              {action.label}
            </button>
          ))}
        </div>
        <div className="tool-result">{props.toolboxResult || "转换结果会显示在这里。"}</div>
        <Button className="clip-primary-button mt-3" onClick={props.onCopyToolboxResult} disabled={!props.toolboxResult.trim()}>
          <Copy className="size-4" />
          复制结果
        </Button>
      </div>
    </section>
  );
}

function SettingsPanel(props: ClipStudioPanelProps) {
  return (
    <section className="clip-panel-view">
      <InfoCard icon={<Settings className="size-4" />} title="设置">
        保留监听、敏感过滤、历史保留策略、本地存储目录和托盘隐藏等桌面能力。
      </InfoCard>
      <DesktopSettingsPanel
        settings={props.desktopSettings}
        isBusy={props.isBusy}
        onSettingsChange={props.onSettingsChange}
        onPurgeDeletedItems={props.onPurgeDeletedItems}
        onHideWindow={props.onHideWindow}
      />
    </section>
  );
}

function InfoCard({ children, icon, title }: { children: React.ReactNode; icon: React.ReactNode; title: string }) {
  return (
    <div className="panel-card">
      <h2>
        {icon}
        {title}
      </h2>
      <p>{children}</p>
    </div>
  );
}
