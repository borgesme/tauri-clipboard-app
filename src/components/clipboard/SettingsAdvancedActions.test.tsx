// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { MaintenanceAction } from "@/components/clipboard/SettingsAdvancedActions";

const confirmMock = vi.fn();

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: (...args: unknown[]) => confirmMock(...args),
}));

beforeEach(() => {
  confirmMock.mockReset();
});

describe("MaintenanceAction purge confirm", () => {
  it("calls onPurgeDeletedItems when confirm resolves true", async () => {
    confirmMock.mockResolvedValue(true);
    const onPurge = vi.fn();
    render(<MaintenanceAction isBusy={false} onPurgeDeletedItems={onPurge} />);

    fireEvent.click(screen.getByText("清理已删除记录"));

    await waitFor(() => expect(onPurge).toHaveBeenCalledTimes(1));
  });

  it("does not call onPurgeDeletedItems when confirm resolves false", async () => {
    confirmMock.mockResolvedValue(false);
    const onPurge = vi.fn();
    render(<MaintenanceAction isBusy={false} onPurgeDeletedItems={onPurge} />);

    fireEvent.click(screen.getByText("清理已删除记录"));

    await waitFor(() => expect(confirmMock).toHaveBeenCalledTimes(1));
    expect(onPurge).not.toHaveBeenCalled();
  });
});
