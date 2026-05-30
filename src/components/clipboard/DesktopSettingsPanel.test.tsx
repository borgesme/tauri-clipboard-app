// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";

import { DesktopSettingsPanel } from "@/components/clipboard/DesktopSettingsPanel";
import type { DesktopSettings } from "@/types/clipboard";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@/api/clipboard", () => ({
  validateStorageDir: vi.fn().mockResolvedValue(undefined),
}));

const SETTINGS: DesktopSettings = {
  autostartEnabled: false,
  monitorEnabled: true,
  retentionDays: 30,
  maxRecordCount: 1000,
  maxTextLength: 5000,
  ignorePasswordLikeText: true,
  customSecretPatterns: "",
  storageDir: "",
};

function renderPanel() {
  const onSettingsChange = vi.fn();
  render(
    <DesktopSettingsPanel
      settings={SETTINGS}
      isBusy={false}
      onSettingsChange={onSettingsChange}
      onPurgeDeletedItems={vi.fn()}
      onHideWindow={vi.fn()}
    />,
  );
  return { onSettingsChange };
}

describe("DesktopSettingsPanel 混合保存", () => {
  it("修改数字字段不立即提交（草稿）", () => {
    const { onSettingsChange } = renderPanel();
    fireEvent.change(screen.getByRole("spinbutton", { name: "天" }), {
      target: { value: "7" },
    });
    expect(onSettingsChange).not.toHaveBeenCalled();
  });

  it("点击保存设置后提交合并草稿值", () => {
    const { onSettingsChange } = renderPanel();
    fireEvent.change(screen.getByRole("spinbutton", { name: "天" }), {
      target: { value: "7" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    expect(onSettingsChange).toHaveBeenCalledTimes(1);
    expect(onSettingsChange).toHaveBeenCalledWith(
      expect.objectContaining({ retentionDays: 7 }),
    );
  });

  it("修改正则不立即提交，保存后提交", () => {
    const { onSettingsChange } = renderPanel();
    fireEvent.change(screen.getByPlaceholderText(/corp_/), {
      target: { value: "^secret_" },
    });
    expect(onSettingsChange).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    expect(onSettingsChange).toHaveBeenCalledWith(
      expect.objectContaining({ customSecretPatterns: "^secret_" }),
    );
  });

  it("拨布尔开关立即提交", () => {
    const { onSettingsChange } = renderPanel();
    fireEvent.click(screen.getByRole("switch", { name: "剪贴板监听" }));
    expect(onSettingsChange).toHaveBeenCalledWith(
      expect.objectContaining({ monitorEnabled: false }),
    );
  });
});
