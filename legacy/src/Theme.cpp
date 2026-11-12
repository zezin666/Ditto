#include "stdafx.h"
#include ".\theme.h"
#include "UiTheme.h"
#include "..\Shared\TextConvert.h"
#include "Misc.h"
#include "Options.h"
#include "..\Shared\Tokenizer.h"
#include "CP_Main.h"
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
	double ColorLuminance(COLORREF color)
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

	double ContrastRatio(COLORREF first, COLORREF second)
	{
		double a = ColorLuminance(first);
		double b = ColorLuminance(second);
		if (a < b)
			std::swap(a, b);
		return (a + 0.05) / (b + 0.05);
	}

	bool IsDarkColor(COLORREF color)
	{
		return ColorLuminance(color) < 0.18;
	}

	BYTE BlendChannel(BYTE foreground, BYTE background, int percentForeground)
	{
		percentForeground = std::max(0, std::min(100, percentForeground));
		return static_cast<BYTE>((foreground * percentForeground +
			background * (100 - percentForeground) + 50) / 100);
	}

	COLORREF BlendColor(COLORREF foreground, COLORREF background, int percentForeground)
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
		return ContrastRatio(dark, background) >= ContrastRatio(light, background) ? dark : light;
	}
}

CTheme::CTheme(void)
{
	m_lFileVersion = 0;
	m_LastWriteTime = 0;
	m_lastTheme = _T("");
	m_windowsAccent = RGB(37, 99, 235);
	m_hasWindowsAccent = false;
	m_bUsingSemanticDefaults = true;

	LoadDefaults();
}

CTheme::~CTheme(void)
{
}


