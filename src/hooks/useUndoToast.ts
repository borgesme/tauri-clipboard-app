import { useCallback, useEffect, useRef, useState } from "react";

export interface UndoState {
  ids: number[];
  date: string;
  count: number;
}

export function useUndoToast(durationMs = 6000) {
  const [pending, setPending] = useState<UndoState | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const show = useCallback(
    (next: UndoState) => {
      clearTimeout(timerRef.current);
      setPending(next);
      timerRef.current = setTimeout(() => setPending(null), durationMs);
    },
    [durationMs],
  );

  const clear = useCallback(() => {
    clearTimeout(timerRef.current);
    setPending(null);
  }, []);

  useEffect(() => () => clearTimeout(timerRef.current), []);

  return { pending, show, clear };
}
