import { DateSidebar } from "@/components/clipboard/DateSidebar";
import { DetailPanel } from "@/components/clipboard/DetailPanel";
import { ItemListPanel } from "@/components/clipboard/ItemListPanel";
import { useClipboardWorkspace } from "@/hooks/useClipboardWorkspace";
import { todayKey } from "@/lib/date";
import "./App.css";

function App() {
  const workspace = useClipboardWorkspace();

  return (
    <main className="grid h-screen grid-cols-[240px_minmax(300px,380px)_minmax(360px,1fr)] gap-4 p-4 text-foreground">
      <DateSidebar
        dates={workspace.dates}
        selectedDate={workspace.selectedDate}
        today={todayKey()}
        monitorEnabled={workspace.monitorEnabled}
        onDateSelect={workspace.selectDate}
        onMonitorToggle={() => void workspace.toggleMonitor()}
      />
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
    </main>
  );
}

export default App;
