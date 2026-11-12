#pragma once

namespace UiWindow
{
	struct Effects
	{
		bool dark;
		bool rounded;
		bool useCaptionColor;
		COLORREF borderColor;
		COLORREF captionColor;
		COLORREF textColor;
	};

	bool ApplyWindowEffects(HWND hwnd);
	bool ApplyWindowEffects(HWND hwnd, const Effects& effects);
	bool SetDarkTitleBar(HWND hwnd, bool dark);
	bool IsWindows11OrGreater();
	void DrawFallbackBorder(CDC* dc, const CRect& rect, COLORREF color);
	void RemoveWindowEffects(HWND hwnd);
}
