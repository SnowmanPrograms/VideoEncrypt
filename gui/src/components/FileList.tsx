import { useAppStore } from "@/stores/appStore";
import { useI18n, t } from "@/stores/i18nStore";
import { Button } from "@/components/ui/button";
import { formatBytes } from "@/lib/utils";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { FileVideo, Lock, Unlock, Trash2, AlertCircle } from "lucide-react";
import type { FileState } from "@/types";

function FileStateIcon({ state }: { state: FileState }) {
  switch (state) {
    case "Encrypted":
      return <Lock className="h-3.5 w-3.5 text-green-500" />;
    case "Locked":
    case "RecoveryNeeded":
      return <AlertCircle className="h-3.5 w-3.5 text-yellow-500" />;
    default:
      return <Unlock className="h-3.5 w-3.5 text-muted-foreground" />;
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
    <span className={`px-1.5 py-0.5 rounded text-xs font-medium ${colors[state]}`}>
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
      <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
        <FileVideo className="h-10 w-10 mb-3 opacity-50" />
        <p className="text-sm">{t("file.noFiles")}</p>
        <p className="text-xs">{t("file.selectHint")}</p>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs text-muted-foreground">
          {t("file.selected", { count: files.length })}
        </span>
        <Button variant="ghost" size="sm" onClick={clearFiles} className="h-7 text-xs">
          {t("file.clearAll")}
        </Button>
      </div>

      <div className="border rounded-lg overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-8"></TableHead>
              <TableHead>文件名</TableHead>
              <TableHead className="w-20">大小</TableHead>
              <TableHead className="w-24">状态</TableHead>
              <TableHead className="w-10"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {files.map((file) => (
              <TableRow key={file.path}>
                <TableCell>
                  <FileVideo className="h-3.5 w-3.5 text-primary" />
                </TableCell>
                <TableCell className="font-medium">
                  <div className="flex items-center gap-1.5">
                    <span className="truncate text-sm">{file.name}</span>
                    <FileStateIcon state={file.state} />
                  </div>
                </TableCell>
                <TableCell className="text-xs text-muted-foreground">
                  {formatBytes(file.size)}
                </TableCell>
                <TableCell>
                  <FileStateBadge state={file.state} />
                </TableCell>
                <TableCell>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-6 w-6"
                    onClick={() => removeFile(file.path)}
                  >
                    <Trash2 className="h-3 w-3 text-destructive" />
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}
