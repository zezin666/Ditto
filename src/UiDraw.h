#pragma once

// Cached drawing primitives for UI controls. All functions are UI-thread only.
namespace UiDraw
{
	enum class Glyph
	{
		Close,
		ChevronUp,
		ChevronDown,
		ChevronLeft,
		ChevronRight,
		Search,
		Folder,
		Cog,
		Check,
		Pin,
		Dots,
		Menu,
		SystemMenu
	};

	int Scale(int logicalPixels, int dpi);
	int IconSizeForDpi(int dpi);

	HFONT Font(const CString& family, int logicalPixels, int weight, int dpi, bool italic = false);
	HBRUSH Brush(COLORREF color);
	HPEN Pen(COLORREF color, int widthPixels);

	CSize MeasureText(CDC* dc, const CString& text, const CString& family,
		int logicalPixels, int weight, int dpi, bool italic = false);
	int DrawTextEllipsis(CDC* dc, const CString& text, const CRect& rect,
		const CString& family, int logicalPixels, int weight, COLORREF color,
		int dpi, UINT drawFlags = DT_LEFT | DT_VCENTER | DT_SINGLELINE, bool italic = false);

	void RoundedRect(Gdiplus::Graphics& graphics, const CRect& rect, int radiusPixels,
		COLORREF fillColor, COLORREF borderColor, int borderWidthPixels);
	void RoundedRect(CDC* dc, const CRect& rect, int radiusPixels,
		COLORREF fillColor, COLORREF borderColor, int borderWidthPixels);
	void FocusRing(CDC* dc, const CRect& rect, int radiusPixels,
		COLORREF color, int widthPixels);
	void DrawFallbackBorder(CDC* dc, const CRect& rect, COLORREF color);

	void DrawGlyph(Gdiplus::Graphics& graphics, const CRect& rect, Glyph glyph,
		COLORREF color, int dpi, int strokeTenths = 15);
	void DrawGlyph(CDC* dc, const CRect& rect, Glyph glyph,
		COLORREF color, int dpi, int strokeTenths = 15);
	void DrawIcon(CDC* dc, Gdiplus::Bitmap* bitmap, const CRect& rect,
		bool tinted, COLORREF tint, int alpha = 255);

	void PurgeCaches();
}
