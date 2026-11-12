#include "stdafx.h"
#include "UiTheme.h"
#include "UiDraw.h"
#include "DarkMode.h"
#include <algorithm>
#include <cmath>

#ifdef min
#undef min
#endif
#ifdef max
#undef max
#endif

namespace
{
	double Luminance(COLORREF color)
	{
		auto channel = [](BYTE value) -> double
		{
			double v = value / 255.0;
			return v <= 0.03928 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4);
		};

		return 0.2126 * channel(GetRValue(color)) +
			0.7152 * channel(GetGValue(color)) +
			0.0722 * channel(GetBValue(color));
	}

	double Contrast(COLORREF first, COLORREF second)
	{
		double a = Luminance(first);
		double b = Luminance(second);
		if (a < b)
			std::swap(a, b);
		return (a + 0.05) / (b + 0.05);
	}

	bool IsDarkColor(COLORREF color)
	{
		return Luminance(color) < 0.18;
	}

	BYTE BlendChannel(BYTE foreground, BYTE background, int percentForeground)
	{
		percentForeground = std::max(0, std::min(100, percentForeground));
		return static_cast<BYTE>((foreground * percentForeground +
			background * (100 - percentForeground) + 50) / 100);
	}

	COLORREF Blend(COLORREF foreground, COLORREF background, int percentForeground)
	{
		return RGB(
			BlendChannel(GetRValue(foreground), GetRValue(background), percentForeground),
			BlendChannel(GetGValue(foreground), GetGValue(background), percentForeground),
			BlendChannel(GetBValue(foreground), GetBValue(background), percentForeground));
	}

	COLORREF BestTextOn(COLORREF background)
	{
		COLORREF dark = RGB(11, 18, 32);
		COLORREF light = RGB(255, 255, 255);
		return Contrast(dark, background) >= Contrast(light, background) ? dark : light;
	}

	bool FontExists(const CString& family)
	{
		if (family.IsEmpty())
			return false;

		LOGFONT lf = { 0 };
		lf.lfCharSet = DEFAULT_CHARSET;
		_tcsncpy_s(lf.lfFaceName, family, _TRUNCATE);

		HDC dc = GetDC(NULL);
		if (dc == NULL)
			return false;

		bool found = false;
		EnumFontFamiliesEx(dc, &lf, [](const LOGFONT*, const TEXTMETRIC*, DWORD, LPARAM data) -> int
		{
			*reinterpret_cast<bool*>(data) = true;
			return 0;
		}, reinterpret_cast<LPARAM>(&found), 0);
		ReleaseDC(NULL, dc);
		return found;
	}

	CString ResolveFamily(const CString& requested, const CString* fallbacks, int count)
	{
		if (!requested.IsEmpty() && FontExists(requested))
			return requested;

		for (int i = 0; i < count; ++i)
		{
			if (FontExists(fallbacks[i]))
				return fallbacks[i];
		}

		return _T("Arial");
	}

	COLORREF SemanticColor(const CTheme& source, LPCTSTR token, COLORREF value, COLORREF fallback)
	{
		if (source.UsesSemanticDefaults() || source.IsSemanticTokenExplicit(token))
			return value;
		return fallback;
	}
}

CUiTheme::CUiTheme()
{
	m_bDark = false;
	m_bHighContrast = false;
	Resolve();
}

CUiTheme::~CUiTheme()
{
}

CUiTheme& CUiTheme::Instance()
{
	static CUiTheme theme;
	return theme;
}

void CUiTheme::OnThemeLoaded(const CTheme& theme)
{
	Instance().Refresh(theme);
}

void CUiTheme::Refresh(const CTheme& theme)
{
	m_source = theme;
	Resolve();
	UiDraw::PurgeCaches();
}

void CUiTheme::Refresh()
{
	Resolve();
	UiDraw::PurgeCaches();
}

void CUiTheme::OnDpiChanged()
{
	UiDraw::PurgeCaches();
}

void CUiTheme::OnSystemThemeChanged()
{
	CDarkMode::RefreshSystemTheme();
	Refresh();
}

