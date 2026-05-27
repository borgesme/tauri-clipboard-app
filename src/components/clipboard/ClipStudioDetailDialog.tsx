import { Copy, Pin, Send, Trash2, X } from "lucide-react";

import {
  formatDateTime,
  getClipKind,
  getClipKindLabel,
} from "@/components/clipboard/clipStudioHelpers";
import { Button } from "@/components/ui/button";
import type { ClipboardItem } from "@/types/clipboard";

interface ClipStudioDetailDialogProps {
  item: ClipboardItem | null;
  onClose: () => void;
  onCopy: (item: ClipboardItem) => void;
  onDelete: (item: ClipboardItem) => void;
  onSendToToolbox: (item: ClipboardItem) => void;
}

export function ClipStudioDetailDialog({ item, onClose, onCopy, onDelete, onSendToToolbox }: ClipStudioDetailDialogProps) {
  if (!item) {
    return null;
  }
  const kind = getClipKind(item);
  return (
    <div className="detail-backdrop" role="dialog" aria-modal="true" aria-label="剪贴板详情" onMouseDown={onClose}>
      <div className="detail-modal" onMouseDown={(event) => event.stopPropagation()}>
        <div className="detail-head">
          <div>
            <h2>剪贴板详情</h2>
            <div className="clip-meta mt-2">
              <span className="clip-tag">{getClipKindLabel(kind)}</span>
              <span>{formatDateTime(item.lastCopiedAt)}</span>
              <span>复制 {item.copyCount} 次</span>
            </div>
          </div>
          <button className="clip-icon-button" type="button" onClick={onClose}>
            <X className="size-4" />
          </button>
        </div>
        <div className="detail-body">
          <div className="detail-content">{item.content}</div>
          <div className="detail-grid">
            <DetailCell label="首次捕获" value={formatDateTime(item.createdAt)} />
            <DetailCell label="最近复制" value={formatDateTime(item.lastCopiedAt)} />
            <DetailCell label="字符数" value={`${item.content.length}`} />
            <DetailCell label="Hash" value={item.contentHash.slice(0, 16)} />
          </div>
          <div className="detail-actions">
            <Button className="clip-primary-button" onClick={() => onCopy(item)}>
              <Copy className="size-4" />
              复制
            </Button>
            <Button variant="outline" onClick={() => onSendToToolbox(item)}>
              <Send className="size-4" />
              送入工具箱
            </Button>
            <Button variant="outline" disabled title="固定功能待后端支持">
              <Pin className="size-4" />
              固定
            </Button>
            <Button variant="destructive" onClick={() => onDelete(item)}>
              <Trash2 className="size-4" />
              删除
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function DetailCell({ label, value }: { label: string; value: string }) {
  return (
    <div className="detail-cell">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
