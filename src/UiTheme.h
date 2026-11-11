#pragma once

#include "Theme.h"

// UI-thread semantic theme facade. It wraps CTheme so legacy call sites keep
// working while new controls consume resolved, accessibility-safe tokens.
class CUiTheme
{
public:
	static CUiTheme& Instance();
	static void OnThemeLoaded(const CTheme& theme);

	void Refresh(const CTheme& theme);
	void Refresh();
	void OnDpiChanged();
	void OnSystemThemeChanged();

	const CTheme& Source() const { return m_source; }
	bool IsDark() const { return m_bDark; }
	bool IsHighContrast() const { return m_bHighContrast; }

	COLORREF Canvas() const { return m_canvas; }
	COLORREF Surface() const { return m_surface; }
	COLORREF SurfaceAlt() const { return m_surfaceAlt; }
	COLORREF SurfaceSunken() const { return m_surfaceSunken; }
	COLORREF Border() const { return m_border; }
	COLORREF BorderStrong() const { return m_borderStrong; }
	COLORREF TextPrimary() const { return m_textPrimary; }
	COLORREF TextSecondary() const { return m_textSecondary; }
	COLORREF TextDisabled() const { return m_textDisabled; }
	COLORREF Accent() const { return m_accent; }
	COLORREF AccentHover() const { return m_accentHover; }
	COLORREF AccentPressed() const { return m_accentPressed; }
	COLORREF AccentFill() const { return m_accentFill; }
	COLORREF AccentFillHover() const { return m_accentFillHover; }
	COLORREF AccentFillPressed() const { return m_accentFillPressed; }
	COLORREF AccentOn() const { return m_accentOn; }
	COLORREF Selection() const { return m_selection; }
	COLORREF SelectionText() const { return m_selectionText; }
	COLORREF Hover() const { return m_hover; }
	COLORREF FocusRing() const { return m_focusRing; }
	COLORREF Danger() const { return m_danger; }
	COLORREF DangerFill() const { return m_dangerFill; }
	COLORREF DangerOn() const { return m_dangerOn; }
	COLORREF Pinned() const { return m_pinned; }
	COLORREF Pasted() const { return m_pasted; }
	COLORREF HighlightBg() const { return m_highlightBg; }
	COLORREF HighlightText() const { return m_highlightText; }
	COLORREF CloseHoverBg() const { return m_closeHoverBg; }
	COLORREF CloseHoverOn() const { return m_closeHoverOn; }
	COLORREF ScrollBarTrack() const { return m_scrollBarTrack; }
	COLORREF ScrollBarThumb() const { return m_scrollBarThumb; }
	COLORREF ScrollBarThumbHover() const { return m_scrollBarThumbHover; }

	COLORREF CaptionLeft() const { return m_captionLeft; }
	COLORREF CaptionRight() const { return m_captionRight; }
	COLORREF CaptionLeftTopMost() const { return m_captionLeftTopMost; }
	COLORREF CaptionRightTopMost() const { return m_captionRightTopMost; }
	COLORREF CaptionLeftNotConnected() const { return m_captionLeftNotConnected; }
	COLORREF CaptionRightNotConnected() const { return m_captionRightNotConnected; }
	COLORREF CaptionText() const { return m_captionText; }
	COLORREF DescriptionWindowBg() const { return m_descriptionWindowBg; }
	COLORREF DescriptionWindowText() const { return m_descriptionWindowText; }
	COLORREF GroupTreeBg() const { return m_groupTreeBg; }
	COLORREF GroupTreeText() const { return m_groupTreeText; }
	COLORREF SearchTextBoxBg() const { return m_searchTextBoxBg; }
	COLORREF SearchTextBoxText() const { return m_searchTextBoxText; }
	COLORREF SearchTextBoxBorder() const { return m_searchTextBoxBorder; }

	bool UsesLegacyZebra() const;
	COLORREF ListRowBackground(int rowIndex) const;

