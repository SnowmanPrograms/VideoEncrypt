import { useAppStore } from "@/stores/appStore";
import { useI18n, t } from "@/stores/i18nStore";
import { Button } from "@/components/ui/button";
import { formatBytes } from "@/lib/utils";
import { FileVideo, Lock, Unlock, Trash2, AlertCircle } from "lucide-react";
import type { FileState } from "@/types";

function FileStateIcon({ state }: { state: FileState }) {
  switch (state) {
    case "Encrypted":
      return <Lock className="h-4 w-4 text-green-500" />;
    case "Locked":
    case "RecoveryNeeded":
      return <AlertCircle className="h-4 w-4 text-yellow-500" />;
    default:
      return <Unlock className="h-4 w-4 text-muted-foreground" />;
  }
}

function FileStateBadge({ state }: { state: FileState }) {
  const i18n = useI18n((s) => s.t);
  const colors: Record<FileState, string> = {
    Normal: "bg-muted text-muted-foreground",
    Encrypted: "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300",
    Locked: "bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300",
    RecoveryNeeded: "bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-300",
  };

  return (
    <span className={`px-2 py-0.5 rounded text-xs font-medium ${colors[state]}`}>
      {i18n.file.status[state]}
    </span>
  );
}

export function FileList() {
  const files = useAppStore((state) => state.files);
  const removeFile = useAppStore((state) => state.removeFile);
  const clearFiles = useAppStore((state) => state.clearFiles);

  if (files.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
        <FileVideo className="h-12 w-12 mb-4 opacity-50" />
        <p>{t("file.noFiles")}</p>
        <p className="text-sm">{t("file.selectHint")}</p>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between mb-4">
        <span className="text-sm text-muted-foreground">
          {t("file.selected", { count: files.length })}
        </span>
        <Button variant="ghost" size="sm" onClick={clearFiles}>
          {t("file.clearAll")}
        </Button>
      </div>

      <div className="max-h-64 overflow-y-auto space-y-1">
        {files.map((file) => (
          <div
            key={file.path}
            className="flex items-center gap-3 p-3 rounded-lg bg-muted/50 hover:bg-muted transition-colors group"
          >
            <FileVideo className="h-5 w-5 text-primary shrink-0" />
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className="font-medium truncate">{file.name}</span>
                <FileStateIcon state={file.state} />
              </div>
              <div className="flex items-center gap-2 mt-1">
                <span className="text-xs text-muted-foreground">
                  {formatBytes(file.size)}
                </span>
                <FileStateBadge state={file.state} />
              </div>
            </div>
            <Button
              variant="ghost"
              size="icon"
              className="opacity-0 group-hover:opacity-100 transition-opacity h-8 w-8"
              onClick={() => removeFile(file.path)}
            >
              <Trash2 className="h-4 w-4 text-destructive" />
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
