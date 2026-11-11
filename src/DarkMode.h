#pragma once

// Best-effort native control/menu dark mode. All undocumented uxtheme entry
// points are resolved at runtime and are safe no-ops when unavailable.
class CDarkMode
{
public:
	static bool Initialize();
	static void Shutdown();
	static void RefreshSystemTheme();

	static bool IsDarkAppMode();
	static bool IsHighContrast();

	static bool ApplyToWindow(HWND hwnd);
	static bool ApplyToControl(HWND hwnd);
	static bool ApplyToMenus(HWND owner);
	static bool ApplyDarkMode(HWND hwnd);
	static bool RestoreControl(HWND hwnd);

private:
	static bool EnsureApi();
	static bool ApplyPreferredAppMode();

	static bool s_initialized;
	static bool s_dark;
	static bool s_highContrast;
};
