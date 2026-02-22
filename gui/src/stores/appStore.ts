import { create } from "zustand";
import type { FileInfo, TaskInfo, TaskMode, ProgressEvent } from "@/types";

interface AppState {
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

  setFiles: (files: FileInfo[]) => void;
  addFiles: (files: FileInfo[]) => void;
  removeFile: (path: string) => void;
  clearFiles: () => void;
  setTaskMode: (mode: TaskMode) => void;
  setPassword: (password: string) => void;
  setConfirmPassword: (password: string) => void;
  setEncryptAudio: (value: boolean) => void;
  setScrubMetadata: (value: boolean) => void;
  setUseWal: (value: boolean) => void;
  setCurrentTask: (task: TaskInfo | null) => void;
  setProgress: (progress: ProgressEvent | null) => void;
  setIsProcessing: (value: boolean) => void;
  reset: () => void;
}

const initialState = {
  files: [],
  taskMode: "Encrypt" as TaskMode,
  password: "",
  confirmPassword: "",
  encryptAudio: false,
  scrubMetadata: false,
  useWal: true,
  currentTask: null,
  progress: null,
  isProcessing: false,
};

export const useAppStore = create<AppState>((set) => ({
  ...initialState,

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
  setTaskMode: (taskMode) => set({ taskMode }),
  setPassword: (password) => set({ password }),
  setConfirmPassword: (confirmPassword) => set({ confirmPassword }),
  setEncryptAudio: (encryptAudio) => set({ encryptAudio }),
  setScrubMetadata: (scrubMetadata) => set({ scrubMetadata }),
  setUseWal: (useWal) => set({ useWal }),
  setCurrentTask: (currentTask) => set({ currentTask }),
  setProgress: (progress) => set({ progress }),
  setIsProcessing: (isProcessing) => set({ isProcessing }),
  reset: () => set(initialState),
}));
