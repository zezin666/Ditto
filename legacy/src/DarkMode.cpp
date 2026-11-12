#include "stdafx.h"
#include "DarkMode.h"
#include "Misc.h"

namespace
{
	typedef int(WINAPI* SetPreferredAppModeProc)(int);
	typedef BOOL(WINAPI* AllowDarkModeForWindowProc)(HWND, BOOL);
	typedef HRESULT(WINAPI* SetWindowThemeProc)(HWND, LPCWSTR, LPCWSTR);
	typedef void(WINAPI* FlushMenuThemesProc)();
	typedef void(WINAPI* RefreshImmersiveColorPolicyStateProc)();

	const int PreferredAppModeDefault = 0;
	const int PreferredAppModeAllowDark = 1;
	const int PreferredAppModeForceDark = 2;
	const int PreferredAppModeForceLight = 3;

	HMODULE g_uxtheme = NULL;
	SetPreferredAppModeProc g_setPreferredAppMode = NULL;
	AllowDarkModeForWindowProc g_allowDarkModeForWindow = NULL;
	SetWindowThemeProc g_setWindowTheme = NULL;
	FlushMenuThemesProc g_flushMenuThemes = NULL;
	RefreshImmersiveColorPolicyStateProc g_refreshImmersiveColorPolicyState = NULL;

	bool IsSupportedControlClass(LPCWSTR className)
	{
		return _wcsicmp(className, WC_TREEVIEW) == 0 ||
			_wcsicmp(className, WC_LISTVIEW) == 0 ||
			_wcsicmp(className, WC_EDIT) == 0 ||
			_wcsicmp(className, WC_COMBOBOX) == 0 ||
			_wcsicmp(className, WC_COMBOBOXEX) == 0 ||
			_wcsicmp(className, WC_TABCONTROL) == 0 ||
			_wcsicmp(className, WC_HEADER) == 0 ||
			_wcsicmp(className, WC_BUTTON) == 0 ||
			_wcsicmp(className, WC_STATIC) == 0 ||
			_wcsicmp(className, TRACKBAR_CLASS) == 0 ||
			_wcsicmp(className, HOTKEY_CLASS) == 0 ||
			_wcsicmp(className, _T("ScrollBar")) == 0;
	}

	BOOL CALLBACK ApplyChild(HWND hwnd, LPARAM)
	{
		CDarkMode::ApplyToControl(hwnd);
		return TRUE;
	}
}

bool CDarkMode::s_initialized = false;
bool CDarkMode::s_dark = false;
bool CDarkMode::s_highContrast = false;

bool CDarkMode::EnsureApi()
{
	if (g_uxtheme != NULL)
		return true;

	g_uxtheme = LoadLibrary(_T("uxtheme.dll"));
	if (g_uxtheme == NULL)
		return false;

	g_setWindowTheme = reinterpret_cast<SetWindowThemeProc>(
		GetProcAddress(g_uxtheme, "SetWindowTheme"));
	g_setPreferredAppMode = reinterpret_cast<SetPreferredAppModeProc>(
		GetProcAddress(g_uxtheme, MAKEINTRESOURCEA(135)));
	g_allowDarkModeForWindow = reinterpret_cast<AllowDarkModeForWindowProc>(
		GetProcAddress(g_uxtheme, MAKEINTRESOURCEA(133)));
	g_flushMenuThemes = reinterpret_cast<FlushMenuThemesProc>(
		GetProcAddress(g_uxtheme, MAKEINTRESOURCEA(136)));
	g_refreshImmersiveColorPolicyState = reinterpret_cast<RefreshImmersiveColorPolicyStateProc>(
		GetProcAddress(g_uxtheme, MAKEINTRESOURCEA(104)));

	return g_setWindowTheme != NULL;
}

bool CDarkMode::ApplyPreferredAppMode()
{
	if (g_setPreferredAppMode == NULL)
		return false;

	int mode = s_highContrast ? PreferredAppModeDefault :
		(s_dark ? PreferredAppModeForceDark : PreferredAppModeForceLight);
	g_setPreferredAppMode(mode);

	if (g_refreshImmersiveColorPolicyState != NULL)
		g_refreshImmersiveColorPolicyState();
	if (g_flushMenuThemes != NULL)
		g_flushMenuThemes();

	return true;
}

