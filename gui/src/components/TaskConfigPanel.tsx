import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/stores/i18nStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { getTaskStatus, startTask } from "@/lib/tauri";
import { Lock, Unlock, Loader2 } from "lucide-react";

export function TaskConfigPanel() {
  const i18n = useI18n((s) => s.t);
  const {
    files,
    taskMode,
    password,
    setPassword,
    confirmPassword,
    setConfirmPassword,
    encryptAudio,
    setEncryptAudio,
    scrubMetadata,
    setScrubMetadata,
    useWal,
    setUseWal,
    isProcessing,
    setIsProcessing,
    setCurrentTask,
    setProgress,
    hideToast,
    showToast,
  } = useAppStore();

  const handleStartTask = async () => {
    if (files.length === 0 || !password) return;
    if (taskMode === "Encrypt" && password !== confirmPassword) return;

    setCurrentTask(null);
    setIsProcessing(true);
    setProgress(null);
    hideToast();

    try {
      const taskId = await startTask({
        files: files.map((f) => f.path),
        password,
        mode: taskMode,
        encrypt_audio: encryptAudio,
        scrub_metadata: scrubMetadata,
        use_wal: useWal,
      });

      const task = await getTaskStatus(taskId);
      setCurrentTask(task);
    } catch (e) {
      console.error("Failed to start task:", e);
      setIsProcessing(false);
      showToast({
        variant: "error",
        title: i18n.progress.failed,
        description: String(e),
        durationMs: 8000,
      });
    }
  };

  const canStart =
    files.length > 0 &&
    password.length > 0 &&
    (taskMode === "Decrypt" || password === confirmPassword);

  return (
    <div className="space-y-4">
      <div className="space-y-3">
        <div className="space-y-2">
          <Label htmlFor="password" className="text-xs">{i18n.config.password}</Label>
          <Input
            id="password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={i18n.config.passwordPlaceholder}
            className="h-8 text-sm"
            disabled={isProcessing}
          />
        </div>

        {taskMode === "Encrypt" && (
          <div className="space-y-2">
            <Label htmlFor="confirm-password" className="text-xs">{i18n.config.confirmPassword}</Label>
            <Input
              id="confirm-password"
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder={i18n.config.confirmPlaceholder}
              disabled={isProcessing}
              className={`h-8 text-sm ${
                confirmPassword && password !== confirmPassword
                  ? "border-destructive" : ""
              }`}
            />
            {confirmPassword && password !== confirmPassword && (
              <p className="text-xs text-destructive">{i18n.config.passwordMismatch}</p>
            )}
          </div>
        )}
      </div>

      {taskMode === "Encrypt" && (
        <div className="space-y-3">
          <Label className="text-xs font-semibold text-muted-foreground uppercase">
            {i18n.config.options}
          </Label>
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="encrypt-audio" className="text-sm">{i18n.config.encryptAudio}</Label>
                <p className="text-xs text-muted-foreground">
                  {i18n.config.encryptAudioHint}
                </p>
              </div>
              <Switch
                id="encrypt-audio"
                checked={encryptAudio}
                onCheckedChange={setEncryptAudio}
                disabled={isProcessing}
              />
            </div>

            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="scrub-metadata" className="text-sm">{i18n.config.scrubMetadata}</Label>
                <p className="text-xs text-muted-foreground">
                  {i18n.config.scrubMetadataHint}
                </p>
              </div>
              <Switch
                id="scrub-metadata"
                checked={scrubMetadata}
                onCheckedChange={setScrubMetadata}
                disabled={isProcessing}
              />
            </div>
          </div>
        </div>
      )}

      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <Label htmlFor="use-wal" className="text-sm">{i18n.config.crashSafety}</Label>
          <p className="text-xs text-muted-foreground">
            {i18n.config.crashSafetyHint}
          </p>
        </div>
        <Switch
          id="use-wal"
          checked={useWal}
          onCheckedChange={setUseWal}
          disabled={isProcessing}
        />
      </div>

      <Button
        className="w-full"
        size="sm"
        onClick={handleStartTask}
        disabled={!canStart || isProcessing}
      >
        {isProcessing ? (
          <>
            <Loader2 className="h-3.5 w-3.5 mr-1.5 animate-spin" />
            {i18n.config.processing}
          </>
        ) : taskMode === "Encrypt" ? (
          <>
            <Lock className="h-3.5 w-3.5 mr-1.5" />
            {i18n.config.startEncrypt}
          </>
        ) : (
          <>
            <Unlock className="h-3.5 w-3.5 mr-1.5" />
            {i18n.config.startDecrypt}
          </>
        )}
      </Button>
    </div>
  );
}
