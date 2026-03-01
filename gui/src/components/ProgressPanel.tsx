import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/stores/i18nStore";
import { Progress } from "@/components/ui/progress";
import { Button } from "@/components/ui/button";
import { formatBytes } from "@/lib/utils";
import { cancelTask } from "@/lib/tauri";
import { X, CheckCircle, AlertCircle } from "lucide-react";
import type { ProgressPhase } from "@/types";

function PhaseIndicator({ phase }: { phase: ProgressPhase }) {
  const i18n = useI18n((s) => s.t);
  
  const phases: Record<ProgressPhase, { label: string; color: string }> = {
    Checking: { label: i18n.progress.checking, color: "text-blue-500" },
    Analyzing: { label: i18n.progress.analyzing, color: "text-purple-500" },
    Backup: { label: i18n.progress.backup, color: "text-yellow-500" },
    Processing: { label: i18n.progress.processing, color: "text-primary" },
    Finalizing: { label: i18n.progress.finalizing, color: "text-green-500" },
    Completed: { label: i18n.progress.completed, color: "text-green-500" },
    Failed: { label: i18n.progress.failed, color: "text-destructive" },
    Cancelled: { label: i18n.progress.cancelled, color: "text-orange-500" },
    Idle: { label: i18n.progress.idle, color: "text-muted-foreground" },
  };

  const info = phases[phase] || phases.Idle;

  return <span className={`text-sm font-medium ${info.color}`}>{info.label}</span>;
}

export function ProgressPanel() {
  const i18n = useI18n((s) => s.t);
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
            {i18n.progress.cancel}
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
            {i18n.progress.iframes}:{" "}
            <span className="font-medium text-foreground">{progress.stats.iframe_count}</span>
          </div>
          <div className="text-muted-foreground">
            {i18n.progress.audio}:{" "}
            <span className="font-medium text-foreground">{progress.stats.audio_count}</span>
          </div>
          <div className="text-muted-foreground">
            {i18n.progress.speed}:{" "}
            <span className="font-medium text-foreground">
              {progress.stats.throughput_mbps.toFixed(2)} MB/s
            </span>
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
            {i18n.progress.newTask}
          </Button>
        </div>
      )}
    </div>
  );
}
