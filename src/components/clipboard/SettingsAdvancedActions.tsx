import { confirm } from "@tauri-apps/plugin-dialog";

import { Button } from "@/components/ui/button";

export function MaintenanceAction({
  isBusy,
  onPurgeDeletedItems,
}: {
  isBusy: boolean;
  onPurgeDeletedItems: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-xl border bg-background/60 p-3 md:col-span-2">
      <div>
        <div className="text-sm font-medium">数据维护</div>
        <div className="text-xs text-muted-foreground">物理删除已移入回收状态的记录并压缩数据库</div>
      </div>
      <Button
        disabled={isBusy}
        size="sm"
        variant="outline"
        onClick={() => void confirmPurge(onPurgeDeletedItems)}
      >
        清理已删除记录
      </Button>
    </div>
  );
}

export async function confirmPurge(onPurgeDeletedItems: () => void) {
  const ok = await confirm(
    "将物理删除所有已移入回收状态的记录，此操作不可恢复。是否继续？",
    { title: "清理已删除记录", kind: "warning" },
  );
  if (ok) {
    onPurgeDeletedItems();
  }
}

export function CustomSecretPatternsSetting({
  isBusy,
  patterns,
  onChange,
}: {
  isBusy: boolean;
  patterns: string;
  onChange: (patterns: string) => void;
}) {
  return (
    <label className="space-y-2 rounded-xl border bg-background/60 p-3 md:col-span-2">
      <div>
        <div className="text-sm font-medium">自定义敏感正则</div>
        <div className="text-xs text-muted-foreground">每行一条正则；匹配内容会按敏感内容跳过</div>
      </div>
      <textarea
        className="min-h-20 w-full resize-y rounded-md border bg-background px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring/40"
        disabled={isBusy}
        placeholder="例如 ^corp_[A-Za-z0-9]{24}$"
        value={patterns}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </label>
  );
}
