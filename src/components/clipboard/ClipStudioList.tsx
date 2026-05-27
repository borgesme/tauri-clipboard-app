import type { Ref } from "react";
import { Copy, Eye, Search, Trash2, X } from "lucide-react";

import { Kbd } from "@/components/clipboard/ClipStudioLayout";
import {
  type ClipFilter,
  formatTime,
  getClipIcon,
  getClipKind,
  getClipKindLabel,
} from "@/components/clipboard/clipStudioHelpers";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ClipboardItem } from "@/types/clipboard";

interface ClipStudioListProps {
  items: ClipboardItem[];
  selectedItem: ClipboardItem | null;
  selectedDate: string;
  searchInputRef?: Ref<HTMLInputElement>;
  searchTerm: string;
  activeFilter: ClipFilter;
  errorMessage: string;
  isBusy: boolean;
  onSearchChange: (value: string) => void;
  onClearSearch: () => void;
  onClearDate: () => void;
  onFilterChange: (filter: ClipFilter) => void;
  onSelectItem: (id: number) => void;
  onCopyItem: (item: ClipboardItem) => void;
  onDeleteItem: (item: ClipboardItem) => void;
  onOpenDetail: (item: ClipboardItem) => void;
}

const filters: Array<{ value: ClipFilter; label: string }> = [
  { value: "all", label: "全部" },
  { value: "text", label: "文本" },
  { value: "link", label: "链接" },
  { value: "code", label: "代码" },
  { value: "secret", label: "敏感" },
  { value: "frequent", label: "高频" },
];

export function ClipStudioList(props: ClipStudioListProps) {
  return (
    <main className="clip-main">
      <SearchRow {...props} />
      <FilterRow activeFilter={props.activeFilter} onFilterChange={props.onFilterChange} />
      <KeyboardStrip />
      <section className="clip-list" tabIndex={0} aria-label="剪贴板记录列表">
        <ListHeader {...props} />
        {props.errorMessage ? <div className="clip-error">{props.errorMessage}</div> : null}
        {props.items.length === 0 ? <EmptyState {...props} /> : <Items {...props} />}
      </section>
    </main>
  );
}

function SearchRow(props: ClipStudioListProps) {
  const hasSearch = props.searchTerm.trim().length > 0;
  return (
    <div className="clip-search-row">
      <label className="clip-searchbox">
        <Search className="size-4" />
        <input
          aria-label="搜索剪贴板记录"
          placeholder="搜索内容、来源应用、链接或代码片段…"
          ref={props.searchInputRef}
          value={props.searchTerm}
          onChange={(event) => props.onSearchChange(event.currentTarget.value)}
        />
      </label>
      <Button className="clip-dark-button" onClick={hasSearch ? props.onClearSearch : props.onClearDate}>
        {hasSearch ? <X className="size-4" /> : null}
        {hasSearch ? "清空搜索" : "清空当天"}
      </Button>
    </div>
  );
}

function FilterRow({
  activeFilter,
  onFilterChange,
}: Pick<ClipStudioListProps, "activeFilter" | "onFilterChange">) {
  return (
    <div className="clip-filter-row" aria-label="记录类型筛选">
      {filters.map((filter) => (
        <button
          className={cn("clip-chip", activeFilter === filter.value && "active")}
          key={filter.value}
          type="button"
          onClick={() => onFilterChange(filter.value)}
        >
          {filter.label}
        </button>
      ))}
    </div>
  );
}

function KeyboardStrip() {
  return (
    <div className="keyboard-strip">
      <span><Kbd>/</Kbd> 搜索</span>
      <span><Kbd>↑ ↓</Kbd> 选择</span>
      <span><Kbd>Enter</Kbd> 复制</span>
      <span><Kbd>Space</Kbd> 详情</span>
      <span><Kbd>T</Kbd> 工具箱</span>
      <span><Kbd>Esc</Kbd> 返回全部</span>
    </div>
  );
}

function ListHeader({ items, searchTerm, selectedDate, activeFilter }: ClipStudioListProps) {
  const title = searchTerm.trim() ? "搜索结果" : selectedDate;
  const subtitle = activeFilter === "all" ? "History" : `${getFilterLabel(activeFilter)} Filter`;
  return (
    <div className="clip-group-head">
      <div className="group-title">
        <span>{title}</span>
        <span className="group-line" />
      </div>
      <span>{subtitle} · {items.length} 条</span>
    </div>
  );
}

function Items(props: ClipStudioListProps) {
  return (
    <div className="clip-items">
      {props.items.map((item) => (
        <ClipRow
          item={item}
          key={item.id}
          selected={props.selectedItem?.id === item.id}
          onCopyItem={props.onCopyItem}
          onDeleteItem={props.onDeleteItem}
          onOpenDetail={props.onOpenDetail}
          onSelectItem={props.onSelectItem}
        />
      ))}
    </div>
  );
}

function ClipRow({
  item,
  selected,
  onCopyItem,
  onDeleteItem,
  onOpenDetail,
  onSelectItem,
}: {
  item: ClipboardItem;
  selected: boolean;
  onCopyItem: (item: ClipboardItem) => void;
  onDeleteItem: (item: ClipboardItem) => void;
  onOpenDetail: (item: ClipboardItem) => void;
  onSelectItem: (id: number) => void;
}) {
  const kind = getClipKind(item);
  return (
    <button className={cn("clip-row", selected && "selected")} type="button" onClick={() => onSelectItem(item.id)}>
      <span className={cn("clip-row-icon", kind === "secret" && "secret")}>{getClipIcon(kind)}</span>
      <span className="clip-row-main">
        <span className="clip-row-title">
          {item.preview || "空白文本"}
        </span>
        <span className="clip-preview">{item.content}</span>
        <span className="clip-meta">
          <span className={cn("clip-tag", kind === "secret" && "warn")}>{getClipKindLabel(kind)}</span>
          {item.copyCount > 1 ? <span className="clip-tag">复制 {item.copyCount} 次</span> : null}
          <span>{formatTime(item.lastCopiedAt)}</span>
          <span>{item.content.length} 字符</span>
        </span>
      </span>
      <span className="clip-row-actions" onClick={(event) => event.stopPropagation()}>
        <IconAction label="详情" onClick={() => onOpenDetail(item)}><Eye className="size-3.5" /></IconAction>
        <IconAction label="复制" onClick={() => onCopyItem(item)}><Copy className="size-3.5" /></IconAction>
        <IconAction label="删除" onClick={() => onDeleteItem(item)}><Trash2 className="size-3.5" /></IconAction>
      </span>
    </button>
  );
}

function IconAction({ children, label, onClick }: { children: React.ReactNode; label: string; onClick: () => void }) {
  return (
    <button className="clip-small-button" title={label} type="button" onClick={onClick}>
      {children}
    </button>
  );
}

function EmptyState({ isBusy, searchTerm }: ClipStudioListProps) {
  const emptyText = searchTerm.trim()
    ? "没有匹配的剪贴板记录。"
    : "当前视图暂无记录。复制任意文本后等待自动捕获。";
  return <div className="clip-empty">{isBusy ? "正在加载剪贴板记录..." : emptyText}</div>;
}

function getFilterLabel(filter: ClipFilter) {
  return filters.find((item) => item.value === filter)?.label ?? "全部";
}
