import { useEffect, useMemo, useRef, useState } from "react";

import { ClipStudioDetailDialog } from "@/components/clipboard/ClipStudioDetailDialog";
import { ClipStudioLayout } from "@/components/clipboard/ClipStudioLayout";
import { ClipStudioList } from "@/components/clipboard/ClipStudioList";
import { ClipStudioPanel } from "@/components/clipboard/ClipStudioPanel";
import { useKeyboardShortcuts } from "@/components/clipboard/useClipStudioKeyboard";
import {
  type ClipFilter,
  type PanelTab,
  countRecords,
  filterClipboardItems,
} from "@/components/clipboard/clipStudioHelpers";
import { useClipboardWorkspace } from "@/hooks/useClipboardWorkspace";
import { todayKey } from "@/lib/date";
import type { ClipboardItem } from "@/types/clipboard";

interface ClipStudioPageProps {
  workspace: ReturnType<typeof useClipboardWorkspace>;
  panelRequest: {
    revision: number;
    tab: PanelTab;
  };
}

export function ClipStudioPage({ workspace, panelRequest }: ClipStudioPageProps) {
  const state = useClipStudioState(workspace, panelRequest.tab);
  usePanelRequest(panelRequest, state.setActivePanel);
  useVisibleSelectionSync(state.selectedItem, workspace);
  usePageKeyboard(workspace, state);

  return (
    <ClipStudioLayout {...createLayoutProps(workspace, state)}>
      <ClipStudioList {...createListProps(workspace, state)} />
      <ClipStudioPanel {...createPanelProps(workspace, state)} />
      <ClipStudioDetailDialog {...createDialogProps(workspace, state)} />
    </ClipStudioLayout>
  );
}

interface ClipStudioState {
  activeFilter: ClipFilter;
  activePanel: PanelTab;
  detailItem: ClipboardItem | null;
  drawerOpen: boolean;
  frequentCount: number;
  searchInputRef: React.MutableRefObject<HTMLInputElement | null>;
  selectedItem: ClipboardItem | null;
  setActiveFilter: (filter: ClipFilter) => void;
  setActivePanel: (panel: PanelTab) => void;
  setDetailItem: (item: ClipboardItem | null) => void;
  setDrawerOpen: (open: boolean) => void;
  setToolboxResult: (value: string) => void;
  setToolboxText: (value: string) => void;
  today: string;
  toolboxResult: string;
  toolboxText: string;
  totalCount: number;
  visibleItems: ClipboardItem[];
}

function useClipStudioState(workspace: ReturnType<typeof useClipboardWorkspace>, initialPanel: PanelTab) {
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const [activeFilter, setActiveFilter] = useState<ClipFilter>("all");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [activePanel, setActivePanel] = useState<PanelTab>(initialPanel);
  const [detailItem, setDetailItem] = useState<ClipboardItem | null>(null);
  const [toolboxText, setToolboxText] = useState("选择一条剪贴板记录后，按 T 或点击“送入工具箱”。");
  const [toolboxResult, setToolboxResult] = useState("");
  const visibleItems = useMemo(() => filterClipboardItems(workspace.items, activeFilter), [workspace.items, activeFilter]);
  const selectedItem = useVisibleSelectedItem(visibleItems, workspace.selectedItem);
  const frequentCount = workspace.items.filter((item) => item.copyCount > 1).length;

  return {
    activeFilter,
    activePanel,
    detailItem,
    drawerOpen,
    frequentCount,
    searchInputRef,
    selectedItem,
    setActiveFilter,
    setActivePanel,
    setDetailItem,
    setDrawerOpen,
    setToolboxResult,
    setToolboxText,
    today: todayKey(),
    toolboxResult,
    toolboxText,
    totalCount: countRecords(workspace.dates),
    visibleItems,
  };
}

function useVisibleSelectedItem(items: ClipboardItem[], selectedItem: ClipboardItem | null) {
  return useMemo(() => {
    if (selectedItem && items.some((item) => item.id === selectedItem.id)) {
      return selectedItem;
    }
    return items[0] ?? null;
  }, [items, selectedItem]);
}

function usePanelRequest(panelRequest: ClipStudioPageProps["panelRequest"], setActivePanel: (panel: PanelTab) => void) {
  useEffect(() => setActivePanel(panelRequest.tab), [panelRequest, setActivePanel]);
}

function useVisibleSelectionSync(selectedItem: ClipboardItem | null, workspace: ReturnType<typeof useClipboardWorkspace>) {
  useEffect(() => {
    if (selectedItem && selectedItem.id !== workspace.selectedItem?.id) {
      workspace.setSelectedItemId(selectedItem.id);
    }
  }, [selectedItem, workspace]);
}

