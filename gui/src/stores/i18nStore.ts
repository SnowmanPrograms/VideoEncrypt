import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { en, zh } from "@/lib/i18n";

export type Locale = "en" | "zh";

interface I18nState {
  locale: Locale;
  t: typeof en;
  setLocale: (locale: Locale) => void;
}

export const useI18n = create<I18nState>()(
  persist(
    (set) => ({
      locale: "en",
      t: en,
      setLocale: (locale) =>
        set({
          locale,
          t: locale === "zh" ? (zh as unknown as typeof en) : en,
        }),
    }),
    {
      name: "media-lock-locale",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ locale: state.locale }),
    }
  )
);

export function t(key: string, params?: Record<string, string | number>): string {
  const state = useI18n.getState();
  const keys = key.split(".");
  let value: unknown = state.t;

  for (const k of keys) {
    if (value && typeof value === "object" && k in value) {
      value = (value as Record<string, unknown>)[k];
    } else {
      return key;
    }
  }

  if (typeof value !== "string") {
    return key;
  }

  if (params) {
    return Object.entries(params).reduce(
      (str, [k, v]) => str.replace(`{${k}}`, String(v)),
      value
    );
  }

  return value;
}
