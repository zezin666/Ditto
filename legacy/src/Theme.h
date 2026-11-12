#pragma once

#include <set>
#include "tinyxml\Tinyxml.h"
#include "tinyxml\tinystr.h"

class CTheme
{
public:
	CTheme(void);
	~CTheme(void);

	bool Load(CString csTheme, bool bHeaderOnly = false, bool bCheckLastWriteTime = false);	

	COLORREF CaptionLeft() const { return m_CaptionLeft; }
	COLORREF CaptionRight() const { return m_CaptionRight; }
	COLORREF Border() const { return m_Border; }
	COLORREF BorderTopMost() const { return m_BorderTopMost; }
	COLORREF BorderNotConnected() const { return m_BorderNotConnected; }
	COLORREF CaptionLeftTopMost() const { return m_CaptionLeftTopMost; }
	COLORREF CaptionRightTopMost() const { return m_CaptionRightTopMost; }
	COLORREF CaptionLeftNotConnected() const { return m_CaptionLeftNotConnected; }
	COLORREF CaptionRightNotConnected() const { return m_CaptionRightNotConnected; }
	COLORREF CaptionTextColor() const { return m_CaptionTextColor; }
	
	COLORREF ListBoxOddRowsBG() const { return m_ListBoxOddRowsBG; }
	COLORREF ListBoxEvenRowsBG() const { return m_ListBoxEvenRowsBG; }
	COLORREF ListBoxOddRowsText() const { return m_ListBoxOddRowsText; }
	COLORREF ListBoxEvenRowsText() const { return m_ListBoxEvenRowsText; }
	COLORREF ListBoxSelectedBG() const { return m_ListBoxSelectedBG; }
	COLORREF ListBoxSelectedNoFocusBG() const { return m_ListBoxSelectedNoFocusBG; }
	COLORREF ListBoxSelectedText() const { return m_ListBoxSelectedText; }
	COLORREF ListBoxSelectedNoFocusText() const { return m_ListBoxSelectedNoFocusText; }
	COLORREF ClipPastedColor() const { return m_clipPastedColor; }

	COLORREF ListSmallQuickPasteIndexColor() const { return m_listSmallQuickPasteIndexColor;  }
	COLORREF MainWindowBG() const { return m_mainWindowBG; }
	COLORREF SearchTextBoxFocusBG() const { return m_searchTextBoxFocusBG; }
	COLORREF SearchTextBoxFocusText() const { return m_searchTextBoxFocusText; }
	COLORREF SearchTextBoxFocusBorder() const { return m_searchTextBoxFocusBorder; }
	COLORREF SearchTextHighlight() const { return m_searchTextHighlight; }

	COLORREF GroupTreeBG() const { return m_groupTreeBG; }
	COLORREF GroupTreeText() const { return m_groupTreeText; }

	int GetCaptionSize() const { return m_captionSize; }
	int GetCaptionFontSize() const { return m_captionFontSize; }

	COLORREF DescriptionWindowBG() const { return m_descriptionWindowBG; }
	COLORREF DescriptionWindowText() const { return m_descriptionWindowText; }

	// Modern scrollbar colors
	COLORREF ScrollBarThumb() const { return m_scrollBarThumb; }
	COLORREF ScrollBarThumbHover() const { return m_scrollBarThumbHover; }
	COLORREF ScrollBarTrack() const { return m_scrollBarTrack; }

	// Semantic color tokens. Use CUiTheme to resolve legacy themes, high
	// contrast, and Windows accent fallback.
	COLORREF Canvas() const { return m_canvas; }
	COLORREF Surface() const { return m_surface; }
	COLORREF SurfaceAlt() const { return m_surfaceAlt; }
	COLORREF SurfaceSunken() const { return m_surfaceSunken; }
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

