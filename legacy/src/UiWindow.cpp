#include "stdafx.h"
#include "UiWindow.h"
#include "UiTheme.h"

#ifndef DWMWA_USE_IMMERSIVE_DARK_MODE
#define DWMWA_USE_IMMERSIVE_DARK_MODE 20
#endif
#ifndef DWMWA_WINDOW_CORNER_PREFERENCE
#define DWMWA_WINDOW_CORNER_PREFERENCE 33
#endif
#ifndef DWMWA_BORDER_COLOR
#define DWMWA_BORDER_COLOR 34
#endif
#ifndef DWMWA_CAPTION_COLOR
#define DWMWA_CAPTION_COLOR 35
#endif
#ifndef DWMWA_TEXT_COLOR
#define DWMWA_TEXT_COLOR 36
#endif
#ifndef DWMWCP_ROUND
#define DWMWCP_ROUND 2
#endif

namespace
{
	typedef HRESULT(WINAPI* DwmSetWindowAttributeProc)(HWND, DWORD, LPCVOID, DWORD);
	typedef HRESULT(WINAPI* RtlGetVersionProc)(OSVERSIONINFOEXW*);

	DwmSetWindowAttributeProc GetDwmSetWindowAttribute()
	{
		static HMODULE module = LoadLibrary(_T("dwmapi.dll"));
		if (module == NULL)
			return NULL;

		static DwmSetWindowAttributeProc proc =
			reinterpret_cast<DwmSetWindowAttributeProc>(
				GetProcAddress(module, "DwmSetWindowAttribute"));
		return proc;
	}

	bool SetDwmAttribute(HWND hwnd, DWORD attribute, const void* value, DWORD size)
	{
		DwmSetWindowAttributeProc proc = GetDwmSetWindowAttribute();
		return proc != NULL && SUCCEEDED(proc(hwnd, attribute, value, size));
	}
}

bool UiWindow::IsWindows11OrGreater()
{
	OSVERSIONINFOEXW version = { 0 };
	version.dwOSVersionInfoSize = sizeof(version);

	HMODULE ntdll = GetModuleHandle(_T("ntdll.dll"));
	if (ntdll == NULL)
		return false;

	RtlGetVersionProc rtlGetVersion = reinterpret_cast<RtlGetVersionProc>(
		GetProcAddress(ntdll, "RtlGetVersion"));
	if (rtlGetVersion == NULL || rtlGetVersion(&version) != 0)
		return false;

	return version.dwMajorVersion > 10 ||
		(version.dwMajorVersion == 10 && version.dwBuildNumber >= 22000);
}

bool UiWindow::SetDarkTitleBar(HWND hwnd, bool dark)
{
	if (!IsWindow(hwnd))
		return false;

	BOOL value = dark ? TRUE : FALSE;
	if (SetDwmAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &value, sizeof(value)))
		return true;

	// Older Windows 10 builds used attribute 19.
	const DWORD legacy = 19;
	return SetDwmAttribute(hwnd, legacy, &value, sizeof(value));
}

bool UiWindow::ApplyWindowEffects(HWND hwnd)
{
	const CUiTheme& theme = UiTheme();
	Effects effects = {
		theme.IsDark(),
		IsWindows11OrGreater(),
		true,
		theme.Border(),
		theme.SurfaceAlt(),
		theme.TextPrimary()
	};
	return ApplyWindowEffects(hwnd, effects);
}

bool UiWindow::ApplyWindowEffects(HWND hwnd, const Effects& effects)
{
	if (!IsWindow(hwnd))
		return false;

	bool applied = SetDarkTitleBar(hwnd, effects.dark);

	if (IsWindows11OrGreater())
	{
		if (effects.rounded)
		{
			int preference = DWMWCP_ROUND;
			applied = SetDwmAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE,
				&preference, sizeof(preference)) || applied;
		}

		COLORREF border = effects.borderColor;
		applied = SetDwmAttribute(hwnd, DWMWA_BORDER_COLOR, &border, sizeof(border)) || applied;

		if (effects.useCaptionColor)
		{
			COLORREF caption = effects.captionColor;
			COLORREF text = effects.textColor;
			applied = SetDwmAttribute(hwnd, DWMWA_CAPTION_COLOR, &caption, sizeof(caption)) || applied;
			applied = SetDwmAttribute(hwnd, DWMWA_TEXT_COLOR, &text, sizeof(text)) || applied;
		}
	}

	return applied;
}

void UiWindow::DrawFallbackBorder(CDC* dc, const CRect& rect, COLORREF color)
{
	if (dc == NULL || rect.Width() <= 0 || rect.Height() <= 0)
		return;

	CPen pen(PS_SOLID, 1, color);
	CPen* oldPen = dc->SelectObject(&pen);
	CBrush* oldBrush = static_cast<CBrush*>(dc->SelectStockObject(NULL_BRUSH));
	dc->Rectangle(rect.left, rect.top, rect.right, rect.bottom);
	dc->SelectObject(oldBrush);
	dc->SelectObject(oldPen);
}

void UiWindow::RemoveWindowEffects(HWND hwnd)
{
	if (!IsWindow(hwnd))
		return;

	int preference = 1; // DWMWCP_DONOTROUND
	SetDwmAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &preference, sizeof(preference));
	SetDarkTitleBar(hwnd, false);
}
