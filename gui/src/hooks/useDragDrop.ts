import { useCallback, useRef, useState } from "react";
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
 * Collects file paths from a drag event by recursively walking
 * directory entries via the FileSystemEntry API (webkitGetAsEntry).
 */
async function collectDroppedPaths(e: React.DragEvent): Promise<string[]> {
  const paths: string[] = [];

  const items = e.dataTransfer.items;
  if (!items || items.length === 0) return paths;

  const collectFromEntry = (entry: FileSystemEntry): Promise<void> => {
    return new Promise((resolve) => {
      if (entry.isFile) {
        (entry as FileSystemFileEntry).file(
          (file: File & { path?: string }) => {
            if (file.path) {
              paths.push(file.path);
            }
            resolve();
          },
          () => resolve()
        );
      } else if (entry.isDirectory) {
        const dirReader = (entry as FileSystemDirectoryEntry).createReader();
        dirReader.readEntries(
          async (entries) => {
            for (const child of entries) {
              await collectFromEntry(child);
            }
            resolve();
          },
          () => resolve()
        );
      } else {
        resolve();
      }
    });
  };

  const promises: Promise<void>[] = [];
  for (let i = 0; i < items.length; i++) {
    const entry = items[i]?.webkitGetAsEntry?.();
    if (entry) {
      promises.push(collectFromEntry(entry));
    }
  }

  await Promise.all(promises);
  return paths;
}

export function useDragDrop(
  options: UseDragDropOptions = {}
): UseDragDropReturn {
  const { disabled = false, onFilesAdded } = options;
  const [isDragOver, setIsDragOver] = useState(false);
  const dragCounter = useRef(0);
  const isProcessing = useAppStore((s) => s.isProcessing);
  const addFiles = useAppStore((s) => s.addFiles);
  const showToast = useAppStore((s) => s.showToast);
  const i18n = useI18n((s) => s.t);

  const effectiveDisabled = disabled || isProcessing;

  const handleDragEnter = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (effectiveDisabled) return;
      dragCounter.current++;
      if (dragCounter.current === 1) {
        setIsDragOver(true);
      }
    },
    [effectiveDisabled]
  );

  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
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
      e.stopPropagation();
      if (effectiveDisabled) return;
      dragCounter.current--;
      if (dragCounter.current <= 0) {
        dragCounter.current = 0;
        setIsDragOver(false);
      }
    },
    [effectiveDisabled]
  );

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      dragCounter.current = 0;
      setIsDragOver(false);

      if (effectiveDisabled) return;

      try {
        const droppedPaths = await collectDroppedPaths(e);
        if (droppedPaths.length === 0) {
          showToast({
            variant: "warning",
            title: i18n.error.noFilesSelected,
          });
          return;
        }

        const fileInfos = await addDroppedFiles(droppedPaths, true);
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
    },
    [effectiveDisabled, addFiles, onFilesAdded, showToast, i18n]
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