void CTheme::LoadDefaults()
{
	m_bUsingSemanticDefaults = true;
	m_loadedSemanticTokens.clear();
	m_hasWindowsAccent = false;
	m_windowsAccent = RGB(37, 99, 235);

	m_canvas = RGB(247, 248, 250);
	m_surface = RGB(255, 255, 255);
	m_surfaceAlt = RGB(241, 243, 246);
	m_surfaceSunken = RGB(238, 241, 245);
	m_borderStrong = RGB(207, 211, 219);
	m_textPrimary = RGB(27, 31, 36);
	m_textSecondary = RGB(92, 100, 112);
	m_textDisabled = RGB(126, 134, 146);
	m_accent = RGB(37, 99, 235);
	m_accentHover = RGB(29, 78, 216);
	m_accentPressed = RGB(30, 64, 175);
	m_accentFill = RGB(37, 99, 235);
	m_accentFillHover = RGB(29, 78, 216);
	m_accentFillPressed = RGB(30, 64, 175);
	m_accentOn = RGB(255, 255, 255);
	m_selection = RGB(232, 240, 255);
	m_selectionText = RGB(27, 31, 36);
	m_hover = RGB(240, 243, 248);
	m_focusRing = RGB(37, 99, 235);
	m_danger = RGB(196, 60, 60);
	m_dangerFill = RGB(196, 60, 60);
	m_dangerOn = RGB(255, 255, 255);
	m_pinned = RGB(138, 90, 0);
	m_pasted = RGB(31, 122, 77);
	m_highlightBg = RGB(232, 240, 255);
	m_highlightText = RGB(27, 31, 36);
	m_closeHoverBg = RGB(196, 43, 28);
	m_closeHoverOn = RGB(255, 255, 255);

	m_CaptionLeft = m_surfaceAlt;
	m_CaptionRight = m_surfaceAlt;

	m_Border = RGB(227, 230, 236);
	m_BorderTopMost = m_Border;
	m_BorderNotConnected = m_Border;

	m_CaptionLeftTopMost = m_surfaceAlt;
	m_CaptionRightTopMost = m_surfaceAlt;
	
	m_CaptionLeftNotConnected = m_surfaceAlt;
	m_CaptionRightNotConnected = m_surfaceAlt;

	m_CaptionTextColor = m_textPrimary;
	m_ListBoxOddRowsBG = m_surface;
	m_ListBoxEvenRowsBG = m_surface;
	m_ListBoxOddRowsText = m_textPrimary;
	m_ListBoxEvenRowsText = m_textPrimary;
	m_ListBoxSelectedBG = m_selection;
	m_ListBoxSelectedNoFocusBG = m_selection;
	m_ListBoxSelectedText = m_selectionText;
	m_ListBoxSelectedNoFocusText = m_selectionText;
	m_clipPastedColor = m_pasted;
	m_listSmallQuickPasteIndexColor = m_textSecondary;
	m_mainWindowBG = m_canvas;
	m_searchTextBoxFocusBG = m_surface;
	m_searchTextBoxFocusText = m_textPrimary;
	m_searchTextBoxFocusBorder = m_focusRing;
	m_searchTextHighlight = m_highlightText;

	m_groupTreeBG = m_canvas;
	m_groupTreeText = m_textSecondary;

	m_descriptionWindowBG = m_canvas;
	m_descriptionWindowText = m_textPrimary;

	m_scrollBarThumb = RGB(196, 201, 210);
	m_scrollBarThumbHover = RGB(154, 161, 172);
	m_scrollBarTrack = m_canvas;

	m_radiusWindow = 12;
	m_radiusPanel = 10;
	m_radiusControl = 8;
	m_radiusListItem = 6;
	m_borderWidth = 1;
	m_focusRingWidth = 2;
	m_captionHeight = 34;
	m_captionButtonSize = 28;
	m_captionIconSize = 16;
	m_searchHeight = 34;
	m_bottomBarHeight = 40;
	m_bottomBarPadding = 6;
	m_listRowHeight = 30;
	m_listRowInsetX = 8;
	m_listRowInsetY = 1;
	m_listIndexColumnWidth = 18;
	m_listIconSize = 16;
	m_iconSizeSmall = 16;
	m_iconSizeMedium = 20;
	m_iconSizeLarge = 24;
	m_scrollBarThumbWidth = 6;
	m_scrollBarThumbHoverWidth = 10;
	m_scrollBarInset = 2;
	m_scrollBarRadius = 3;
	m_scrollBarMinThumb = 24;
	m_tooltipRadius = 10;
	m_tooltipPadding = 12;
	m_dialogPadding = 16;
	m_fieldGap = 8;
	m_sectionGap = 16;
	m_buttonHeight = 30;
	m_buttonMinWidth = 76;
	m_buttonPaddingX = 14;
	m_buttonRadius = 6;
	m_inputHeight = 30;
	m_inputRadius = 6;
	m_inputPaddingX = 8;
	m_tabHeight = 36;
	m_tabPaddingX = 12;
	m_tabUnderlineHeight = 2;
	m_menuItemHeight = 28;
	m_menuPaddingX = 12;
	m_menuGutterWidth = 28;
	m_emptyStateMaxWidth = 240;
	m_fontSizeBody = 13;
	m_fontSizeSecondary = 12;
	m_fontSizeCaption = 11;
	m_fontSizeTitle = 14;
	m_fontSizeWindowTitle = 14;
	m_fontSizeDialogBody = 12;
	m_iconStrokeTenths = 15;
	m_motionFastMs = 120;
	m_motionStdMs = 160;
	m_scrollBarAutoHideMs = 800;

	m_fontFamilyUi.Empty();
	m_fontFamilyMono.Empty();

	m_captionSize = m_captionHeight;
	m_captionFontSize = m_fontSizeWindowTitle;
}