void CUiTheme::Resolve()
{
	m_bHighContrast = CDarkMode::IsHighContrast();

	COLORREF legacyCanvas = m_source.MainWindowBG();
	m_canvas = SemanticColor(m_source, _T("Canvas"), m_source.Canvas(), legacyCanvas);
	m_bDark = IsDarkColor(m_canvas);

	COLORREF surfaceFallback = m_source.ListBoxOddRowsBG();
	if (surfaceFallback == m_canvas)
		surfaceFallback = m_bDark ? RGB(36, 38, 42) : RGB(255, 255, 255);
	m_surface = SemanticColor(m_source, _T("Surface"), m_source.Surface(), surfaceFallback);
	m_textPrimary = SemanticColor(m_source, _T("TextPrimary"), m_source.TextPrimary(),
		m_source.ListBoxEvenRowsText());

	COLORREF surfaceAltFallback = m_source.ListBoxEvenRowsBG();
	if (surfaceAltFallback == m_surface || surfaceAltFallback == m_canvas)
		surfaceAltFallback = Blend(m_textPrimary, m_surface, 4);
	m_surfaceAlt = SemanticColor(m_source, _T("SurfaceAlt"), m_source.SurfaceAlt(), surfaceAltFallback);
	m_surfaceSunken = SemanticColor(m_source, _T("SurfaceSunken"), m_source.SurfaceSunken(),
		Blend(m_canvas, m_surface, 35));
	m_border = m_source.Border();
	m_borderStrong = SemanticColor(m_source, _T("BorderStrong"), m_source.BorderStrong(),
		Blend(m_textPrimary, m_border, 18));
	m_textSecondary = SemanticColor(m_source, _T("TextSecondary"), m_source.TextSecondary(),
		m_source.GroupTreeText());
	m_textDisabled = SemanticColor(m_source, _T("TextDisabled"), m_source.TextDisabled(),
		Blend(m_textSecondary, m_canvas, 55));

	COLORREF defaultAccent = m_bDark ? RGB(108, 155, 255) : RGB(37, 99, 235);
	COLORREF accentFallback = m_source.HasWindowsAccent() ? m_source.WindowsAccent() : defaultAccent;
	m_accent = SemanticColor(m_source, _T("Accent"), m_source.Accent(), accentFallback);
	if (m_source.HasWindowsAccent())
		m_accent = m_source.WindowsAccent();

	COLORREF toward = m_bDark ? RGB(255, 255, 255) : RGB(0, 0, 0);
	m_accentHover = SemanticColor(m_source, _T("AccentHover"), m_source.AccentHover(),
		Blend(toward, m_accent, 15));
	m_accentPressed = SemanticColor(m_source, _T("AccentPressed"), m_source.AccentPressed(),
		Blend(toward, m_accent, 28));
	m_accentFill = SemanticColor(m_source, _T("AccentFill"), m_source.AccentFill(), m_accent);
	m_accentFillHover = SemanticColor(m_source, _T("AccentFillHover"), m_source.AccentFillHover(),
		Blend(toward, m_accentFill, 15));
	m_accentFillPressed = SemanticColor(m_source, _T("AccentFillPressed"), m_source.AccentFillPressed(),
		Blend(toward, m_accentFill, 28));
	m_accentOn = SemanticColor(m_source, _T("AccentOn"), m_source.AccentOn(), BestTextOn(m_accentFill));
	if (Contrast(m_accentOn, m_accentFill) < 4.5)
	{
		m_accentFill = m_accent;
		m_accentOn = BestTextOn(m_accentFill);
	}

	m_selection = SemanticColor(m_source, _T("Selection"), m_source.Selection(),
		m_source.ListBoxSelectedBG());
	m_selectionText = SemanticColor(m_source, _T("SelectionText"), m_source.SelectionText(),
		m_source.ListBoxSelectedText());
	m_hover = SemanticColor(m_source, _T("Hover"), m_source.Hover(),
		Blend(m_textPrimary, m_surface, 4));
	m_focusRing = SemanticColor(m_source, _T("FocusRing"), m_source.FocusRing(), m_accent);
	m_danger = SemanticColor(m_source, _T("Danger"), m_source.Danger(),
		m_bDark ? RGB(255, 107, 107) : RGB(196, 60, 60));
	m_dangerFill = SemanticColor(m_source, _T("DangerFill"), m_source.DangerFill(), m_danger);
	m_dangerOn = SemanticColor(m_source, _T("DangerOn"), m_source.DangerOn(), BestTextOn(m_dangerFill));
	m_pinned = SemanticColor(m_source, _T("Pinned"), m_source.Pinned(),
		m_bDark ? RGB(224, 169, 74) : RGB(138, 90, 0));
	m_pasted = SemanticColor(m_source, _T("Pasted"), m_source.Pasted(), m_source.ClipPastedColor());
	m_highlightBg = SemanticColor(m_source, _T("HighlightBg"), m_source.HighlightBg(), m_selection);
	m_highlightText = SemanticColor(m_source, _T("HighlightText"), m_source.HighlightText(),
		m_source.SearchTextHighlight());
	m_closeHoverBg = SemanticColor(m_source, _T("CloseHoverBg"), m_source.CloseHoverBg(), RGB(196, 43, 28));
	m_closeHoverOn = SemanticColor(m_source, _T("CloseHoverOn"), m_source.CloseHoverOn(), RGB(255, 255, 255));

	m_scrollBarTrack = SemanticColor(m_source, _T("ScrollBarTrack"), m_source.ScrollBarTrack(),
		m_canvas);
	m_scrollBarThumb = SemanticColor(m_source, _T("ScrollBarThumb"), m_source.ScrollBarThumb(),
		m_bDark ? RGB(78, 83, 92) : RGB(196, 201, 210));
	m_scrollBarThumbHover = SemanticColor(m_source, _T("ScrollBarThumbHover"), m_source.ScrollBarThumbHover(),
		m_bDark ? RGB(108, 114, 124) : RGB(154, 161, 172));

	m_captionLeft = m_source.CaptionLeft();
	m_captionRight = m_source.CaptionRight();
	m_captionLeftTopMost = m_source.CaptionLeftTopMost();
	m_captionRightTopMost = m_source.CaptionRightTopMost();
	m_captionLeftNotConnected = m_source.CaptionLeftNotConnected();
	m_captionRightNotConnected = m_source.CaptionRightNotConnected();
	m_captionText = m_source.CaptionTextColor();
	m_descriptionWindowBg = m_source.DescriptionWindowBG();
	m_descriptionWindowText = m_source.DescriptionWindowText();
	m_groupTreeBg = m_source.GroupTreeBG();
	m_groupTreeText = m_source.GroupTreeText();
	m_searchTextBoxBg = m_source.SearchTextBoxFocusBG();
	m_searchTextBoxText = m_source.SearchTextBoxFocusText();
	m_searchTextBoxBorder = m_source.SearchTextBoxFocusBorder();

	CString uiFallbacks[] = { _T("Segoe UI Variable Text"), _T("Segoe UI"), _T("Tahoma"), _T("Arial") };
	CString monoFallbacks[] = { _T("Cascadia Mono"), _T("Consolas"), _T("Courier New") };
	m_fontFamilyUi = ResolveFamily(m_source.FontFamilyUi(), uiFallbacks, _countof(uiFallbacks));
	m_fontFamilyMono = ResolveFamily(m_source.FontFamilyMono(), monoFallbacks, _countof(monoFallbacks));

	if (m_bHighContrast)
		ApplyHighContrast();
}