	int RadiusWindow() const { return m_source.RadiusWindow(); }
	int RadiusPanel() const { return m_source.RadiusPanel(); }
	int RadiusControl() const { return m_source.RadiusControl(); }
	int RadiusListItem() const { return m_source.RadiusListItem(); }
	int BorderWidth() const { return 1; }
	int FocusRingWidth() const { return m_source.FocusRingWidth(); }
	int CaptionHeight() const;
	int CaptionButtonSize() const { return m_source.CaptionButtonSize(); }
	int CaptionIconSize() const { return m_source.CaptionIconSize(); }
	int SearchHeight() const { return m_source.SearchHeight(); }
	int BottomBarHeight() const { return m_source.BottomBarHeight(); }
	int BottomBarPadding() const { return m_source.BottomBarPadding(); }
	int ListRowHeight() const { return m_source.ListRowHeight(); }
	int ListRowInsetX() const { return m_source.ListRowInsetX(); }
	int ListRowInsetY() const { return m_source.ListRowInsetY(); }
	int ListIndexColumnWidth() const { return m_source.ListIndexColumnWidth(); }
	int ListIconSize() const { return m_source.ListIconSize(); }
	int IconSizeSmall() const { return m_source.IconSizeSmall(); }
	int IconSizeMedium() const { return m_source.IconSizeMedium(); }
	int IconSizeLarge() const { return m_source.IconSizeLarge(); }
	int ScrollBarThumbWidth() const { return m_source.ScrollBarThumbWidth(); }
	int ScrollBarThumbHoverWidth() const { return m_source.ScrollBarThumbHoverWidth(); }
	int ScrollBarInset() const { return m_source.ScrollBarInset(); }
	int ScrollBarRadius() const { return m_source.ScrollBarRadius(); }
	int ScrollBarMinThumb() const { return m_source.ScrollBarMinThumb(); }
	int TooltipRadius() const { return m_source.TooltipRadius(); }
	int TooltipPadding() const { return m_source.TooltipPadding(); }
	int DialogPadding() const { return m_source.DialogPadding(); }
	int FieldGap() const { return m_source.FieldGap(); }
	int SectionGap() const { return m_source.SectionGap(); }
	int ButtonHeight() const { return m_source.ButtonHeight(); }
	int ButtonMinWidth() const { return m_source.ButtonMinWidth(); }
	int ButtonPaddingX() const { return m_source.ButtonPaddingX(); }
	int ButtonRadius() const { return m_source.ButtonRadius(); }
	int InputHeight() const { return m_source.InputHeight(); }
	int InputRadius() const { return m_source.InputRadius(); }
	int InputPaddingX() const { return m_source.InputPaddingX(); }
	int TabHeight() const { return m_source.TabHeight(); }
	int TabPaddingX() const { return m_source.TabPaddingX(); }
	int TabUnderlineHeight() const { return m_source.TabUnderlineHeight(); }
	int MenuItemHeight() const { return m_source.MenuItemHeight(); }
	int MenuPaddingX() const { return m_source.MenuPaddingX(); }
	int MenuGutterWidth() const { return m_source.MenuGutterWidth(); }
	int EmptyStateMaxWidth() const { return m_source.EmptyStateMaxWidth(); }
	int FontSizeBody() const { return m_source.FontSizeBody(); }
	int FontSizeSecondary() const { return m_source.FontSizeSecondary(); }
	int FontSizeCaption() const { return m_source.FontSizeCaption(); }
	int FontSizeTitle() const { return m_source.FontSizeTitle(); }
	int FontSizeWindowTitle() const { return m_source.FontSizeWindowTitle(); }
	int FontSizeDialogBody() const { return m_source.FontSizeDialogBody(); }
	int IconStrokeTenths() const { return m_source.IconStrokeTenths(); }
	int MotionFastMs() const { return m_source.MotionFastMs(); }
	int MotionStdMs() const { return m_source.MotionStdMs(); }
	int ScrollBarAutoHideMs() const { return m_source.ScrollBarAutoHideMs(); }

	CString FontFamilyUi() const { return m_fontFamilyUi; }
	CString FontFamilyMono() const { return m_fontFamilyMono; }

private:
	CUiTheme();
	~CUiTheme();
	CUiTheme(const CUiTheme&);
	CUiTheme& operator=(const CUiTheme&);

	void Resolve();
	void ApplyHighContrast();

	CTheme m_source;
	bool m_bDark;
	bool m_bHighContrast;

	COLORREF m_canvas;
	COLORREF m_surface;
	COLORREF m_surfaceAlt;
	COLORREF m_surfaceSunken;
	COLORREF m_border;
	COLORREF m_borderStrong;
	COLORREF m_textPrimary;
	COLORREF m_textSecondary;
	COLORREF m_textDisabled;
	COLORREF m_accent;
	COLORREF m_accentHover;
	COLORREF m_accentPressed;
	COLORREF m_accentFill;
	COLORREF m_accentFillHover;
	COLORREF m_accentFillPressed;
	COLORREF m_accentOn;
	COLORREF m_selection;
	COLORREF m_selectionText;
	COLORREF m_hover;
	COLORREF m_focusRing;
	COLORREF m_danger;
	COLORREF m_dangerFill;
	COLORREF m_dangerOn;
	COLORREF m_pinned;
	COLORREF m_pasted;
	COLORREF m_highlightBg;
	COLORREF m_highlightText;
	COLORREF m_closeHoverBg;
	COLORREF m_closeHoverOn;
	COLORREF m_scrollBarTrack;
	COLORREF m_scrollBarThumb;
	COLORREF m_scrollBarThumbHover;

	COLORREF m_captionLeft;
	COLORREF m_captionRight;
	COLORREF m_captionLeftTopMost;
	COLORREF m_captionRightTopMost;
	COLORREF m_captionLeftNotConnected;
	COLORREF m_captionRightNotConnected;
	COLORREF m_captionText;
	COLORREF m_descriptionWindowBg;
	COLORREF m_descriptionWindowText;
	COLORREF m_groupTreeBg;
	COLORREF m_groupTreeText;
	COLORREF m_searchTextBoxBg;
	COLORREF m_searchTextBoxText;
	COLORREF m_searchTextBoxBorder;

	CString m_fontFamilyUi;
	CString m_fontFamilyMono;
};

inline CUiTheme& UiTheme()
{
	return CUiTheme::Instance();
}