bool CTheme::Load(CString csTheme, bool bHeaderOnly, bool bCheckLastWriteTime)
{
	bool followWindows10Theme = false;
	if (csTheme.IsEmpty())
	{
		followWindows10Theme = true;

		if (DarkAppWindows10Setting())
		{
			csTheme = _T("DarkerDitto");
			Log(_T("Loading theme based on windows setting of dark mode for apps"));			
		}
	}

	if (csTheme.IsEmpty() || csTheme == _T("Ditto") || csTheme == _T("(Default)") || csTheme == _T("(Ditto)"))
	{
		LoadDefaults();

		if (followWindows10Theme)
		{
			LoadWindowsAccentColor();
		}

		m_LastWriteTime = 0;
		m_lastTheme = _T("");

		Log(_T("Loading default ditto values for themes"));

		CUiTheme::OnThemeLoaded(*this);
		return false;
	}

	CString csPath = CGetSetOptions::GetPath(PATH_THEMES);
	csPath += csTheme;
	csPath += ".xml";

	__int64 LastWrite = GetLastWriteTime(csPath);

	if(bCheckLastWriteTime)
	{	
		if(m_lastTheme == csTheme &&
			LastWrite == m_LastWriteTime)
		{
			return true;
		}
	}

	LoadDefaults();

	m_LastWriteTime = LastWrite;
	m_lastTheme = csTheme;

	Log(StrF(_T("Loading Theme %s"), csPath));

	TiXmlDocument doc;
	if(!doc.LoadFile(csPath.GetBuffer()))
	{
		m_csLastError.Format(_T("Error loading Theme %s - reason = %s"), csPath, doc.ErrorDesc());
		ASSERT(!m_csLastError);
		Log(m_csLastError);
		CUiTheme::OnThemeLoaded(*this);
		return false;
	}

	TiXmlElement *ItemHeader = doc.FirstChildElement("Ditto_Theme_File");
	if(!ItemHeader)
	{
		m_csLastError.Format(_T("Error finding the section Ditto_Theme_File"));
		ASSERT(!m_csLastError);
		Log(m_csLastError);
		CUiTheme::OnThemeLoaded(*this);
		return false;
	}

	CString csVersion = ItemHeader->Attribute("Version");
	m_lFileVersion = ATOI(csVersion);
	m_csAuthor = ItemHeader->Attribute("Author");
	m_csNotes = ItemHeader->Attribute("Notes");

	if(bHeaderOnly)
		return true;

	m_bUsingSemanticDefaults = false;
	m_loadedSemanticTokens.clear();

	LoadColor(ItemHeader, "CaptionLeft", m_CaptionLeft);
	LoadColor(ItemHeader, "CaptionRight", m_CaptionRight);
	LoadColor(ItemHeader, "CaptionLeftTopMost", m_CaptionLeftTopMost);
	LoadColor(ItemHeader, "CaptionRightTopMost", m_CaptionRightTopMost);
	LoadColor(ItemHeader, "CaptionLeftNotConnected", m_CaptionLeftNotConnected);
	LoadColor(ItemHeader, "CaptionRightNotConnected", m_CaptionRightNotConnected);
	LoadColor(ItemHeader, "CaptionTextColor", m_CaptionTextColor);
	LoadColor(ItemHeader, "ListBoxOddRowsBG", m_ListBoxOddRowsBG);
	LoadColor(ItemHeader, "ListBoxEvenRowsBG", m_ListBoxEvenRowsBG);
	LoadColor(ItemHeader, "ListBoxOddRowsText", m_ListBoxOddRowsText);
	LoadColor(ItemHeader, "ListBoxEvenRowsText", m_ListBoxEvenRowsText);
	LoadColor(ItemHeader, "ListBoxSelectedBG", m_ListBoxSelectedBG);
	LoadColor(ItemHeader, "ListBoxSelectedNoFocusBG", m_ListBoxSelectedNoFocusBG);
	LoadColor(ItemHeader, "ListBoxSelectedText", m_ListBoxSelectedText);
	LoadColor(ItemHeader, "ListBoxSelectedNoFocusText", m_ListBoxSelectedNoFocusText);
	LoadColor(ItemHeader, "ClipPastedColor", m_clipPastedColor);
	LoadColor(ItemHeader, "ListSmallQuickPasteIndexColor", m_listSmallQuickPasteIndexColor);
	LoadColor(ItemHeader, "MainWindowBG", m_mainWindowBG);
	LoadColor(ItemHeader, "SearchTextBoxFocusBG", m_searchTextBoxFocusBG);
	LoadColor(ItemHeader, "SearchTextBoxFocusText", m_searchTextBoxFocusText);
	LoadColor(ItemHeader, "SearchTextBoxFocusBorder", m_searchTextBoxFocusBorder);
	LoadColor(ItemHeader, "SearchTextHighlight", m_searchTextHighlight);

	LoadColor(ItemHeader, "Border", m_Border);
	LoadColor(ItemHeader, "BorderTopMost", m_BorderTopMost);
	LoadColor(ItemHeader, "BorderNotConnected", m_BorderNotConnected);

	LoadColor(ItemHeader, "GroupTreeBG", m_groupTreeBG);
	LoadColor(ItemHeader, "GroupTreeText", m_groupTreeText);
	
	LoadInt(ItemHeader, "CaptionSize", m_captionSize);
	LoadInt(ItemHeader, "CaptionFontSize", m_captionFontSize);

	LoadColor(ItemHeader, "DescriptionWindowBG", m_descriptionWindowBG);
	LoadColor(ItemHeader, "DescriptionWindowText", m_descriptionWindowText);

	// Modern scrollbar colors. These are optional for legacy themes.
	LoadSemanticColor(ItemHeader, "ScrollBarThumb", m_scrollBarThumb);
	LoadSemanticColor(ItemHeader, "ScrollBarThumbHover", m_scrollBarThumbHover);
	LoadSemanticColor(ItemHeader, "ScrollBarTrack", m_scrollBarTrack);

	LoadSemanticColor(ItemHeader, "Canvas", m_canvas);
	LoadSemanticColor(ItemHeader, "Surface", m_surface);
	LoadSemanticColor(ItemHeader, "SurfaceAlt", m_surfaceAlt);
	LoadSemanticColor(ItemHeader, "SurfaceSunken", m_surfaceSunken);
	LoadSemanticColor(ItemHeader, "BorderStrong", m_borderStrong);
	LoadSemanticColor(ItemHeader, "TextPrimary", m_textPrimary);
	LoadSemanticColor(ItemHeader, "TextSecondary", m_textSecondary);
	LoadSemanticColor(ItemHeader, "TextDisabled", m_textDisabled);
	LoadSemanticColor(ItemHeader, "Accent", m_accent);
	LoadSemanticColor(ItemHeader, "AccentHover", m_accentHover);
	LoadSemanticColor(ItemHeader, "AccentPressed", m_accentPressed);
	LoadSemanticColor(ItemHeader, "AccentFill", m_accentFill);
	LoadSemanticColor(ItemHeader, "AccentFillHover", m_accentFillHover);
	LoadSemanticColor(ItemHeader, "AccentFillPressed", m_accentFillPressed);
	LoadSemanticColor(ItemHeader, "AccentOn", m_accentOn);
	LoadSemanticColor(ItemHeader, "Selection", m_selection);
	LoadSemanticColor(ItemHeader, "SelectionText", m_selectionText);
	LoadSemanticColor(ItemHeader, "Hover", m_hover);
	LoadSemanticColor(ItemHeader, "FocusRing", m_focusRing);
	LoadSemanticColor(ItemHeader, "Danger", m_danger);
	LoadSemanticColor(ItemHeader, "DangerFill", m_dangerFill);
	LoadSemanticColor(ItemHeader, "DangerOn", m_dangerOn);
	LoadSemanticColor(ItemHeader, "Pinned", m_pinned);
	LoadSemanticColor(ItemHeader, "Pasted", m_pasted);
	LoadSemanticColor(ItemHeader, "HighlightBg", m_highlightBg);
	LoadSemanticColor(ItemHeader, "HighlightText", m_highlightText);
	LoadSemanticColor(ItemHeader, "CloseHoverBg", m_closeHoverBg);
	LoadSemanticColor(ItemHeader, "CloseHoverOn", m_closeHoverOn);

	LoadSemanticInt(ItemHeader, "RadiusWindow", m_radiusWindow);
	LoadSemanticInt(ItemHeader, "RadiusPanel", m_radiusPanel);
	LoadSemanticInt(ItemHeader, "RadiusControl", m_radiusControl);
	LoadSemanticInt(ItemHeader, "RadiusListItem", m_radiusListItem);
	LoadSemanticInt(ItemHeader, "BorderWidth", m_borderWidth);
	LoadSemanticInt(ItemHeader, "FocusRingWidth", m_focusRingWidth);
	LoadSemanticInt(ItemHeader, "CaptionHeight", m_captionHeight);
	LoadSemanticInt(ItemHeader, "CaptionButtonSize", m_captionButtonSize);
	LoadSemanticInt(ItemHeader, "CaptionIconSize", m_captionIconSize);
	LoadSemanticInt(ItemHeader, "SearchHeight", m_searchHeight);
	LoadSemanticInt(ItemHeader, "BottomBarHeight", m_bottomBarHeight);
	LoadSemanticInt(ItemHeader, "BottomBarPadding", m_bottomBarPadding);
	LoadSemanticInt(ItemHeader, "ListRowHeight", m_listRowHeight);
	LoadSemanticInt(ItemHeader, "ListRowInsetX", m_listRowInsetX);
	LoadSemanticInt(ItemHeader, "ListRowInsetY", m_listRowInsetY);
	LoadSemanticInt(ItemHeader, "ListIndexColumnWidth", m_listIndexColumnWidth);
	LoadSemanticInt(ItemHeader, "ListIconSize", m_listIconSize);
	LoadSemanticInt(ItemHeader, "IconSizeSmall", m_iconSizeSmall);
	LoadSemanticInt(ItemHeader, "IconSizeMedium", m_iconSizeMedium);
	LoadSemanticInt(ItemHeader, "IconSizeLarge", m_iconSizeLarge);
	LoadSemanticInt(ItemHeader, "ScrollBarThumbWidth", m_scrollBarThumbWidth);
	LoadSemanticInt(ItemHeader, "ScrollBarThumbHoverWidth", m_scrollBarThumbHoverWidth);
	LoadSemanticInt(ItemHeader, "ScrollBarInset", m_scrollBarInset);
	LoadSemanticInt(ItemHeader, "ScrollBarRadius", m_scrollBarRadius);
	LoadSemanticInt(ItemHeader, "ScrollBarMinThumb", m_scrollBarMinThumb);
	LoadSemanticInt(ItemHeader, "TooltipRadius", m_tooltipRadius);
	LoadSemanticInt(ItemHeader, "TooltipPadding", m_tooltipPadding);
	LoadSemanticInt(ItemHeader, "DialogPadding", m_dialogPadding);
	LoadSemanticInt(ItemHeader, "FieldGap", m_fieldGap);
	LoadSemanticInt(ItemHeader, "SectionGap", m_sectionGap);
	LoadSemanticInt(ItemHeader, "ButtonHeight", m_buttonHeight);
	LoadSemanticInt(ItemHeader, "ButtonMinWidth", m_buttonMinWidth);
	LoadSemanticInt(ItemHeader, "ButtonPaddingX", m_buttonPaddingX);
	LoadSemanticInt(ItemHeader, "ButtonRadius", m_buttonRadius);
	LoadSemanticInt(ItemHeader, "InputHeight", m_inputHeight);
	LoadSemanticInt(ItemHeader, "InputRadius", m_inputRadius);
	LoadSemanticInt(ItemHeader, "InputPaddingX", m_inputPaddingX);
	LoadSemanticInt(ItemHeader, "TabHeight", m_tabHeight);
	LoadSemanticInt(ItemHeader, "TabPaddingX", m_tabPaddingX);
	LoadSemanticInt(ItemHeader, "TabUnderlineHeight", m_tabUnderlineHeight);
	LoadSemanticInt(ItemHeader, "MenuItemHeight", m_menuItemHeight);
	LoadSemanticInt(ItemHeader, "MenuPaddingX", m_menuPaddingX);
	LoadSemanticInt(ItemHeader, "MenuGutterWidth", m_menuGutterWidth);
	LoadSemanticInt(ItemHeader, "EmptyStateMaxWidth", m_emptyStateMaxWidth);
	LoadSemanticInt(ItemHeader, "FontSizeBody", m_fontSizeBody);
	LoadSemanticInt(ItemHeader, "FontSizeSecondary", m_fontSizeSecondary);
	LoadSemanticInt(ItemHeader, "FontSizeCaption", m_fontSizeCaption);
	LoadSemanticInt(ItemHeader, "FontSizeTitle", m_fontSizeTitle);
	LoadSemanticInt(ItemHeader, "FontSizeWindowTitle", m_fontSizeWindowTitle);
	LoadSemanticInt(ItemHeader, "FontSizeDialogBody", m_fontSizeDialogBody);
	LoadSemanticInt(ItemHeader, "IconStrokeTenths", m_iconStrokeTenths);
	LoadSemanticInt(ItemHeader, "MotionFastMs", m_motionFastMs);
	LoadSemanticInt(ItemHeader, "MotionStdMs", m_motionStdMs);
	LoadSemanticInt(ItemHeader, "ScrollBarAutoHideMs", m_scrollBarAutoHideMs);

	LoadSemanticString(ItemHeader, "FontFamilyUi", m_fontFamilyUi);
	LoadSemanticString(ItemHeader, "FontFamilyMono", m_fontFamilyMono);

	if (followWindows10Theme)
	{
		LoadWindowsAccentColor();
	}

	CUiTheme::OnThemeLoaded(*this);
	return true;
}