function usePageKeyboard(workspace: ReturnType<typeof useClipboardWorkspace>, state: ClipStudioState) {
  useKeyboardShortcuts({
    detailItem: state.detailItem,
    searchInputRef: state.searchInputRef,
    selectedItem: state.selectedItem,
    visibleItems: state.visibleItems,
    onCopy: (item) => void workspace.copyItem(item),
    onOpenDetail: state.setDetailItem,
    onReset: () => resetView(workspace, state),
    onSelectItem: workspace.setSelectedItemId,
    onSendToToolbox: (item) => sendToToolbox(item, state),
  });
}

function createLayoutProps(workspace: ReturnType<typeof useClipboardWorkspace>, state: ClipStudioState) {
  return {
    totalCount: state.totalCount,
    frequentCount: state.frequentCount,
    monitorEnabled: workspace.monitorEnabled,
    message: workspace.message,
    onOpenCalendar: () => openPanel("calendar", state),
    onOpenToolbox: () => openPanel("toolbox", state),
    onOpenSettings: () => openPanel("settings", state),
    onShowFrequent: () => state.setActiveFilter("frequent"),
    onToggleMonitor: () => void workspace.toggleMonitor(),
  };
}

function createListProps(workspace: ReturnType<typeof useClipboardWorkspace>, state: ClipStudioState) {
  return {
    items: state.visibleItems,
    selectedItem: state.selectedItem,
    selectedDate: workspace.searchTerm.trim() ? "搜索结果" : workspace.selectedDate,
    searchInputRef: state.searchInputRef,
    searchTerm: workspace.searchTerm,
    activeFilter: state.activeFilter,
    errorMessage: workspace.errorMessage,
    isBusy: workspace.isBusy,
    onSearchChange: workspace.setSearchTerm,
    onClearSearch: () => workspace.setSearchTerm(""),
    onClearDate: () => void workspace.clearDate(),
    onFilterChange: state.setActiveFilter,
    onSelectItem: workspace.setSelectedItemId,
    onCopyItem: (item: ClipboardItem) => void workspace.copyItem(item),
    onDeleteItem: (item: ClipboardItem) => void workspace.deleteItem(item),
    onOpenDetail: state.setDetailItem,
  };
}

function createPanelProps(workspace: ReturnType<typeof useClipboardWorkspace>, state: ClipStudioState) {
  return {
    activeTab: state.activePanel,
    dates: workspace.dates,
    selectedDate: workspace.selectedDate,
    today: state.today,
    frequentCount: state.frequentCount,
    selectedItem: state.selectedItem,
    toolboxText: state.toolboxText,
    toolboxResult: state.toolboxResult,
    desktopSettings: workspace.desktopSettings,
    isBusy: workspace.isBusy,
    drawerOpen: state.drawerOpen,
    onTabChange: (tab: PanelTab) => openPanel(tab, state),
    onCloseDrawer: () => state.setDrawerOpen(false),
    onDateSelect: workspace.selectDate,
    onToolboxTextChange: state.setToolboxText,
    onToolboxResultChange: state.setToolboxResult,
    onSendSelectedToToolbox: () => state.selectedItem && sendToToolbox(state.selectedItem, state),
    onCopyToolboxResult: () => void navigator.clipboard.writeText(state.toolboxResult),
    onSettingsChange: (settings: NonNullable<typeof workspace.desktopSettings>) => void workspace.updateSettings(settings),
    onPurgeDeletedItems: () => void workspace.purgeDeletedItems(),
    onHideWindow: () => void workspace.hideWindow(),
  };
}

function createDialogProps(workspace: ReturnType<typeof useClipboardWorkspace>, state: ClipStudioState) {
  return {
    item: state.detailItem,
    onClose: () => state.setDetailItem(null),
    onCopy: (item: ClipboardItem) => void workspace.copyItem(item),
    onDelete: (item: ClipboardItem) => void workspace.deleteItem(item),
    onSendToToolbox: (item: ClipboardItem) => sendToToolbox(item, state),
  };
}

function sendToToolbox(item: ClipboardItem, state: ClipStudioState) {
  state.setToolboxText(item.content);
  state.setToolboxResult("");
  openPanel("toolbox", state);
}

function openPanel(tab: PanelTab, state: ClipStudioState) {
  state.setActivePanel(tab);
  state.setDrawerOpen(true);
}

function resetView(workspace: ReturnType<typeof useClipboardWorkspace>, state: ClipStudioState) {
  state.setDetailItem(null);
  state.setDrawerOpen(false);
  state.setActiveFilter("all");
  workspace.setSearchTerm("");
}