void CUiTheme::ApplyHighContrast()
{
	m_bDark = false;
	m_canvas = GetSysColor(COLOR_WINDOW);
	m_surface = GetSysColor(COLOR_WINDOW);
	m_surfaceAlt = GetSysColor(COLOR_3DFACE);
	m_surfaceSunken = GetSysColor(COLOR_WINDOW);
	m_border = GetSysColor(COLOR_WINDOWFRAME);
	m_borderStrong = GetSysColor(COLOR_WINDOWFRAME);
	m_textPrimary = GetSysColor(COLOR_WINDOWTEXT);
	m_textSecondary = GetSysColor(COLOR_WINDOWTEXT);
	m_textDisabled = GetSysColor(COLOR_GRAYTEXT);
	m_accent = GetSysColor(COLOR_HIGHLIGHT);
	m_accentHover = GetSysColor(COLOR_HIGHLIGHT);
	m_accentPressed = GetSysColor(COLOR_HIGHLIGHT);
	m_accentFill = GetSysColor(COLOR_HIGHLIGHT);
	m_accentFillHover = GetSysColor(COLOR_HIGHLIGHT);
	m_accentFillPressed = GetSysColor(COLOR_HIGHLIGHT);
	m_accentOn = GetSysColor(COLOR_HIGHLIGHTTEXT);
	m_selection = GetSysColor(COLOR_HIGHLIGHT);
	m_selectionText = GetSysColor(COLOR_HIGHLIGHTTEXT);
	m_hover = GetSysColor(COLOR_WINDOW);
	m_focusRing = GetSysColor(COLOR_HIGHLIGHT);
	m_danger = GetSysColor(COLOR_WINDOWTEXT);
	m_dangerFill = GetSysColor(COLOR_HIGHLIGHT);
	m_dangerOn = GetSysColor(COLOR_HIGHLIGHTTEXT);
	m_pinned = GetSysColor(COLOR_WINDOWTEXT);
	m_pasted = GetSysColor(COLOR_WINDOWTEXT);
	m_highlightBg = GetSysColor(COLOR_HIGHLIGHT);
	m_highlightText = GetSysColor(COLOR_HIGHLIGHTTEXT);
	m_closeHoverBg = GetSysColor(COLOR_HIGHLIGHT);
	m_closeHoverOn = GetSysColor(COLOR_HIGHLIGHTTEXT);
	m_scrollBarTrack = GetSysColor(COLOR_WINDOW);
	m_scrollBarThumb = GetSysColor(COLOR_WINDOWTEXT);
	m_scrollBarThumbHover = GetSysColor(COLOR_HIGHLIGHT);

	m_captionLeft = GetSysColor(COLOR_3DFACE);
	m_captionRight = GetSysColor(COLOR_3DFACE);
	m_captionLeftTopMost = GetSysColor(COLOR_3DFACE);
	m_captionRightTopMost = GetSysColor(COLOR_3DFACE);
	m_captionLeftNotConnected = GetSysColor(COLOR_3DFACE);
	m_captionRightNotConnected = GetSysColor(COLOR_3DFACE);
	m_captionText = GetSysColor(COLOR_WINDOWTEXT);
	m_descriptionWindowBg = GetSysColor(COLOR_WINDOW);
	m_descriptionWindowText = GetSysColor(COLOR_WINDOWTEXT);
	m_groupTreeBg = GetSysColor(COLOR_WINDOW);
	m_groupTreeText = GetSysColor(COLOR_WINDOWTEXT);
	m_searchTextBoxBg = GetSysColor(COLOR_WINDOW);
	m_searchTextBoxText = GetSysColor(COLOR_WINDOWTEXT);
	m_searchTextBoxBorder = GetSysColor(COLOR_HIGHLIGHT);
}

bool CUiTheme::UsesLegacyZebra() const
{
	return !m_bHighContrast &&
		m_source.ListBoxOddRowsBG() != m_source.ListBoxEvenRowsBG();
}

COLORREF CUiTheme::ListRowBackground(int rowIndex) const
{
	if (m_bHighContrast)
		return m_surface;

	if (UsesLegacyZebra())
	{
		return (rowIndex % 2 == 0) ?
			m_source.ListBoxOddRowsBG() : m_source.ListBoxEvenRowsBG();
	}

	return m_surface;
}

int CUiTheme::CaptionHeight() const
{
	if (m_source.UsesSemanticDefaults() || m_source.IsSemanticTokenExplicit(_T("CaptionHeight")))
		return m_source.CaptionHeight();
	return m_source.GetCaptionSize();
}