void CTheme::LoadWindowsAccentColor()
{
	DWORD accent = Windows10AccentColor();
	if (accent == static_cast<DWORD>(-1))
	{
		m_hasWindowsAccent = false;
		return;
	}

	// DWM reports COLORREF-compatible BGR data.
	COLORREF candidate = RGB(GetBValue(accent), GetGValue(accent), GetRValue(accent));
	COLORREF canvas = m_bUsingSemanticDefaults ? m_canvas : m_mainWindowBG;
	COLORREF surface = m_bUsingSemanticDefaults ? m_surface : m_ListBoxOddRowsBG;
	bool dark = IsDarkColor(canvas);
	COLORREF fallback = dark ? RGB(108, 155, 255) : RGB(37, 99, 235);
	COLORREF on = BestTextOn(candidate);

	bool valid = ContrastRatio(candidate, surface) >= 4.5 &&
		ContrastRatio(on, candidate) >= 4.5;

	m_windowsAccent = valid ? candidate : fallback;
	m_hasWindowsAccent = true;

	COLORREF toward = dark ? RGB(255, 255, 255) : RGB(0, 0, 0);
	m_accent = m_windowsAccent;
	m_accentHover = BlendColor(toward, m_accent, 15);
	m_accentPressed = BlendColor(toward, m_accent, 28);
	m_accentFill = m_accent;
	m_accentFillHover = m_accentHover;
	m_accentFillPressed = m_accentPressed;
	m_accentOn = BestTextOn(m_accentFill);
	if (ContrastRatio(m_accentOn, m_accentFill) < 4.5)
	{
		m_accentFill = fallback;
		m_accentFillHover = BlendColor(toward, m_accentFill, 15);
		m_accentFillPressed = BlendColor(toward, m_accentFill, 28);
		m_accentOn = BestTextOn(m_accentFill);
	}

	m_focusRing = m_accent;
	m_searchTextBoxFocusBorder = m_accent;
	m_searchTextHighlight = m_accent;

	if (!valid)
	{
		Log(_T("Windows accent failed contrast validation; using the default accent"));
	}
}

