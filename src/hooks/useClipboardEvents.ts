import { useEffect } from "react";

import {
  onClipboardItemCreated,
  onClipboardItemDeleted,
  onClipboardItemUpdated,
  onClipboardMonitorStatusChanged,
} from "@/api/clipboard";

interface ClipboardEventsOptions {
  refreshView: () => Promise<void>;
  setMessage: (value: string) => void;
  setErrorMessage: (value: string) => void;
  setMonitorEnabled: (value: boolean) => void;
}

export function useClipboardEvents({
  refreshView,
  setMessage,
  setErrorMessage,
  setMonitorEnabled,
}: ClipboardEventsOptions) {
  useEffect(() => {
    const disposers: Array<() => void> = [];
    let disposed = false;
    const remoteChange = (message: string) => {
      if (disposed) {
        return;
      }
      setMessage(message);
      void refreshView();
    };
    void registerEvents(disposers, remoteChange, setMonitorEnabled)
      .catch((error: unknown) => setErrorMessage(String(error)));
    return () => {
      disposed = true;
      disposers.forEach((dispose) => dispose());
    };
  }, [refreshView, setMessage, setErrorMessage, setMonitorEnabled]);
}

async function registerEvents(
  disposers: Array<() => void>,
  remoteChange: (message: string) => void,
  setMonitorEnabled: (value: boolean) => void,
) {
  disposers.push(await onClipboardItemCreated(() => remoteChange("已捕获新的剪贴板文本。")));
  disposers.push(await onClipboardItemUpdated(() => remoteChange("重复内容已更新计数。")));
  disposers.push(await onClipboardItemDeleted(() => remoteChange("记录已删除。")));
  disposers.push(await onClipboardMonitorStatusChanged((status) => setMonitorEnabled(status.enabled)));
}
