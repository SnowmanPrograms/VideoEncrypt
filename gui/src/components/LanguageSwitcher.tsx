import { useI18n } from "@/stores/i18nStore";
import { Button } from "@/components/ui/button";
import { Languages } from "lucide-react";

export function LanguageSwitcher() {
  const locale = useI18n((state) => state.locale);
  const setLocale = useI18n((state) => state.setLocale);

  return (
    <Button
      variant="ghost"
      size="sm"
      onClick={() => setLocale(locale === "en" ? "zh" : "en")}
      className="gap-2"
    >
      <Languages className="h-4 w-4" />
      <span className="text-xs">{locale === "en" ? "中文" : "EN"}</span>
    </Button>
  );
}