COLORREF HslToRgb(float h, float s, float l)
{
	if (s == 0.0f)
	{
		// Grayscale, achromatic
		BYTE gray = static_cast<BYTE>(l * 255.0f + 0.5f);
		return RGB(gray, gray, gray);
	}

	auto hueToRgb = [](float p, float q, float t) -> float
	{
		if (t < 0.0f) t += 1.0f;
		if (t > 1.0f) t -= 1.0f;
		if (t < 1.0f / 6.0f) return p + (q - p) * 6.0f * t;
		if (t < 1.0f / 2.0f) return q;
		if (t < 2.0f / 3.0f) return p + (q - p) * (2.0f / 3.0f - t) * 6.0f;
		return p;
	};

	float q = l < 0.5f ? l * (1.0f + s) : l + s - l * s;
	float p = 2.0f * l - q;
	float h_norm = h / 360.0f;

	float r_f = hueToRgb(p, q, h_norm + 1.0f / 3.0f);
	float g_f = hueToRgb(p, q, h_norm);
	float b_f = hueToRgb(p, q, h_norm - 1.0f / 3.0f);

	BYTE r = static_cast<BYTE>(r_f * 255.0f + 0.5f);
	BYTE g = static_cast<BYTE>(g_f * 255.0f + 0.5f);
	BYTE b = static_cast<BYTE>(b_f * 255.0f + 0.5f);

	return RGB(r, g, b);
}

