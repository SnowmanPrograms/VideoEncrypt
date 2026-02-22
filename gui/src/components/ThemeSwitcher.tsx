import { useAppStore, type Theme } from "@/stores/appStore";
import { Button } from "@/components/ui/button";
import { Sun, Moon, Monitor } from "lucide-react";

const themeIcons: Record<Theme, React.ReactNode> = {
  light: <Sun className="h-4 w-4" />,
  dark: <Moon className="h-4 w-4" />,
  system: <Monitor className="h-4 w-4" />,
};

const themes: Theme[] = ["light", "dark", "system"];

export function ThemeSwitcher() {
  const theme = useAppStore((state) => state.theme);
  const setTheme = useAppStore((state) => state.setTheme);

  const cycleTheme = () => {
    const currentIndex = themes.indexOf(theme);
    const nextIndex = (currentIndex + 1) % themes.length;
    setTheme(themes[nextIndex]);
  };

  return (
    <Button variant="ghost" size="sm" onClick={cycleTheme} className="gap-2">
      {themeIcons[theme]}
      <span className="text-xs capitalize">{theme}</span>
    </Button>
  );
}
