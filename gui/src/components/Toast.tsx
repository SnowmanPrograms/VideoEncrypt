import { useEffect, useMemo } from "react";
import { useAppStore } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { CheckCircle2, XCircle, AlertTriangle, Info, X } from "lucide-react";

export function Toast() {
  const toast = useAppStore((s) => s.toast);
  const hideToast = useAppStore((s) => s.hideToast);

  useEffect(() => {
    if (!toast) return;
    const duration = toast.durationMs ?? 4500;
    if (duration <= 0) return;
    const timer = window.setTimeout(() => hideToast(), duration);
    return () => window.clearTimeout(timer);
  }, [toast?.id, toast?.durationMs, hideToast, toast]);

  const { icon: Icon, iconClassName, borderClassName } = useMemo(() => {
    switch (toast?.variant) {
      case "success":
        return {
          icon: CheckCircle2,
          iconClassName: "text-green-600",
          borderClassName: "border-green-200 dark:border-green-900",
        };
      case "error":
        return {
          icon: XCircle,
          iconClassName: "text-destructive",
          borderClassName: "border-destructive/30",
        };
      case "warning":
        return {
          icon: AlertTriangle,
          iconClassName: "text-yellow-600",
          borderClassName: "border-yellow-200 dark:border-yellow-900",
        };
      case "info":
      default:
        return {
          icon: Info,
          iconClassName: "text-primary",
          borderClassName: "border-border",
        };
    }
  }, [toast?.variant]);

  if (!toast) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 w-[360px] max-w-[calc(100vw-2rem)] pointer-events-none">
      <div
        className={cn(
          "pointer-events-auto rounded-lg border bg-card shadow-lg",
          "p-3",
          "animate-in fade-in-0 slide-in-from-bottom-2",
          borderClassName
        )}
        role="status"
        aria-live="polite"
      >
        <div className="flex items-start gap-2">
          <Icon className={cn("mt-0.5 h-5 w-5 shrink-0", iconClassName)} />
          <div className="min-w-0 flex-1">
            <div className="flex items-start justify-between gap-2">
              <p className="text-sm font-medium leading-5 truncate">
                {toast.title}
              </p>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 -mr-1 -mt-1"
                onClick={hideToast}
              >
                <X className="h-4 w-4" />
              </Button>
            </div>
            {toast.description && (
              <p className="mt-1 text-xs text-muted-foreground leading-4">
                {toast.description}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