bool CTheme::LoadColor(TiXmlElement *pParent, CStringA csNode, COLORREF &Color)
{
	int intValue = 0;
	return LoadElement(pParent, csNode, Color, intValue);
}

bool CTheme::LoadInt(TiXmlElement *pParent, CStringA csNode, int &intValue)
{
	COLORREF colorValue = 0;
	return LoadElement(pParent, csNode, colorValue, intValue);
}

bool CTheme::LoadOptionalColor(TiXmlElement *pParent, CStringA csNode, COLORREF &Color)
{
	int intValue = 0;
	return LoadElement(pParent, csNode, Color, intValue, false);
}

bool CTheme::LoadOptionalInt(TiXmlElement *pParent, CStringA csNode, int &intValue)
{
	COLORREF colorValue = 0;
	return LoadElement(pParent, csNode, colorValue, intValue, false);
}

bool CTheme::LoadOptionalString(TiXmlElement *pParent, CStringA csNode, CString &value)
{
	TiXmlElement *pNode = pParent->FirstChildElement(csNode);
	if (pNode == NULL)
		return false;

	TiXmlNode *pValue = pNode->FirstChild();
	if (pValue == NULL)
		return false;

	CString text = pValue->Value();
	text.Trim();
	if (text.IsEmpty())
		return false;

	value = text;
	return true;
}

