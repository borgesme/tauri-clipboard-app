// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";

import { UndoToast } from "@/components/clipboard/UndoToast";

describe("UndoToast", () => {
  it("renders nothing when pending is null", () => {
    const { container } = render(
      <UndoToast pending={null} onUndo={() => {}} onDismiss={() => {}} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders the count and fires onUndo / onDismiss", () => {
    const onUndo = vi.fn();
    const onDismiss = vi.fn();
    render(
      <UndoToast
        pending={{ ids: [1, 2, 3], date: "2026-05-29", count: 3 }}
        onUndo={onUndo}
        onDismiss={onDismiss}
      />,
    );

    expect(screen.getByText(/已清空 3 条记录/)).toBeTruthy();

    fireEvent.click(screen.getByText("撤销"));
    expect(onUndo).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByLabelText("关闭"));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
