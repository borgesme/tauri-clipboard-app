import { Check, Copy, Trash2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { ClipboardItem } from "@/types/clipboard";

interface DetailPanelProps {
  item: ClipboardItem | null;
  isBusy: boolean;
  message: string;
  onCopy: (item: ClipboardItem) => void;
  onDelete: (item: ClipboardItem) => void;
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

export function DetailPanel({ item, isBusy, message, onCopy, onDelete }: DetailPanelProps) {
  return (
    <Card className="min-h-0 gap-4 overflow-hidden border-border/70 bg-card/90 shadow-xl backdrop-blur">
      <CardHeader className="flex-row items-start justify-between gap-3 space-y-0">
        <div>
          <CardDescription>Detail</CardDescription>
          <CardTitle className="mt-1 text-2xl">内容详情</CardTitle>
        </div>
        {isBusy ? <Badge variant="outline">加载中</Badge> : null}
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden">
        {item ? (
          <>
            <pre className="min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words rounded-xl bg-slate-950 p-4 text-sm leading-6 text-slate-50">
              {item.content}
            </pre>
            <div className="grid grid-cols-3 gap-3 text-xs">
              <Metadata label="首次捕获" value={formatDateTime(item.createdAt)} />
              <Metadata label="最近复制" value={formatDateTime(item.lastCopiedAt)} />
              <Metadata label="Hash" value={item.contentHash.slice(0, 16)} />
            </div>
            <div className="flex flex-wrap gap-2">
              <Button onClick={() => onCopy(item)}>
                <Copy className="size-4" />
                复制回剪贴板
              </Button>
              <Button variant="destructive" onClick={() => onDelete(item)}>
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
  );
}

function Metadata({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-muted p-3">
      <div className="text-muted-foreground">{label}</div>
      <div className="mt-1 truncate font-medium">{value}</div>
    </div>
  );
}
