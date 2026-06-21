import { writable, derived, get } from "svelte/store";
import { en } from "./locales/en";
import { ja } from "./locales/ja";
import { es } from "./locales/es";
import { fr } from "./locales/fr";
import { de } from "./locales/de";
import { zh } from "./locales/zh";

export type LocaleCode = "en" | "ja" | "es" | "fr" | "de" | "zh";

export const locales: Record<LocaleCode, string> = {
  en: "English",
  ja: "日本語",
  es: "Español",
  fr: "Français",
  de: "Deutsch",
  zh: "中文",
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Dict = Record<string, any>;

const dictionaries: Record<LocaleCode, Dict> = { en, ja, es, fr, de, zh };

const STORAGE_KEY = "kve.locale";

function detectLocale(): LocaleCode {
  try {
    const saved = localStorage.getItem(STORAGE_KEY) as LocaleCode | null;
    if (saved && saved in locales) return saved;
  } catch {
    /* ignore */
  }
  const nav = (typeof navigator !== "undefined" ? navigator.language : "en").slice(0, 2).toLowerCase();
  if (nav in locales) return nav as LocaleCode;
  return "en";
}

export const locale = writable<LocaleCode>(detectLocale());

locale.subscribe((val) => {
  try {
    localStorage.setItem(STORAGE_KEY, val);
    document.documentElement.lang = val;
  } catch {
    /* ignore */
  }
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function lookup(dict: Dict, key: string): any {
  const parts = key.split(".");
  let cur: unknown = dict;
  for (const p of parts) {
    if (cur && typeof cur === "object" && p in (cur as Dict)) {
      cur = (cur as Dict)[p];
    } else {
      return undefined;
    }
  }
  return cur;
}

export const t = derived(locale, ($locale) => {
  return (key: string, vars?: Record<string, string | number>): string => {
    let val = lookup(dictionaries[$locale], key);
    if (val === undefined) val = lookup(dictionaries.en, key);
    if (val === undefined) return key;
    if (typeof val !== "string") return String(val);
    if (vars) {
      for (const [k, v] of Object.entries(vars)) {
        val = (val as string).replace(new RegExp(`\\{${k}\\}`, "g"), String(v));
      }
    }
    return val as string;
  };
});

export function setLocale(code: LocaleCode): void {
  locale.set(code);
}

export function getCurrentLocale(): LocaleCode {
  return get(locale);
}