	// Semantic metrics are logical pixels at 96 DPI.
	int RadiusWindow() const { return m_radiusWindow; }
	int RadiusPanel() const { return m_radiusPanel; }
	int RadiusControl() const { return m_radiusControl; }
	int RadiusListItem() const { return m_radiusListItem; }
	int BorderWidth() const { return m_borderWidth; }
	int FocusRingWidth() const { return m_focusRingWidth; }
	int CaptionHeight() const { return m_captionHeight; }
	int CaptionButtonSize() const { return m_captionButtonSize; }
	int CaptionIconSize() const { return m_captionIconSize; }
	int SearchHeight() const { return m_searchHeight; }
	int BottomBarHeight() const { return m_bottomBarHeight; }
	int BottomBarPadding() const { return m_bottomBarPadding; }
	int ListRowHeight() const { return m_listRowHeight; }
	int ListRowInsetX() const { return m_listRowInsetX; }
	int ListRowInsetY() const { return m_listRowInsetY; }
	int ListIndexColumnWidth() const { return m_listIndexColumnWidth; }
	int ListIconSize() const { return m_listIconSize; }
	int IconSizeSmall() const { return m_iconSizeSmall; }
	int IconSizeMedium() const { return m_iconSizeMedium; }
	int IconSizeLarge() const { return m_iconSizeLarge; }
	int ScrollBarThumbWidth() const { return m_scrollBarThumbWidth; }
	int ScrollBarThumbHoverWidth() const { return m_scrollBarThumbHoverWidth; }
	int ScrollBarInset() const { return m_scrollBarInset; }
	int ScrollBarRadius() const { return m_scrollBarRadius; }
	int ScrollBarMinThumb() const { return m_scrollBarMinThumb; }
	int TooltipRadius() const { return m_tooltipRadius; }
	int TooltipPadding() const { return m_tooltipPadding; }
	int DialogPadding() const { return m_dialogPadding; }
	int FieldGap() const { return m_fieldGap; }
	int SectionGap() const { return m_sectionGap; }
	int ButtonHeight() const { return m_buttonHeight; }
	int ButtonMinWidth() const { return m_buttonMinWidth; }
	int ButtonPaddingX() const { return m_buttonPaddingX; }
	int ButtonRadius() const { return m_buttonRadius; }
	int InputHeight() const { return m_inputHeight; }
	int InputRadius() const { return m_inputRadius; }
	int InputPaddingX() const { return m_inputPaddingX; }
	int TabHeight() const { return m_tabHeight; }
	int TabPaddingX() const { return m_tabPaddingX; }
	int TabUnderlineHeight() const { return m_tabUnderlineHeight; }
	int MenuItemHeight() const { return m_menuItemHeight; }
	int MenuPaddingX() const { return m_menuPaddingX; }
	int MenuGutterWidth() const { return m_menuGutterWidth; }
	int EmptyStateMaxWidth() const { return m_emptyStateMaxWidth; }
	int FontSizeBody() const { return m_fontSizeBody; }
	int FontSizeSecondary() const { return m_fontSizeSecondary; }
	int FontSizeCaption() const { return m_fontSizeCaption; }
	int FontSizeTitle() const { return m_fontSizeTitle; }
	int FontSizeWindowTitle() const { return m_fontSizeWindowTitle; }
	int FontSizeDialogBody() const { return m_fontSizeDialogBody; }
	int IconStrokeTenths() const { return m_iconStrokeTenths; }
	int MotionFastMs() const { return m_motionFastMs; }
	int MotionStdMs() const { return m_motionStdMs; }
	int ScrollBarAutoHideMs() const { return m_scrollBarAutoHideMs; }

	CString FontFamilyUi() const { return m_fontFamilyUi; }
	CString FontFamilyMono() const { return m_fontFamilyMono; }

	COLORREF WindowsAccent() const { return m_windowsAccent; }
	bool HasWindowsAccent() const { return m_hasWindowsAccent; }

	// True when a semantic key came from a theme XML file. LoadDefaults()
	// intentionally reports false so CUiTheme can map old themes to legacy keys.
	bool IsSemanticTokenExplicit(LPCTSTR psToken) const;
	bool UsesSemanticDefaults() const { return m_bUsingSemanticDefaults; }

	CString Notes() const { return m_csNotes; }
	CString Author() const { return m_csAuthor; }
	long FileVersion() const { return m_lFileVersion; }

	CString LastError() const { return m_csLastError; }

protected:
	bool LoadElement(TiXmlElement *pParent, CStringA csNode, COLORREF &Color, int &intValue, bool bLogErrors = true);

