import { useEffect } from "react";

import {
  onClipboardItemCreated,
  onClipboardItemDeleted,
  onClipboardItemUpdated,
  onClipboardItemSkipped,
  onClipboardMonitorError,
  onClipboardMonitorStatusChanged,
} from "@/api/clipboard";
import type {
  ClipboardMonitorErrorEvent,
  ClipboardSkippedEvent,
  DesktopSettings,
} from "@/types/clipboard";

interface ClipboardEventsOptions {
  refreshView: () => Promise<void>;
  setMessage: (value: string) => void;
  setErrorMessage: (value: string) => void;
  setMonitorEnabled: (value: boolean) => void;
  setDesktopSettings: React.Dispatch<React.SetStateAction<DesktopSettings | null>>;
}

export function useClipboardEvents({
  refreshView,
  setMessage,
  setErrorMessage,
  setMonitorEnabled,
  setDesktopSettings,
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
    const monitorError = (event: ClipboardMonitorErrorEvent) => {
      if (disposed) {
        return;
      }
      if (event.failing) {
        setErrorMessage(event.message ?? "剪贴板监听出现错误。");
      } else {
        setErrorMessage("");
        setMessage("剪贴板监听已恢复。");
      }
    };
    void registerEvents(disposers, remoteChange, monitorError, setMonitorEnabled, setDesktopSettings)
      .catch((error: unknown) => setErrorMessage(String(error)));
    return () => {
      disposed = true;
      disposers.forEach((dispose) => dispose());
    };
  }, [refreshView, setMessage, setErrorMessage, setMonitorEnabled, setDesktopSettings]);
}

async function registerEvents(
  disposers: Array<() => void>,
  remoteChange: (message: string) => void,
  monitorError: (event: ClipboardMonitorErrorEvent) => void,
  setMonitorEnabled: (value: boolean) => void,
  setDesktopSettings: React.Dispatch<React.SetStateAction<DesktopSettings | null>>,
) {
  disposers.push(await onClipboardItemCreated(() => remoteChange("已捕获新的剪贴板文本。")));
  disposers.push(await onClipboardItemUpdated(() => remoteChange("重复内容已更新计数。")));
  disposers.push(await onClipboardItemDeleted(() => remoteChange("记录已删除。")));
  disposers.push(await onClipboardItemSkipped((event) => remoteChange(skipMessage(event))));
  disposers.push(await onClipboardMonitorError(monitorError));
  disposers.push(await onClipboardMonitorStatusChanged((status) => {
    setMonitorEnabled(status.enabled);
    setDesktopSettings((settings) => settings ? { ...settings, monitorEnabled: status.enabled } : settings);
  }));
}

export function skipMessage(event: ClipboardSkippedEvent) {
  if (event.reason === "tooLong") {
    return `该剪贴板内容超过单条文本上限（${event.maxTextLength} 字），已跳过。`;
  }
  if (event.reason === "secretLike") {
    return "疑似敏感内容已按设置跳过。";
  }
  return "该剪贴板内容已跳过。";
}
