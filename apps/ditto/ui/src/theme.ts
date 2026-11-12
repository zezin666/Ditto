import {
  teamsHighContrastTheme,
  webDarkTheme,
  webLightTheme,
  type Theme,
} from "@fluentui/react-components";
import { tokens } from "./tokens.generated";
import type { ThemeChoice } from "./types";

const light = tokens["color.canvas"].$value.light;
const dark = tokens["color.canvas"].$value.dark;
export const uiFontFamily = tokens["font.family.ui"].$value.light.join(", ");

const lightTheme: Theme = {
  ...webLightTheme,
  colorNeutralBackground1: tokens["color.surface"].$value.light,
  colorNeutralBackground2: tokens["color.surfaceAlt"].$value.light,
  colorNeutralBackground3: tokens["color.surfaceSunken"].$value.light,
  colorNeutralForeground1: tokens["color.textPrimary"].$value.light,
  colorNeutralForeground2: tokens["color.textSecondary"].$value.light,
  colorNeutralForegroundDisabled: tokens["color.textDisabled"].$value.light,
  colorNeutralStroke1: tokens["color.border"].$value.light,
  colorNeutralStroke2: tokens["color.borderStrong"].$value.light,
  colorBrandBackground: tokens["color.accentFill"].$value.light,
  colorBrandBackgroundHover: tokens["color.accentHover"].$value.light,
  colorBrandBackgroundPressed: tokens["color.accentPressed"].$value.light,
  colorBrandForeground1: tokens["color.accent"].$value.light,
  colorNeutralForegroundOnBrand: tokens["color.onAccent"].$value.light,
  colorSubtleBackgroundHover: tokens["color.hover"].$value.light,
  colorSubtleBackgroundSelected: tokens["color.selection"].$value.light,
  colorStrokeFocus2: tokens["color.focus"].$value.light,
  colorPaletteRedForeground1: tokens["color.danger"].$value.light,
  colorPaletteGreenForeground1: tokens["color.pasted"].$value.light,
  fontFamilyBase: tokens["font.family.ui"].$value.light.join(", "),
  fontSizeBase300: `${tokens["font.body.size"].$value.light}px`,
  lineHeightBase300: `${tokens["font.body.lineHeight"].$value.light}px`,
  borderRadiusMedium: `${tokens["radius.control"].$value.light}px`,
  borderRadiusLarge: `${tokens["radius.window"].$value.light}px`,
};

const darkTheme: Theme = {
  ...webDarkTheme,
  colorNeutralBackground1: tokens["color.surface"].$value.dark,
  colorNeutralBackground2: tokens["color.surfaceAlt"].$value.dark,
  colorNeutralBackground3: tokens["color.surfaceSunken"].$value.dark,
  colorNeutralForeground1: tokens["color.textPrimary"].$value.dark,
  colorNeutralForeground2: tokens["color.textSecondary"].$value.dark,
  colorNeutralForegroundDisabled: tokens["color.textDisabled"].$value.dark,
  colorNeutralStroke1: tokens["color.border"].$value.dark,
  colorNeutralStroke2: tokens["color.borderStrong"].$value.dark,
  colorBrandBackground: tokens["color.accentFill"].$value.dark,
  colorBrandBackgroundHover: tokens["color.accentHover"].$value.dark,
  colorBrandBackgroundPressed: tokens["color.accentPressed"].$value.dark,
  colorBrandForeground1: tokens["color.accent"].$value.dark,
  colorNeutralForegroundOnBrand: tokens["color.onAccent"].$value.dark,
  colorSubtleBackgroundHover: tokens["color.hover"].$value.dark,
  colorSubtleBackgroundSelected: tokens["color.selection"].$value.dark,
  colorStrokeFocus2: tokens["color.focus"].$value.dark,
  colorPaletteRedForeground1: tokens["color.danger"].$value.dark,
  colorPaletteGreenForeground1: tokens["color.pasted"].$value.dark,
  fontFamilyBase: tokens["font.family.ui"].$value.dark.join(", "),
  fontSizeBase300: `${tokens["font.body.size"].$value.dark}px`,
  lineHeightBase300: `${tokens["font.body.lineHeight"].$value.dark}px`,
  borderRadiusMedium: `${tokens["radius.control"].$value.dark}px`,
  borderRadiusLarge: `${tokens["radius.window"].$value.dark}px`,
};

export interface ResolvedTheme {
  mode: "light" | "dark" | "hc";
  theme: Theme;
}

export function resolveTheme(choice: ThemeChoice): ResolvedTheme {
  const systemHighContrast =
    typeof window !== "undefined" &&
    window.matchMedia("(forced-colors: active)").matches;
  if (choice === "highContrast" || (choice === "system" && systemHighContrast)) {
    return { mode: "hc", theme: teamsHighContrastTheme };
  }
  if (choice === "dark") {
    return { mode: "dark", theme: darkTheme };
  }
  if (choice === "light") {
    return { mode: "light", theme: lightTheme };
  }
  const systemDark =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches;
  return systemDark
    ? { mode: "dark", theme: darkTheme }
    : { mode: "light", theme: lightTheme };
}

export const themeCanvas = {
  light,
  dark,
};
