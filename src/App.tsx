import { useEffect, useState } from "react";

import { onOpenSettings } from "@/api/clipboard";
import { DateSidebar } from "@/components/clipboard/DateSidebar";
import { DesktopSettingsPanel } from "@/components/clipboard/DesktopSettingsPanel";
import { DetailPanel } from "@/components/clipboard/DetailPanel";
import { ItemListPanel } from "@/components/clipboard/ItemListPanel";
import { Button } from "@/components/ui/button";
import { useClipboardWorkspace } from "@/hooks/useClipboardWorkspace";
import { todayKey } from "@/lib/date";
import "./App.css";

function App() {
  const workspace = useClipboardWorkspace();
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    let dispose: (() => void) | undefined;
    void onOpenSettings(() => setSettingsOpen(true)).then((unlisten) => {
      dispose = unlisten;
    });
    return () => dispose?.();
  }, []);

  return (
    <main className="relative h-screen p-4 text-foreground">
      <HistoryView workspace={workspace} />
      {settingsOpen ? (
        <SettingsOverlay workspace={workspace} onClose={() => setSettingsOpen(false)} />
      ) : null}
    </main>
  );
}

function SettingsOverlay({
  workspace,
  onClose,
}: {
  workspace: ReturnType<typeof useClipboardWorkspace>;
  onClose: () => void;
}) {
  return (
    <div className="absolute inset-0 z-30 bg-background/30 p-4 backdrop-blur-sm">
      <div className="ml-auto w-[min(760px,calc(100vw-2rem))]">
        <div className="mb-2 flex justify-end">
          <Button size="sm" variant="secondary" onClick={onClose}>
            关闭设置
          </Button>
        </div>
        <DesktopSettingsPanel
          settings={workspace.desktopSettings}
          monitorEnabled={workspace.monitorEnabled}
          isBusy={workspace.isBusy}
          onMonitorToggle={() => void workspace.toggleMonitor()}
          onSettingsChange={(settings) => void workspace.updateSettings(settings)}
          onHideWindow={() => void workspace.hideWindow()}
        />
      </div>
    </div>
  );
}

function HistoryView({ workspace }: { workspace: ReturnType<typeof useClipboardWorkspace> }) {
  return (
    <section className="grid h-full min-h-0 grid-cols-[260px_minmax(300px,380px)_minmax(360px,1fr)] gap-4">
      <aside className="min-h-0">
        <DateSidebar
          dates={workspace.dates}
          selectedDate={workspace.selectedDate}
          today={todayKey()}
          onDateSelect={workspace.selectDate}
        />
      </aside>
      <ItemListPanel
        items={workspace.items}
        selectedDate={workspace.selectedDate}
        selectedItemId={workspace.selectedItem?.id ?? null}
        searchTerm={workspace.searchTerm}
        isBusy={workspace.isBusy}
        errorMessage={workspace.errorMessage}
        onSearchChange={workspace.setSearchTerm}
        onClearSearch={() => workspace.setSearchTerm("")}
        onItemSelect={workspace.setSelectedItemId}
        onClearDate={() => void workspace.clearDate()}
      />
      <DetailPanel
        item={workspace.selectedItem}
        isBusy={workspace.isBusy}
        message={workspace.message}
        onCopy={(item) => void workspace.copyItem(item)}
        onDelete={(item) => void workspace.deleteItem(item)}
      />
    </section>
  );
}

export default App;
