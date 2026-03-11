import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { FileInfo, FileState, TaskInfo, TaskMode, ProgressEvent } from "@/types";

export type Theme = "light" | "dark" | "system";

export type ToastVariant = "success" | "error" | "warning" | "info";

export interface ToastMessage {
  id: string;
  variant: ToastVariant;
  title: string;
  description?: string;
  durationMs?: number;
}

interface AppState {
  theme: Theme;
  files: FileInfo[];
  taskMode: TaskMode;
  password: string;
  confirmPassword: string;
  encryptAudio: boolean;
  scrubMetadata: boolean;
  useWal: boolean;
  currentTask: TaskInfo | null;
  progress: ProgressEvent | null;
  isProcessing: boolean;
  toast: ToastMessage | null;

  setTheme: (theme: Theme) => void;
  setFiles: (files: FileInfo[]) => void;
  addFiles: (files: FileInfo[]) => void;
  removeFile: (path: string) => void;
  clearFiles: () => void;
  updateFileState: (path: string, state: FileState) => void;
  setTaskMode: (mode: TaskMode) => void;
  setPassword: (password: string) => void;
  setConfirmPassword: (password: string) => void;
  setEncryptAudio: (value: boolean) => void;
  setScrubMetadata: (value: boolean) => void;
  setUseWal: (value: boolean) => void;
  setCurrentTask: (task: TaskInfo | null) => void;
  setProgress: (progress: ProgressEvent | null) => void;
  setIsProcessing: (value: boolean) => void;
  showToast: (toast: Omit<ToastMessage, "id"> & { id?: string }) => void;
  hideToast: () => void;
  reset: () => void;
}

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      theme: "system",
      files: [],
      taskMode: "Encrypt",
      password: "",
      confirmPassword: "",
      encryptAudio: false,
      scrubMetadata: false,
      useWal: true,
      currentTask: null,
      progress: null,
      isProcessing: false,
      toast: null,

      setTheme: (theme) => set({ theme }),
      setFiles: (files) => set({ files }),
      addFiles: (files) =>
        set((state) => {
          const existingPaths = new Set(state.files.map((f) => f.path));
          const newFiles = files.filter((f) => !existingPaths.has(f.path));
          return { files: [...state.files, ...newFiles] };
        }),
      removeFile: (path) =>
        set((state) => ({
          files: state.files.filter((f) => f.path !== path),
        })),
      clearFiles: () => set({ files: [] }),
      updateFileState: (path, state) =>
        set((s) => ({
          files: s.files.map((f) => (f.path === path ? { ...f, state } : f)),
        })),
      setTaskMode: (taskMode) => set({ taskMode }),
      setPassword: (password) => set({ password }),
      setConfirmPassword: (confirmPassword) => set({ confirmPassword }),
      setEncryptAudio: (encryptAudio) => set({ encryptAudio }),
      setScrubMetadata: (scrubMetadata) => set({ scrubMetadata }),
      setUseWal: (useWal) => set({ useWal }),
      setCurrentTask: (currentTask) => set({ currentTask }),
      setProgress: (progress) => set({ progress }),
      setIsProcessing: (isProcessing) => set({ isProcessing }),
      showToast: (toast) =>
        set({
          toast: {
            id: toast.id ?? `${Date.now()}`,
            variant: toast.variant,
            title: toast.title,
            description: toast.description,
            durationMs: toast.durationMs,
          },
        }),
      hideToast: () => set({ toast: null }),
      reset: () =>
        set({
          files: [],
          taskMode: "Encrypt",
          password: "",
          confirmPassword: "",
          currentTask: null,
          progress: null,
          isProcessing: false,
          toast: null,
        }),
    }),
    {
      name: "media-lock-settings",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        theme: state.theme,
        encryptAudio: state.encryptAudio,
        scrubMetadata: state.scrubMetadata,
        useWal: state.useWal,
      }),
    }
  )
);
