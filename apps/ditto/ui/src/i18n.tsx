import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import enUS from "../locales/en-US.json";
import zhCN from "../locales/zh-CN.json";

export type LocaleCode = "en-US" | "zh-CN";
type Catalog = Record<string, string>;
type Replacement = string | number;

const catalogs: Record<LocaleCode, Catalog> = {
  "en-US": enUS,
  "zh-CN": zhCN,
};

interface I18nContextValue {
  locale: LocaleCode;
  setLocale: (locale: LocaleCode) => void;
  t: (key: string, replacements?: Record<string, Replacement>) => string;
  missingKeys: string[];
  fallbackKeys: string[];
  localeVersion: string;
  localeKeyCount: number;
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function localeKeys(catalog: Catalog) {
  return Object.keys(catalog)
    .filter((key) => !key.startsWith("$"))
    .sort();
}

function interpolate(template: string, replacements?: Record<string, Replacement>) {
  if (!replacements) {
    return template;
  }
  return template.replace(/\{(\w+)\}/g, (match, key: string) =>
    Object.hasOwn(replacements, key) ? String(replacements[key]) : match,
  );
}

export function I18nProvider({
  initialLocale,
  children,
}: {
  initialLocale: LocaleCode;
  children: ReactNode;
}) {
  const [locale, setLocaleState] = useState<LocaleCode>(initialLocale);
  const [missingKeys, setMissingKeys] = useState<string[]>([]);
  const [fallbackKeys, setFallbackKeys] = useState<string[]>([]);
  const reported = useRef(new Set<string>());

  useEffect(() => {
    setLocaleState(initialLocale);
  }, [initialLocale]);

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  const setLocale = useCallback((nextLocale: LocaleCode) => {
    setLocaleState(nextLocale);
  }, []);

  const report = useCallback(
    (key: string, kind: "missing" | "fallback") => {
      const reportKey = `${kind}:${key}`;
      if (reported.current.has(reportKey)) {
        return;
      }
      reported.current.add(reportKey);
      window.setTimeout(() => {
        if (kind === "missing") {
          setMissingKeys((current) => (current.includes(key) ? current : [...current, key]));
        } else {
          setFallbackKeys((current) => (current.includes(key) ? current : [...current, key]));
        }
      }, 0);
    },
    [],
  );

  const t = useCallback(
    (key: string, replacements?: Record<string, Replacement>) => {
      const localized = catalogs[locale][key];
      if (localized) {
        return interpolate(localized, replacements);
      }
      const fallback = catalogs["en-US"][key];
      if (fallback) {
        report(key, "fallback");
        return interpolate(fallback, replacements);
      }
      report(key, "missing");
      return key;
    },
    [locale, report],
  );

  const value = useMemo<I18nContextValue>(
    () => ({
      locale,
      setLocale,
      t,
      missingKeys,
      fallbackKeys,
      localeVersion: String((catalogs[locale] as Catalog)["$meta.version"] ?? "unknown"),
      localeKeyCount: localeKeys(catalogs[locale]).length,
    }),
    [fallbackKeys, locale, missingKeys, setLocale, t],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const context = useContext(I18nContext);
  if (!context) {
    throw new Error("useI18n must be used inside I18nProvider");
  }
  return context;
}
