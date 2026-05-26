import { Clock3, Search, X } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { ClipboardItem } from "@/types/clipboard";

interface ItemListPanelProps {
  items: ClipboardItem[];
  selectedDate: string;
  selectedItemId: number | null;
  searchTerm: string;
  isBusy: boolean;
  errorMessage: string;
  onSearchChange: (value: string) => void;
  onClearSearch: () => void;
  onItemSelect: (id: number) => void;
  onClearDate: () => void;
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

export function ItemListPanel(props: ItemListPanelProps) {
  const isSearchMode = props.searchTerm.trim().length > 0;
  const title = isSearchMode ? "搜索结果" : props.selectedDate;

  return (
    <Card className="min-h-0 gap-4 overflow-hidden border-border/70 bg-card/90 shadow-xl backdrop-blur">
      <ItemListHeader {...props} title={title} isSearchMode={isSearchMode} />
      <CardContent className="min-h-0 flex-1 overflow-hidden">
        {props.items.length === 0 ? <EmptyState {...props} isSearchMode={isSearchMode} /> : <ItemList {...props} />}
      </CardContent>
    </Card>
  );
}

function ItemListHeader(props: ItemListPanelProps & { title: string; isSearchMode: boolean }) {
  return (
    <CardHeader className="gap-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <CardDescription>{props.isSearchMode ? "Search" : "History"}</CardDescription>
          <CardTitle className="mt-1 text-2xl">{props.title}</CardTitle>
        </div>
        <Badge variant="secondary">{props.items.length} 条记录</Badge>
      </div>
      <SearchToolbar {...props} />
      {props.errorMessage ? <p className="text-sm text-destructive">{props.errorMessage}</p> : null}
    </CardHeader>
  );
}

function SearchToolbar(props: ItemListPanelProps & { isSearchMode: boolean }) {
  return (
    <div className="flex gap-2">
      <label className="flex min-w-0 flex-1 items-center gap-2 rounded-md border bg-background px-3 py-2 text-sm shadow-xs focus-within:ring-2 focus-within:ring-ring/40">
        <Search className="size-4 text-muted-foreground" />
        <input
          className="min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
          placeholder="跨日期搜索内容..."
          value={props.searchTerm}
          onChange={(event) => props.onSearchChange(event.currentTarget.value)}
        />
      </label>
      {props.isSearchMode ? <ClearSearchButton onClick={props.onClearSearch} /> : <ClearDateButton {...props} />}
    </div>
  );
}

function ClearSearchButton({ onClick }: { onClick: () => void }) {
  return (
    <Button variant="outline" size="icon" onClick={onClick}>
      <X className="size-4" />
    </Button>
  );
}

function ClearDateButton(props: ItemListPanelProps) {
  return (
    <Button variant="outline" onClick={props.onClearDate} disabled={props.items.length === 0 || props.isBusy}>
      清空当天
    </Button>
  );
}

function EmptyState(props: ItemListPanelProps & { isSearchMode: boolean }) {
  const emptyText = props.isSearchMode ? "没有匹配的剪贴板记录。" : "当前日期暂无记录。复制任意文本后等待自动捕获。";
  return (
    <div className="grid h-full place-items-center rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
      {props.isBusy ? "正在加载剪贴板记录..." : emptyText}
    </div>
  );
}

function ItemList(props: ItemListPanelProps) {
  return (
    <div className="h-full space-y-3 overflow-auto pr-1">
      {props.items.map((item) => (
        <ClipboardItemCard item={item} selected={props.selectedItemId === item.id} onSelect={props.onItemSelect} key={item.id} />
      ))}
    </div>
  );
}

function ClipboardItemCard({
  item,
  selected,
  onSelect,
}: {
  item: ClipboardItem;
  selected: boolean;
  onSelect: (id: number) => void;
}) {
  return (
    <button className={itemCardClass(selected)} type="button" onClick={() => onSelect(item.id)}>
      <span className="line-clamp-3 block whitespace-pre-wrap break-words text-sm leading-6">
        {item.preview || "空白文本"}
      </span>
      <span className="mt-3 flex items-center justify-between gap-2 text-xs text-muted-foreground">
        <span className="inline-flex items-center gap-1">
          <Clock3 className="size-3.5" />
          {formatTime(item.lastCopiedAt)}
        </span>
        <Badge variant={item.copyCount > 1 ? "default" : "outline"}>复制 {item.copyCount} 次</Badge>
      </span>
    </button>
  );
}

function itemCardClass(selected: boolean) {
  const base = "w-full rounded-xl border p-4 text-left transition hover:bg-accent hover:text-accent-foreground";
  const active = "border-primary bg-accent text-accent-foreground shadow-sm";
  const inactive = "border-border bg-background/60";
  return `${base} ${selected ? active : inactive}`;
}
