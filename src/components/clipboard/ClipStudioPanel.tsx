import { CalendarDays, Copy, Send, Settings, WandSparkles } from "lucide-react";

import { DesktopSettingsPanel } from "@/components/clipboard/DesktopSettingsPanel";
import { Kbd } from "@/components/clipboard/ClipStudioLayout";
import {
  type PanelTab,
  type ToolboxAction,
  countRecords,
  countTodayRecords,
  createToolboxResult,
} from "@/components/clipboard/clipStudioHelpers";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ClipboardDateGroup, ClipboardItem, DesktopSettings } from "@/types/clipboard";

interface ClipStudioPanelProps {
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
  onTabChange: (tab: PanelTab) => void;
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
    <aside className="clip-panel">
      <TabBar activeTab={props.activeTab} onTabChange={props.onTabChange} />
      <div className="clip-panel-body">
        {props.activeTab === "calendar" ? <CalendarPanel {...props} /> : null}
        {props.activeTab === "toolbox" ? <ToolboxPanel {...props} /> : null}
        {props.activeTab === "settings" ? <SettingsPanel {...props} /> : null}
      </div>
    </aside>
  );
}

function TabBar({ activeTab, onTabChange }: Pick<ClipStudioPanelProps, "activeTab" | "onTabChange">) {
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
    </div>
  );
}

function CalendarPanel(props: ClipStudioPanelProps) {
  return (
    <section className="clip-panel-view">
      <InfoCard icon={<CalendarDays className="size-4" />} title="日期看板">
        保留原来的日期浏览方式。点击有记录日期后，中间列表会切换到当天记录。
      </InfoCard>
      <div className="panel-card">
        <div className="date-summary">
          <DateStat label="今日记录" value={countTodayRecords(props.dates, props.today)} />
          <DateStat label="高频复用" value={props.frequentCount} />
          <DateStat label="总记录" value={countRecords(props.dates)} />
        </div>
      </div>
      <div className="panel-card calendar-list">
        {props.dates.length === 0 ? <div className="mini-empty">暂无日期分组</div> : null}
        {props.dates.map((group) => (
          <button
            className={cn("date-button", props.selectedDate === group.date && "active")}
            key={group.date}
            type="button"
            onClick={() => props.onDateSelect(group.date)}
          >
            <span>{group.date}</span>
            <span>{group.count} 条</span>
          </button>
        ))}
      </div>
    </section>
  );
}

function ToolboxPanel(props: ClipStudioPanelProps) {
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
        </div>
        <textarea
          className="toolbox-input"
          value={props.toolboxText}
          onChange={(event) => props.onToolboxTextChange(event.currentTarget.value)}
        />
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

function DateStat({ label, value }: { label: string; value: number }) {
  return (
    <div className="date-stat">
      <b>{value}</b>
      <span>{label}</span>
    </div>
  );
}