bool CTheme::LoadSemanticColor(TiXmlElement *pParent, CStringA csNode, COLORREF &Color)
{
	if (!LoadOptionalColor(pParent, csNode, Color))
		return false;

	m_loadedSemanticTokens.insert(CString(csNode));
	return true;
}

bool CTheme::LoadSemanticInt(TiXmlElement *pParent, CStringA csNode, int &intValue)
{
	if (!LoadOptionalInt(pParent, csNode, intValue))
		return false;

	m_loadedSemanticTokens.insert(CString(csNode));
	return true;
}

bool CTheme::LoadSemanticString(TiXmlElement *pParent, CStringA csNode, CString &value)
{
	if (!LoadOptionalString(pParent, csNode, value))
		return false;

	m_loadedSemanticTokens.insert(CString(csNode));
	return true;
}

bool CTheme::IsSemanticTokenExplicit(LPCTSTR psToken) const
{
	return m_loadedSemanticTokens.find(CString(psToken)) != m_loadedSemanticTokens.end();
}

bool CTheme::LoadElement(TiXmlElement *pParent, CStringA csNode, COLORREF &Color, int &intValue, bool bLogErrors)
{
	CString csNodeName(csNode);
	TiXmlElement *pColorNode = pParent->FirstChildElement(csNode);
	if(pColorNode == NULL)
	{
		if (bLogErrors)
		{
			m_csLastError.Format(_T("Theme Load, error loading Node = %s"), csNodeName.GetString());
			Log(m_csLastError);
		}
		return false;
	}

	TiXmlNode *pColor = pColorNode->FirstChild();
	if(pColor == NULL)
	{
		if (bLogErrors)
		{
			m_csLastError.Format(_T("Theme Load, error getting node text for = %s"), csNodeName.GetString());
			Log(m_csLastError);
		}
		return false;
	}
	
	CString csColor = pColor->Value();
	csColor.Trim();

	if (csColor.IsEmpty())
	{
		return false;
	}

	if (csColor.GetLength() > 4 && csColor.Left(4).CompareNoCase(_T("rgb(")) == 0)
	{
		CString values = csColor.Mid(4, csColor.GetLength() - 5);
		values.Trim();

		CTokenizer token(values, _T(", "));
		CString csR, csG, csB;

		token.Next(csR);
		token.Next(csG);
		token.Next(csB);

		csR.Trim();
		csG.Trim();
		csB.Trim();

		if (!csR.IsEmpty() && csG.IsEmpty() && csB.IsEmpty())
		{
			Color = ATOI(csR);
		}
		else if (!csR.IsEmpty() && !csG.IsEmpty() && !csB.IsEmpty())
		{
			Color = RGB(ATOI(csR), ATOI(csG), ATOI(csB));
		}
		else
		{
			if (bLogErrors)
			{
				m_csLastError.Format(_T("Theme Load, malformed/incomplete RGB value for Node = %s, Value = %s"), csNodeName.GetString(), csColor.GetString());
				Log(m_csLastError);
			}
			return false;
		}
	}
	else if (csColor.GetLength() > 4 && csColor.Left(4).CompareNoCase(_T("hsl(")) == 0)
	{
		CString values = csColor.Mid(4, csColor.GetLength() - 5);
		values.Trim();

		CTokenizer token(values, _T(", %"));
		CString csH, csS, csL;

		token.Next(csH);
		token.Next(csS);
		token.Next(csL);

		csH.Trim();
		csS.Trim();
		csL.Trim();

		if (!csH.IsEmpty() && !csS.IsEmpty() && !csL.IsEmpty())
		{
			float h = (float)_tstof(csH);
			float s = (float)_tstof(csS);
			float l = (float)_tstof(csL);

			s = max(0.0f, min(100.0f, s)) / 100.0f;
			l = max(0.0f, min(100.0f, l)) / 100.0f;

			Color = HslToRgb(h, s, l);
		}
		else
		{
			if (bLogErrors)
			{
				m_csLastError.Format(_T("Theme Load, malformed/incomplete HSL value for Node = %s, Value = %s"), csNodeName.GetString(), csColor.GetString());
				Log(m_csLastError);
			}
			return false;
		}
	}
	else if (csColor.GetAt(0) == _T('#') && csColor.GetLength() == 7)
	{
		long r = _tcstol(csColor.Mid(1, 2), NULL, 16);
		long g = _tcstol(csColor.Mid(3, 2), NULL, 16);
		long b = _tcstol(csColor.Mid(5, 2), NULL, 16);

		Color = RGB(r, g, b);
	}
	else
	{
		intValue = ATOI(csColor);
		Color = (COLORREF)intValue;
	}

	return true;
}
