import type { ReactNode } from "react";
import { ChevronRight } from "lucide-react";

import { cn } from "@/lib/utils";

interface ClipStudioLayoutProps {
  totalCount: number;
  frequentCount: number;
  monitorEnabled: boolean;
  message: string;
  children: ReactNode;
  onOpenCalendar: () => void;
  onOpenToolbox: () => void;
  onOpenSettings: () => void;
  onShowFrequent: () => void;
  onToggleMonitor: () => void;
}

export function ClipStudioLayout(props: ClipStudioLayoutProps) {
  return (
    <section className="clip-app" aria-label="Clip Studio 剪贴板工作台">
      <div className="clip-workspace">
        <Sidebar {...props} />
        {props.children}
      </div>
      <StatusBar message={props.message} />
    </section>
  );
}

function Sidebar(props: ClipStudioLayoutProps) {
  return (
    <aside className="clip-sidebar">
      <div>
        <Brand />
        <nav className="clip-nav" aria-label="剪贴板导航">
          <NavLabel>空间</NavLabel>
          <NavButton active icon="⌘" label="剪贴板历史" count={props.totalCount} />
          <NavButton icon="★" label="高频片段" count={props.frequentCount} onClick={props.onShowFrequent} />
          <NavButton icon="✎" label="文本处理" onClick={props.onOpenToolbox} />
          <NavButton icon="◷" label="日期看板" onClick={props.onOpenCalendar} />
          <NavLabel>偏好</NavLabel>
          <NavButton icon="⚙" label="设置" onClick={props.onOpenSettings} />
        </nav>
      </div>
      <div />
      <MonitorStatus enabled={props.monitorEnabled} onToggle={props.onToggleMonitor} />
    </aside>
  );
}

function Brand() {
  return (
    <div className="clip-brand">
      <div className="clip-mark">C</div>
      <div>
        <div className="clip-name">Clip Studio</div>
        <div className="clip-subtitle">私有剪贴板中心</div>
      </div>
    </div>
  );
}

function NavLabel({ children }: { children: ReactNode }) {
  return <div className="clip-nav-label">{children}</div>;
}

function NavButton({
  active = false,
  count,
  icon,
  label,
  onClick,
}: {
  active?: boolean;
  count?: number;
  icon: string;
  label: string;
  onClick?: () => void;
}) {
  return (
    <button className={cn("clip-nav-item", active && "active")} type="button" onClick={onClick}>
      <span>{icon}</span>
      <span className="nav-text">{label}</span>
      {typeof count === "number" ? <span className="clip-count">{count}</span> : <ChevronRight className="ml-auto size-3" />}
    </button>
  );
}

function MonitorStatus({ enabled, onToggle }: { enabled: boolean; onToggle: () => void }) {
  return (
    <button className="clip-monitor" type="button" onClick={onToggle}>
      <div className="monitor-row">
        <span>
          <i className={cn("monitor-dot", enabled && "enabled")} />
          {enabled ? "本地监听中" : "监听已暂停"}
        </span>
        <Kbd>{enabled ? "24h" : "off"}</Kbd>
      </div>
      <div className="usage-bar"><span style={{ width: enabled ? "72%" : "22%" }} /></div>
      <div className="monitor-help">点击可快速切换剪贴板监听状态。</div>
    </button>
  );
}

function StatusBar({ message }: { message: string }) {
  return (
    <footer className="clip-statusbar">
      <span>{message || "Ready"}</span>
      <span>窗口可调整 · 自动保存尺寸 · 键盘优先增强</span>
    </footer>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <span className="clip-kbd">{children}</span>;
}
