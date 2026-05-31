// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";

vi.mock("@/components/clipboard/DesktopSettingsPanel", () => ({
  DesktopSettingsPanel: () => null,
}));
vi.mock("@/components/clipboard/ClipStudioCalendarPanel", () => ({
  ClipStudioCalendarPanel: () => null,
}));

import { ClipStudioPanel } from "@/components/clipboard/ClipStudioPanel";
import type { ClipStudioPanelProps } from "@/components/clipboard/ClipStudioPanel";

function makeProps(overrides: Partial<ClipStudioPanelProps> = {}): ClipStudioPanelProps {
  return {
    activeTab: "toolbox",
    dates: [],
    selectedDate: "2026-05-31",
    today: "2026-05-31",
    frequentCount: 0,
    selectedItem: null,
    toolboxText: "",
    toolboxResult: "",
    desktopSettings: null,
    isBusy: false,
    drawerOpen: true,
    onTabChange: vi.fn(),
    onCloseDrawer: vi.fn(),
    onDateSelect: vi.fn(),
    onToolboxTextChange: vi.fn(),
    onToolboxResultChange: vi.fn(),
    onSendSelectedToToolbox: vi.fn(),
    onCopyToolboxResult: vi.fn(),
    onSettingsChange: vi.fn(),
    onPurgeDeletedItems: vi.fn(),
    onHideWindow: vi.fn(),
    ...overrides,
  };
}

describe("ToolboxPanel highlight", () => {
  it("shows edit/preview switch only when toolbox text is code", () => {
    const { rerender, container } = render(
      <ClipStudioPanel {...makeProps({ toolboxText: "just a plain note" })} />,
    );
    expect(screen.queryByText("预览")).toBeNull();
    expect(container.querySelector("textarea.toolbox-input")).not.toBeNull();

    rerender(<ClipStudioPanel {...makeProps({ toolboxText: "const x = 1;\nfunction f() { return x; }" })} />);
    expect(screen.getByText("预览")).toBeTruthy();
    expect(screen.getByText("编辑")).toBeTruthy();
  });

  it("renders a highlighted code block in preview mode", () => {
    const { container } = render(
      <ClipStudioPanel {...makeProps({ toolboxText: "const x = 1;\nfunction f() { return x; }" })} />,
    );
    expect(container.querySelector("textarea.toolbox-input")).not.toBeNull();

    fireEvent.click(screen.getByText("预览"));
    expect(container.querySelector("code.hljs")).not.toBeNull();
    expect(container.querySelector("textarea.toolbox-input")).toBeNull();
  });
});