	bool LoadInt(TiXmlElement *pParent, CStringA csNode, int &intValue);
	bool LoadColor(TiXmlElement *pParent, CStringA csNode, COLORREF &Color);
	bool LoadOptionalInt(TiXmlElement *pParent, CStringA csNode, int &intValue);
	bool LoadOptionalColor(TiXmlElement *pParent, CStringA csNode, COLORREF &Color);
	bool LoadOptionalString(TiXmlElement *pParent, CStringA csNode, CString &value);
	bool LoadSemanticColor(TiXmlElement *pParent, CStringA csNode, COLORREF &Color);
	bool LoadSemanticInt(TiXmlElement *pParent, CStringA csNode, int &intValue);
	bool LoadSemanticString(TiXmlElement *pParent, CStringA csNode, CString &value);
	void LoadWindowsAccentColor();

protected:
	COLORREF m_CaptionLeft;
	COLORREF m_CaptionRight;
	COLORREF m_CaptionLeftTopMost;
	COLORREF m_CaptionRightTopMost;
	COLORREF m_CaptionLeftNotConnected;
	COLORREF m_CaptionRightNotConnected;
	COLORREF m_CaptionTextColor;

	COLORREF m_ListBoxOddRowsBG;
	COLORREF m_ListBoxEvenRowsBG;
	COLORREF m_ListBoxOddRowsText;
	COLORREF m_ListBoxEvenRowsText;
	COLORREF m_ListBoxSelectedBG;
	COLORREF m_ListBoxSelectedNoFocusBG;
	COLORREF m_ListBoxSelectedText;
	COLORREF m_ListBoxSelectedNoFocusText;	
	COLORREF m_clipPastedColor;
	COLORREF m_listSmallQuickPasteIndexColor;
	COLORREF m_mainWindowBG;
	COLORREF m_Border;
	COLORREF m_BorderTopMost;
	COLORREF m_BorderNotConnected;
	COLORREF m_searchTextBoxFocusBG;
	COLORREF m_searchTextBoxFocusText;
	COLORREF m_searchTextBoxFocusBorder;
	COLORREF m_searchTextHighlight;

	COLORREF m_groupTreeBG;
	COLORREF m_groupTreeText;

	COLORREF m_descriptionWindowBG;
	COLORREF m_descriptionWindowText;

	// Modern scrollbar colors
	COLORREF m_scrollBarThumb;
	COLORREF m_scrollBarThumbHover;
	COLORREF m_scrollBarTrack;

	COLORREF m_canvas;
	COLORREF m_surface;
	COLORREF m_surfaceAlt;
	COLORREF m_surfaceSunken;
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

	int m_radiusWindow;
	int m_radiusPanel;
	int m_radiusControl;
	int m_radiusListItem;
	int m_borderWidth;
	int m_focusRingWidth;
	int m_captionHeight;
	int m_captionButtonSize;
	int m_captionIconSize;
	int m_searchHeight;
	int m_bottomBarHeight;
	int m_bottomBarPadding;
	int m_listRowHeight;
	int m_listRowInsetX;
	int m_listRowInsetY;
	int m_listIndexColumnWidth;
	int m_listIconSize;
	int m_iconSizeSmall;
	int m_iconSizeMedium;
	int m_iconSizeLarge;
	int m_scrollBarThumbWidth;
	int m_scrollBarThumbHoverWidth;
	int m_scrollBarInset;
	int m_scrollBarRadius;
	int m_scrollBarMinThumb;
	int m_tooltipRadius;
	int m_tooltipPadding;
	int m_dialogPadding;
	int m_fieldGap;
	int m_sectionGap;
	int m_buttonHeight;
	int m_buttonMinWidth;
	int m_buttonPaddingX;
	int m_buttonRadius;
	int m_inputHeight;
	int m_inputRadius;
	int m_inputPaddingX;
	int m_tabHeight;
	int m_tabPaddingX;
	int m_tabUnderlineHeight;
	int m_menuItemHeight;
	int m_menuPaddingX;
	int m_menuGutterWidth;
	int m_emptyStateMaxWidth;
	int m_fontSizeBody;
	int m_fontSizeSecondary;
	int m_fontSizeCaption;
	int m_fontSizeTitle;
	int m_fontSizeWindowTitle;
	int m_fontSizeDialogBody;
	int m_iconStrokeTenths;
	int m_motionFastMs;
	int m_motionStdMs;
	int m_scrollBarAutoHideMs;

	CString m_fontFamilyUi;
	CString m_fontFamilyMono;

	COLORREF m_windowsAccent;
	bool m_hasWindowsAccent;
	bool m_bUsingSemanticDefaults;
	std::set<CString> m_loadedSemanticTokens;

	int m_captionSize;
	int m_captionFontSize;

	CString m_csLastError;
	long m_lFileVersion;
	CString m_csAuthor;
	CString m_csNotes;

	__int64 m_LastWriteTime;
	CString m_lastTheme;

	void LoadDefaults();
};
