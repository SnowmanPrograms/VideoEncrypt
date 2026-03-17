import { useCallback, useRef, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { addDroppedFiles } from "@/lib/tauri";
import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/stores/i18nStore";
import type { FileInfo } from "@/types";

interface UseDragDropOptions {
  disabled?: boolean;
  onFilesAdded?: (files: FileInfo[]) => void;
}

interface UseDragDropReturn {
  isDragOver: boolean;
  dragProps: {
    onDragEnter: (e: React.DragEvent) => void;
    onDragOver: (e: React.DragEvent) => void;
    onDragLeave: (e: React.DragEvent) => void;
    onDrop: (e: React.DragEvent) => void;
  };
}

/**
 * Hook that integrates Tauri 2 native file drop events with React drag state.
 *
 * Tauri 2's webview intercepts native OS file drops at the webview layer,
 * so the HTML5 Drag and Drop API cannot receive file paths via dataTransfer.
 * Instead, we use Tauri's `onDragDropEvent()` which provides actual file paths
 * from the OS. HTML5 drag events are still used for immediate visual feedback.
 */
export function useDragDrop(
  options: UseDragDropOptions = {}
): UseDragDropReturn {
  const { disabled = false, onFilesAdded } = options;
  const isDragOver = useAppStore((s) => s.isDragOver);
  const setIsDragOver = useAppStore((s) => s.setIsDragOver);
  const dragCounter = useRef(0);
  const isProcessing = useAppStore((s) => s.isProcessing);
  const addFiles = useAppStore((s) => s.addFiles);
  const showToast = useAppStore((s) => s.showToast);
  const i18n = useI18n((s) => s.t);

  const effectiveDisabled = disabled || isProcessing;

  // Listen to Tauri 2 native drag-and-drop events via the dedicated API.
  // This is the only reliable way to get actual file paths in Tauri 2's webview.
  useEffect(() => {
    if (effectiveDisabled) {
      dragCounter.current = 0;
      setIsDragOver(false);
      return;
    }

    let unlisten: (() => void) | undefined;

    const setup = async () => {
      const window = getCurrentWindow();
      unlisten = await window.onDragDropEvent(async (event) => {
        switch (event.payload.type) {
          case "enter": {
            dragCounter.current++;
            if (dragCounter.current === 1) {
              setIsDragOver(true);
            }
            break;
          }
          case "over": {
            // Position updates while dragging over - no action needed
            break;
          }
          case "drop": {
            dragCounter.current = 0;
            setIsDragOver(false);

            const paths = event.payload.paths;
            if (!paths || paths.length === 0) {
              showToast({
                variant: "warning",
                title: i18n.error.noFilesSelected,
              });
              return;
            }

            try {
              const fileInfos = await addDroppedFiles(paths, true);
              if (fileInfos.length > 0) {
                if (onFilesAdded) {
                  onFilesAdded(fileInfos);
                } else {
                  addFiles(fileInfos);
                }
                showToast({
                  variant: "success",
                  title: i18n.file.selected.replace(
                    "{count}",
                    String(fileInfos.length)
                  ),
                });
              } else {
                showToast({
                  variant: "warning",
                  title: i18n.error.noFilesSelected,
                });
              }
            } catch (err) {
              console.error("Failed to process dropped files:", err);
              showToast({
                variant: "error",
                title: i18n.error.noFilesSelected,
              });
            }
            break;
          }
          case "leave": {
            dragCounter.current--;
            if (dragCounter.current <= 0) {
              dragCounter.current = 0;
              setIsDragOver(false);
            }
            break;
          }
        }
      });
    };

    setup();

    return () => {
      unlisten?.();
    };
  }, [effectiveDisabled, addFiles, onFilesAdded, showToast, i18n, setIsDragOver]);

  // HTML5 drag events for immediate visual feedback.
  // These fire synchronously before the Tauri native event arrives,
  // providing a snappier UI response.
  const handleDragEnter = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      if (effectiveDisabled) return;
      dragCounter.current++;
      if (dragCounter.current === 1) {
        setIsDragOver(true);
      }
    },
    [effectiveDisabled, setIsDragOver]
  );

  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      if (effectiveDisabled) {
        e.dataTransfer.dropEffect = "none";
      } else {
        e.dataTransfer.dropEffect = "copy";
      }
    },
    [effectiveDisabled]
  );

  const handleDragLeave = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      if (effectiveDisabled) return;
      dragCounter.current--;
      if (dragCounter.current <= 0) {
        dragCounter.current = 0;
        setIsDragOver(false);
      }
    },
    [effectiveDisabled, setIsDragOver]
  );

  // HTML5 onDrop is intentionally a no-op for file processing.
  // File paths are handled by the Tauri native onDragDropEvent above.
  // We only preventDefault here to avoid the browser navigating to the file.
  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      dragCounter.current = 0;
      setIsDragOver(false);
    },
    [setIsDragOver]
  );

  return {
    isDragOver,
    dragProps: {
      onDragEnter: handleDragEnter,
      onDragOver: handleDragOver,
      onDragLeave: handleDragLeave,
      onDrop: handleDrop,
    },
  };
}
