import { useEffect, useState } from "react";

import { onOpenSettings } from "@/api/clipboard";
import { ClipStudioPage } from "@/components/clipboard/ClipStudioPage";
import type { PanelTab } from "@/components/clipboard/clipStudioHelpers";
import { useClipboardWorkspace } from "@/hooks/useClipboardWorkspace";
import "./App.css";

interface PanelRequest {
  revision: number;
  tab: PanelTab;
}

function App() {
  const workspace = useClipboardWorkspace();
  const [panelRequest, setPanelRequest] = useState<PanelRequest>({ revision: 0, tab: "calendar" });

  useEffect(() => {
    let dispose: (() => void) | undefined;
    void onOpenSettings(() => {
      setPanelRequest((current) => ({ revision: current.revision + 1, tab: "settings" }));
    }).then((unlisten) => {
      dispose = unlisten;
    });
    return () => dispose?.();
  }, []);

  return (
    <main className="clip-root">
      <ClipStudioPage workspace={workspace} panelRequest={panelRequest} />
    </main>
  );
}

export default App;
