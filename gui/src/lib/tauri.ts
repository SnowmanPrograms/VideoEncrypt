import { invoke } from "@tauri-apps/api/core";
import type { FileInfo, TaskConfig, TaskInfo, FileState } from "@/types";

export async function selectFiles(recursive: boolean): Promise<FileInfo[]> {
  return invoke("select_files", { recursive });
}

export async function selectFolder(recursive: boolean): Promise<FileInfo[]> {
  return invoke("select_folder", { recursive });
}

export async function startTask(config: TaskConfig): Promise<string> {
  return invoke("start_task", { config });
}

export async function cancelTask(taskId: string): Promise<void> {
  return invoke("cancel_task", { taskId });
}

export async function getTaskStatus(taskId: string): Promise<TaskInfo> {
  return invoke("get_task_status", { taskId });
}

export async function checkFileStatus(path: string): Promise<FileState> {
  return invoke("check_file_status", { path });
}

export function getSupportedExtensions(): string[] {
  return ["mp4", "m4v", "mov", "mkv", "webm", "m4a", "mka"];
}

export async function addDroppedFiles(
  paths: string[],
  recursive: boolean
): Promise<FileInfo[]> {
  return invoke("add_dropped_files", { paths, recursive });
}
