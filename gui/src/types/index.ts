export interface FileInfo {
  path: string;
  name: string;
  size: number;
  state: FileState;
}

export type FileState = "Normal" | "Encrypted" | "Locked" | "RecoveryNeeded";

export interface TaskConfig {
  files: string[];
  password: string;
  mode: TaskMode;
  encrypt_audio: boolean;
  scrub_metadata: boolean;
  use_wal: boolean;
}

export type TaskMode = "Encrypt" | "Decrypt";

export interface TaskInfo {
  id: string;
  config: TaskConfig;
  status: TaskStatus;
  current_file_index: number;
  total_files: number;
  current_file: string | null;
  progress: number;
  error: string | null;
  results: FileResult[];
}

export type TaskStatus =
  | "Pending"
  | "Running"
  | "Paused"
  | "Completed"
  | "Failed"
  | "Cancelled";

export interface FileResult {
  path: string;
  success: boolean;
  error: string | null;
  stats: FileStats | null;
}

export interface FileStats {
  file_size: number;
  data_size: number;
  iframe_count: number;
  audio_count: number;
  total_time_ms: number;
  throughput_mbps: number;
}

export interface ProgressEvent {
  task_id: string;
  phase: ProgressPhase;
  total_bytes: number;
  processed_bytes: number;
  current_file: string | null;
  message: string;
  stats: TaskProgressStats | null;
}

export type ProgressPhase =
  | "Idle"
  | "Checking"
  | "Analyzing"
  | "Backup"
  | "Processing"
  | "Finalizing"
  | "Completed"
  | "Failed"
  | "Cancelled";

export interface TaskProgressStats {
  iframe_count: number;
  audio_count: number;
  parse_time_ms: number;
  kdf_time_ms: number;
  io_time_ms: number;
  crypto_time_ms: number;
  throughput_mbps: number;
}

export interface GuiError {
  kind: string;
  message: string;
}
