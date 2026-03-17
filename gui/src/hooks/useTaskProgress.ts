import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ProgressEvent } from "@/types";
import { useAppStore } from "@/stores/appStore";
import { checkFileStatus, getTaskStatus } from "@/lib/tauri";
import { t } from "@/stores/i18nStore";

export function useTaskProgress() {
  const setProgress = useAppStore((state) => state.setProgress);
  const setIsProcessing = useAppStore((state) => state.setIsProcessing);
  const setCurrentTask = useAppStore((state) => state.setCurrentTask);
  const updateFileState = useAppStore((state) => state.updateFileState);
  const showToast = useAppStore((state) => state.showToast);

  useEffect(() => {
    let disposed = false;
    let syncQueue = Promise.resolve();

    const enqueue = (job: () => Promise<void>) => {
      syncQueue = syncQueue
        .then(async () => {
          if (disposed) return;
          await job();
        })
        .catch((e) => {
          console.error("Task sync pipeline error:", e);
        });
    };

    const syncTask = async (taskId: string, fileToRefresh?: string) => {
      try {
        const info = await getTaskStatus(taskId);
        setCurrentTask(info);

        if (fileToRefresh) {
          const state = await checkFileStatus(fileToRefresh);
          updateFileState(fileToRefresh, state);
        }
      } catch (e) {
        console.error("Failed to sync task status:", e);
      }
    };

    const finalizeTask = async (taskId: string) => {
      try {
        const info = await getTaskStatus(taskId);
        setCurrentTask(info);

        await Promise.all(
          info.config.files.map(async (path) => {
            const state = await checkFileStatus(path);
            updateFileState(path, state);
          })
        );

        const totalFiles = info.total_files || info.config.files.length;
        const successCount = info.results.filter((r) => r.success).length;
        const failureCount = info.results.filter((r) => !r.success).length;
        const modeLabel =
          info.config.mode === "Encrypt" ? t("config.encrypt") : t("config.decrypt");

        const summary = t("result.summaryLine", {
          mode: modeLabel,
          success: successCount,
          failed: failureCount,
          total: totalFiles,
        });

        const firstError = info.results.find((r) => !r.success)?.error;

        if (info.status === "Cancelled") {
          showToast({
            variant: "info",
            title: t("progress.cancelled"),
            description: summary,
            durationMs: 4500,
          });
          return;
        }

        if (info.status === "Failed") {
          showToast({
            variant: "error",
            title: t("progress.failed"),
            description: info.error ?? firstError ?? summary,
            durationMs: 8000,
          });
          return;
        }

        if (failureCount > 0) {
          const allFailed = successCount === 0 && totalFiles > 0;
          showToast(
            allFailed
              ? {
                  variant: "error",
                  title: t("progress.failed"),
                  description: firstError ?? summary,
                  durationMs: 8000,
                }
              : {
                  variant: "warning",
                  title: t("result.completedWithErrors"),
                  description: firstError ? `${summary} • ${firstError}` : summary,
                  durationMs: 8000,
                }
          );
        } else {
          showToast({
            variant: "success",
            title: t("result.completed"),
            description: summary,
            durationMs: 3500,
          });
        }
      } catch (e) {
        console.error("Failed to refresh file states:", e);
        showToast({
          variant: "error",
          title: t("progress.failed"),
          description: String(e),
          durationMs: 8000,
        });
      }
    };

    const unlisten = listen<ProgressEvent>("task-progress", (event) => {
      const progress = event.payload;
      const currentFile = progress.current_file;
      const activeTaskId = useAppStore.getState().currentTask?.id;
      if (activeTaskId && progress.task_id !== activeTaskId) return;

      setProgress(progress);

      if (progress.phase === "Checking") {
        enqueue(() => syncTask(progress.task_id));
      }

      // File-level completion/failure (handler emits current_file).
      if (
        (progress.phase === "Completed" || progress.phase === "Failed") &&
        currentFile
      ) {
        enqueue(() => syncTask(progress.task_id, currentFile));
      }

      // Task-level end state (TaskManager emits current_file: null).
      if (
        (progress.phase === "Completed" && !currentFile) ||
        progress.phase === "Cancelled" ||
        (progress.phase === "Failed" && !currentFile)
      ) {
        setIsProcessing(false);
        enqueue(() => finalizeTask(progress.task_id));
      }
    });

    return () => {
      disposed = true;
      unlisten.then((fn) => fn());
    };
  }, [
    setCurrentTask,
    setIsProcessing,
    setProgress,
    showToast,
    updateFileState,
  ]);
}
