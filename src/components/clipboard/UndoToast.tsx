import type { UndoState } from "@/hooks/useUndoToast";

export function UndoToast({
  pending,
  onUndo,
  onDismiss,
}: {
  pending: UndoState | null;
  onUndo: () => void;
  onDismiss: () => void;
}) {
  if (!pending) {
    return null;
  }
  return (
    <div className="undo-toast" role="status">
      <span>已清空 {pending.count} 条记录</span>
      <button type="button" onClick={onUndo}>
        撤销
      </button>
      <button type="button" aria-label="关闭" onClick={onDismiss}>
        ×
      </button>
    </div>
  );
}
