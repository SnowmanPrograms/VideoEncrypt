import { useAppStore } from "@/stores/appStore";
import { Progress } from "@/components/ui/progress";
import { Button } from "@/components/ui/button";
import { formatBytes, formatDuration } from "@/lib/utils";
import { cancelTask } from "@/lib/tauri";
import { X, CheckCircle, AlertCircle, Loader2 } from "lucide-react";

function PhaseIndicator({ phase }: { phase: string }) {
  const phases: Record<string, { label: string; color: string }> = {
    Checking: { label: "Checking file...", color: "text-blue-500" },
    Analyzing: { label: "Analyzing structure...", color: "text-purple-500" },
    Backup: { label: "Creating backup...", color: "text-yellow-500" },
    Processing: { label: "Processing data...", color: "text-primary" },
    Finalizing: { label: "Finalizing...", color: "text-green-500" },
    Completed: { label: "Completed", color: "text-green-500" },
    Failed: { label: "Failed", color: "text-destructive" },
    Cancelled: { label: "Cancelled", color: "text-orange-500" },
    Idle: { label: "Ready", color: "text-muted-foreground" },
  };

  const info = phases[phase] || phases.Idle;

  return <span className={`text-sm font-medium ${info.color}`}>{info.label}</span>;
}

export function ProgressPanel() {
  const progress = useAppStore((state) => state.progress);
  const currentTask = useAppStore((state) => state.currentTask);
  const isProcessing = useAppStore((state) => state.isProcessing);
  const reset = useAppStore((state) => state.reset);

  if (!isProcessing && !progress) {
    return null;
  }

  const handleCancel = async () => {
    if (currentTask?.id) {
      await cancelTask(currentTask.id);
    }
  };

  const progressPercent =
    progress?.total_bytes && progress.total_bytes > 0
      ? (progress.processed_bytes / progress.total_bytes) * 100
      : 0;

  return (
    <div className="space-y-4 p-4 border rounded-lg bg-card">
      <div className="flex items-center justify-between">
        <PhaseIndicator phase={progress?.phase || "Idle"} />
        {isProcessing && (
          <Button variant="ghost" size="sm" onClick={handleCancel}>
            <X className="h-4 w-4 mr-1" />
            Cancel
          </Button>
        )}
      </div>

      {progress?.current_file && (
        <p className="text-sm text-muted-foreground truncate">
          {progress.current_file}
        </p>
      )}

      <Progress value={progressPercent} className="h-2" />

      <div className="flex justify-between text-sm text-muted-foreground">
        <span>
          {formatBytes(progress?.processed_bytes || 0)} /{" "}
          {formatBytes(progress?.total_bytes || 0)}
        </span>
        <span>{progressPercent.toFixed(1)}%</span>
      </div>

      {progress?.stats && (
        <div className="grid grid-cols-2 gap-2 text-sm">
          <div className="text-muted-foreground">
            I-Frames: <span className="font-medium text-foreground">{progress.stats.iframe_count}</span>
          </div>
          <div className="text-muted-foreground">
            Audio: <span className="font-medium text-foreground">{progress.stats.audio_count}</span>
          </div>
          <div className="text-muted-foreground">
            Speed: <span className="font-medium text-foreground">{progress.stats.throughput_mbps.toFixed(2)} MB/s</span>
          </div>
        </div>
      )}

      {progress?.message && (
        <p className="text-xs text-muted-foreground">{progress.message}</p>
      )}

      {(progress?.phase === "Completed" ||
        progress?.phase === "Failed" ||
        progress?.phase === "Cancelled") && (
        <div className="flex items-center gap-2 pt-2">
          {progress.phase === "Completed" && (
            <CheckCircle className="h-5 w-5 text-green-500" />
          )}
          {progress.phase === "Failed" && (
            <AlertCircle className="h-5 w-5 text-destructive" />
          )}
          <Button variant="outline" size="sm" onClick={reset}>
            Start New Task
          </Button>
        </div>
      )}
    </div>
  );
}
