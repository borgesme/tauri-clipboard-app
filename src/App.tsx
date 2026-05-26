import { useCallback, useEffect, useMemo, useState } from "react";
import { Check, ClipboardList, Clock3, Copy, Trash2 } from "lucide-react";

import {
  copyClipboardItem,
  deleteClipboardItem,
  listClipboardDates,
  listClipboardItems,
  onClipboardItemCreated,
} from "@/api/clipboard";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { ClipboardDateGroup, ClipboardItem } from "@/types/clipboard";
import "./App.css";

function todayKey(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function formatDateTime(value: string): string {
  return new Intl.DateTimeFormat(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

function App() {
  const [dates, setDates] = useState<ClipboardDateGroup[]>([]);
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [selectedDate, setSelectedDate] = useState(todayKey());
  const [selectedItemId, setSelectedItemId] = useState<number | null>(null);
  const [message, setMessage] = useState("复制一段文本后，它会自动出现在这里。");
  const [isBusy, setIsBusy] = useState(false);

  const selectedItem = useMemo(
    () => items.find((item) => item.id === selectedItemId) ?? items[0] ?? null,
    [items, selectedItemId],
  );

  const loadDates = useCallback(async () => {
    const nextDates = await listClipboardDates();
    setDates(nextDates);
  }, []);

  const loadItems = useCallback(async (date: string) => {
    const nextItems = await listClipboardItems(date);
    setItems(nextItems);
    setSelectedItemId((currentId) => {
      if (currentId && nextItems.some((item) => item.id === currentId)) {
        return currentId;
      }
      return nextItems[0]?.id ?? null;
    });
  }, []);

  useEffect(() => {
    setIsBusy(true);
    Promise.all([loadDates(), loadItems(selectedDate)])
      .catch((error: unknown) => setMessage(String(error)))
      .finally(() => setIsBusy(false));
  }, [loadDates, loadItems, selectedDate]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void onClipboardItemCreated(async (event) => {
      if (disposed) {
        return;
      }

      await loadDates();
      if (event.item.createdAt.startsWith(selectedDate)) {
        await loadItems(selectedDate);
      }
      setMessage("已捕获新的剪贴板文本。");
    })
      .then((dispose) => {
        unlisten = dispose;
      })
      .catch((error: unknown) => setMessage(String(error)));

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [loadDates, loadItems, selectedDate]);

  async function handleDateClick(date: string) {
    setSelectedDate(date);
    setMessage(`正在查看 ${date} 的剪贴板记录。`);
  }

  async function handleCopy(item: ClipboardItem) {
    await copyClipboardItem(item.id);
    setMessage("已复制回系统剪贴板。");
  }

  async function handleDelete(item: ClipboardItem) {
    await deleteClipboardItem(item.id);
    const nextItems = items.filter((candidate) => candidate.id !== item.id);
    setItems(nextItems);
    setSelectedItemId(nextItems[0]?.id ?? null);
    await loadDates();
    setMessage("已删除该条记录。");
  }

  return (
    <main className="grid h-screen grid-cols-[240px_minmax(300px,380px)_minmax(360px,1fr)] gap-4 p-4 text-foreground">
      <Card className="min-h-0 gap-4 overflow-hidden border-border/70 bg-card/90 shadow-xl backdrop-blur">
        <CardHeader>
          <CardDescription>Local Clipboard</CardDescription>
          <CardTitle className="flex items-center gap-2 text-2xl">
            <ClipboardList className="size-6 text-primary" />
            剪贴板工具箱
          </CardTitle>
        </CardHeader>
        <CardContent className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
          <Button
            className="h-auto justify-start rounded-xl px-3 py-3 text-left"
            variant={selectedDate === todayKey() ? "default" : "secondary"}
            onClick={() => void handleDateClick(todayKey())}
          >
            <span className="flex flex-col items-start gap-1">
              <span className="text-sm font-semibold">今天</span>
              <span className="text-xs opacity-80">{todayKey()}</span>
            </span>
          </Button>

          <div className="min-h-0 space-y-2 overflow-auto pr-1">
            {dates.map((group) => (
              <Button
                className="h-auto w-full justify-between rounded-xl px-3 py-3"
                key={group.date}
                variant={selectedDate === group.date ? "default" : "ghost"}
                onClick={() => void handleDateClick(group.date)}
              >
                <span>{group.date}</span>
                <Badge variant={selectedDate === group.date ? "secondary" : "outline"}>
                  {group.count} 条
                </Badge>
              </Button>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card className="min-h-0 gap-4 overflow-hidden border-border/70 bg-card/90 shadow-xl backdrop-blur">
        <CardHeader className="flex-row items-start justify-between gap-3 space-y-0">
          <div>
            <CardDescription>History</CardDescription>
            <CardTitle className="mt-1 text-2xl">{selectedDate}</CardTitle>
          </div>
          <Badge variant="secondary">{items.length} 条记录</Badge>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 overflow-hidden">
          {items.length === 0 ? (
            <div className="grid h-full place-items-center rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
              当前日期暂无记录。复制任意文本后等待自动捕获。
            </div>
          ) : (
            <div className="h-full space-y-3 overflow-auto pr-1">
              {items.map((item) => (
                <button
                  className={`w-full rounded-xl border p-4 text-left transition hover:bg-accent hover:text-accent-foreground ${
                    selectedItem?.id === item.id
                      ? "border-primary bg-accent text-accent-foreground shadow-sm"
                      : "border-border bg-background/60"
                  }`}
                  key={item.id}
                  type="button"
                  onClick={() => setSelectedItemId(item.id)}
                >
                  <span className="line-clamp-3 block whitespace-pre-wrap break-words text-sm leading-6">
                    {item.preview || "空白文本"}
                  </span>
                  <span className="mt-3 flex items-center justify-between gap-2 text-xs text-muted-foreground">
                    <span className="inline-flex items-center gap-1">
                      <Clock3 className="size-3.5" />
                      {formatTime(item.lastCopiedAt)}
                    </span>
                    <span>复制 {item.copyCount} 次</span>
                  </span>
                </button>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card className="min-h-0 gap-4 overflow-hidden border-border/70 bg-card/90 shadow-xl backdrop-blur">
        <CardHeader className="flex-row items-start justify-between gap-3 space-y-0">
          <div>
            <CardDescription>Detail</CardDescription>
            <CardTitle className="mt-1 text-2xl">内容详情</CardTitle>
          </div>
          {isBusy ? <Badge variant="outline">加载中</Badge> : null}
        </CardHeader>
        <CardContent className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden">
          {selectedItem ? (
            <>
              <pre className="min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words rounded-xl bg-slate-950 p-4 text-sm leading-6 text-slate-50">
                {selectedItem.content}
              </pre>
              <div className="grid grid-cols-3 gap-3 text-xs">
                <div className="rounded-lg bg-muted p-3">
                  <div className="text-muted-foreground">首次捕获</div>
                  <div className="mt-1 truncate font-medium">
                    {formatDateTime(selectedItem.createdAt)}
                  </div>
                </div>
                <div className="rounded-lg bg-muted p-3">
                  <div className="text-muted-foreground">最近复制</div>
                  <div className="mt-1 truncate font-medium">
                    {formatDateTime(selectedItem.lastCopiedAt)}
                  </div>
                </div>
                <div className="rounded-lg bg-muted p-3">
                  <div className="text-muted-foreground">Hash</div>
                  <div className="mt-1 truncate font-medium">
                    {selectedItem.contentHash.slice(0, 16)}
                  </div>
                </div>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button onClick={() => void handleCopy(selectedItem)}>
                  <Copy className="size-4" />
                  复制回剪贴板
                </Button>
                <Button variant="destructive" onClick={() => void handleDelete(selectedItem)}>
                  <Trash2 className="size-4" />
                  删除记录
                </Button>
              </div>
            </>
          ) : (
            <div className="grid flex-1 place-items-center rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
              选择一条记录查看完整内容。
            </div>
          )}

          <div className="flex min-h-6 items-center gap-2 text-sm text-muted-foreground">
            <Check className="size-4 text-primary" />
            {message}
          </div>
        </CardContent>
      </Card>
    </main>
  );
}

export default App;
