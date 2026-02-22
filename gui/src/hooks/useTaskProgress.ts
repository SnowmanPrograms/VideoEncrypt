import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ProgressEvent } from "@/types";
import { useAppStore } from "@/stores/appStore";

export function useTaskProgress() {
  const setProgress = useAppStore((state) => state.setProgress);
  const setIsProcessing = useAppStore((state) => state.setIsProcessing);

  useEffect(() => {
    const unlisten = listen<ProgressEvent>("task-progress", (event) => {
      const progress = event.payload;
      setProgress(progress);

      if (
        progress.phase === "Completed" ||
        progress.phase === "Failed" ||
        progress.phase === "Cancelled"
      ) {
        setIsProcessing(false);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setProgress, setIsProcessing]);
}