bool CDarkMode::Initialize()
{
	EnsureApi();
	RefreshSystemTheme();
	return g_setWindowTheme != NULL;
}

void CDarkMode::Shutdown()
{
	if (g_uxtheme != NULL)
	{
		FreeLibrary(g_uxtheme);
		g_uxtheme = NULL;
	}

	g_setPreferredAppMode = NULL;
	g_allowDarkModeForWindow = NULL;
	g_setWindowTheme = NULL;
	g_flushMenuThemes = NULL;
	g_refreshImmersiveColorPolicyState = NULL;
	s_initialized = false;
}

void CDarkMode::RefreshSystemTheme()
{
	s_highContrast = IsHighContrast();
	s_dark = !s_highContrast && DarkAppWindows10Setting();
	s_initialized = true;

	EnsureApi();
	ApplyPreferredAppMode();
}

bool CDarkMode::IsDarkAppMode()
{
	if (!s_initialized)
		RefreshSystemTheme();
	return s_dark;
}

bool CDarkMode::IsHighContrast()
{
	HIGHCONTRAST highContrast = { 0 };
	highContrast.cbSize = sizeof(highContrast);
	if (SystemParametersInfo(SPI_GETHIGHCONTRAST, sizeof(highContrast), &highContrast, 0))
		return (highContrast.dwFlags & HCF_HIGHCONTRASTON) != 0;
	return false;
}

bool CDarkMode::ApplyToControl(HWND hwnd)
{
	if (!IsWindow(hwnd) || !EnsureApi())
		return false;
	if (!s_initialized)
		RefreshSystemTheme();

	if (s_highContrast || !s_dark)
		return RestoreControl(hwnd);

	TCHAR className[128] = { 0 };
	GetClassName(hwnd, className, _countof(className));

	if (!IsSupportedControlClass(className) &&
		_wcsicmp(className, _T("#32770")) != 0 &&
		_wcsicmp(className, _T("#32768")) != 0)
	{
		return false;
	}

	if (g_allowDarkModeForWindow != NULL)
		g_allowDarkModeForWindow(hwnd, TRUE);

	LPCWSTR theme = _wcsicmp(className, WC_COMBOBOX) == 0 ? L"DarkMode_CFD" : L"DarkMode_Explorer";
	return SUCCEEDED(g_setWindowTheme(hwnd, theme, NULL));
}

bool CDarkMode::ApplyToWindow(HWND hwnd)
{
	if (!IsWindow(hwnd) || !EnsureApi())
		return false;
	if (!s_initialized)
		RefreshSystemTheme();

	bool applied = ApplyToControl(hwnd);
	EnumChildWindows(hwnd, ApplyChild, 0);
	return applied;
}

bool CDarkMode::ApplyToMenus(HWND owner)
{
	if (!EnsureApi())
		return false;
	if (!s_initialized)
		RefreshSystemTheme();
	if (!s_dark || s_highContrast)
		return false;

	if (owner != NULL && g_allowDarkModeForWindow != NULL)
		g_allowDarkModeForWindow(owner, TRUE);
	if (owner != NULL && g_setWindowTheme != NULL)
		g_setWindowTheme(owner, L"DarkMode_Explorer", NULL);
	if (g_flushMenuThemes != NULL)
		g_flushMenuThemes();

	return owner != NULL;
}

bool CDarkMode::ApplyDarkMode(HWND hwnd)
{
	return ApplyToWindow(hwnd);
}

bool CDarkMode::RestoreControl(HWND hwnd)
{
	if (!IsWindow(hwnd) || !EnsureApi())
		return false;

	if (g_allowDarkModeForWindow != NULL)
		g_allowDarkModeForWindow(hwnd, FALSE);
	return SUCCEEDED(g_setWindowTheme(hwnd, L"Explorer", NULL));
}
