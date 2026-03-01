import { useI18n } from "@/stores/i18nStore";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Languages } from "lucide-react";

export function LanguageSwitcher() {
  const locale = useI18n((state) => state.locale);
  const setLocale = useI18n((state) => state.setLocale);

  return (
    <Select value={locale} onValueChange={(value) => setLocale(value as "en" | "zh")}>
      <SelectTrigger className="w-[130px] h-8">
        <Languages className="h-3.5 w-3.5 mr-1.5" />
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="en">English</SelectItem>
        <SelectItem value="zh">中文</SelectItem>
      </SelectContent>
    </Select>
  );
}
