// AeroEdit.cpp : implementation file
//

#include "stdafx.h"
#include "SymbolEdit.h"
#include "cp_main.h"
#include "QListCtrl.h"
#include "..\Shared\TextConvert.h"
#include "DarkMode.h"
#include "UiDraw.h"
#include "UiTheme.h"

// CSymbolEdit

#define RANGE_START 3000
#define CLEAR_LIST 3010
#define LIST_MAX_COUNT 10
#define MAX_SAVED_SEARCH_LENGTH 50

IMPLEMENT_DYNAMIC(CSymbolEdit, CEdit)

CSymbolEdit::CSymbolEdit() :
	m_hSymbolIcon(NULL),
	m_bInternalIcon(false),
	m_colorPromptText(UiTheme().TextSecondary()),
	m_lastBrushColor(CLR_INVALID),
	m_centerTextDiff(0)
{
	const CUiTheme& theme = UiTheme();
	m_fontPrompt.CreateFont(
		-theme.FontSizeBody(),     // nHeight
		0,                         // nWidth
		0,                         // nEscapement
		0,                         // nOrientation
		FW_NORMAL,                 // nWeight
		TRUE,                      // bItalic
		FALSE,                     // bUnderline
		0,                         // cStrikeOut
		DEFAULT_CHARSET,           // nCharSet
		OUT_DEFAULT_PRECIS,        // nOutPrecision
		CLIP_DEFAULT_PRECIS,       // nClipPrecision
		DEFAULT_QUALITY,           // nQuality
		DEFAULT_PITCH | FF_SWISS,  // nPitchAndFamily
		theme.FontFamilyUi());

	m_mouseDownOnSearches = false;
	m_mouseHoveringOverSearches = false;
	m_mouseDownOnClose = false;
	m_mouseHoveringOverClose = false;
	m_windowDpi = NULL;

	//m_searchButton.LoadStdImageDPI(Search_16, Search_20, Search_24, Search_32, _T("PNG"));
	
}

CSymbolEdit::~CSymbolEdit()
{
	DestroyIcon();
}


BEGIN_MESSAGE_MAP(CSymbolEdit, CEdit)
	ON_WM_PAINT()
	ON_MESSAGE(WM_SETFONT, OnSetFont)
	//ON_MESSAGE(WM_EXITMENULOOP, OnMenuExit)
	ON_WM_CTLCOLOR_REFLECT()
	ON_WM_SETFOCUS()
	ON_WM_KILLFOCUS()
	ON_WM_SETCURSOR()
	ON_WM_LBUTTONUP()
	ON_WM_LBUTTONDOWN()
	ON_WM_MOUSEMOVE()
	ON_COMMAND_RANGE(RANGE_START, (RANGE_START + LIST_MAX_COUNT), OnSelectSearchString)
	ON_WM_EXITSIZEMOVE()
	//ON_WM_ERASEBKGND()
	ON_WM_NCCALCSIZE()
	ON_WM_NCPAINT()
	ON_WM_TIMER()
END_MESSAGE_MAP()

BOOL CSymbolEdit::PreTranslateMessage(MSG* pMsg)
{
	// TODO: Add your specialized code here and/or call the base class
	// Intercept Ctrl + Z (Undo), Ctrl + X (Cut), Ctrl + C (Copy), Ctrl + V (Paste) and Ctrl + A (Select All)
	// before CEdit base class gets a hold of them.
	if (pMsg->message == WM_KEYDOWN &&
		CONTROL_PRESSED)
	{
		switch (pMsg->wParam)
		{
		case 'Z':
			Undo();
			return TRUE;
		case 'X':
			Cut();
			return TRUE;
		case 'C':
			{
				int startChar;
				int endChar;
				this->GetSel(startChar, endChar);
				if (startChar == endChar)
				{
					CWnd *pWnd = GetParent();
					if (pWnd)
					{
						pWnd->SendMessage(NM_COPY_CLIP, pMsg->wParam, pMsg->lParam);
					}
				}
				else
				{
					Copy();
				}
			}
			return TRUE;
		case 'V':
			Paste();
			return TRUE;
		case 'A':
			SetSel(0, -1);
			return TRUE;
		}
	}

	switch (pMsg->message)
	{
	case WM_KEYDOWN:
	{
		if (pMsg->wParam == VK_RETURN)
		{
			CWnd *pWnd = GetParent();
			if (pWnd)
			{
				if (CGetSetOptions::m_bFindAsYouType)
				{
					pWnd->SendMessage(NM_SEARCH_ENTER_PRESSED, 0, 0);
				}
				else
				{
					//Send a message to the parent to refill the lb from the search
					pWnd->PostMessage(CB_SEARCH, 0, 0);
				}

				AddToSearchHistory();
			}

			return TRUE;
		}
		else if (pMsg->wParam == VK_DOWN &&
			((GetKeyState(VK_CONTROL) & 0x8000) || ((GetKeyState(VK_CONTROL) & 0x8000) && (GetKeyState(VK_SHIFT) & 0x8000))))
		{
			if (ShowSearchHistoryMenu())
			{
				return TRUE;
			}
		}
		else if (pMsg->wParam == VK_DOWN ||
			pMsg->wParam == VK_UP ||
			pMsg->wParam == VK_PRIOR ||
			pMsg->wParam == VK_NEXT)
		{
			CWnd *pWnd = GetParent();
			if (pWnd)
			{
				pWnd->SendMessage(CB_UPDOWN, pMsg->wParam, pMsg->lParam);
				return TRUE;
			}
		}
		else if (pMsg->wParam == VK_DELETE)
		{
			int startChar;
			int endChar;
			this->GetSel(startChar, endChar);
			CString cs;
			this->GetWindowText(cs);
			//if selection is at the end then forward this on to the parent to delete the selected clip
			if(startChar == cs.GetLength() &&
				endChar == cs.GetLength())
			{ 
				CWnd *pWnd = GetParent();
				if (pWnd)
				{
					pWnd->SendMessage(NM_DELETE, pMsg->wParam, pMsg->lParam);
					return TRUE;
				}
			}
		}
		break;
	}
	}

	return CEdit::PreTranslateMessage(pMsg);
}

CString CSymbolEdit::SavePastSearches()
{
	TiXmlDocument doc;

	TiXmlElement* outer = new TiXmlElement("PastSearches");
	doc.LinkEndChild(outer);

	int count = (int)m_searches.GetCount();
	for (int i = 0; i < count; i++)
	{		
		TiXmlElement* searchElement = new TiXmlElement("Search");

		CStringA t = CTextConvert::UnicodeToUTF8(m_searches[i]);
		searchElement->SetAttribute("text", t);

		outer->LinkEndChild(searchElement);
	}

	TiXmlPrinter printer;
	printer.SetLineBreak("");
	doc.Accept(&printer);
	CString cs = printer.CStr();

	return cs;
}

void CSymbolEdit::LoadPastSearches(CString values)
{
	m_searches.RemoveAll();

	TiXmlDocument doc;
	CStringA xmlA = CTextConvert::UnicodeToUTF8(values);
	doc.Parse(xmlA);

	TiXmlElement *ItemHeader = doc.FirstChildElement("PastSearches");

	if (ItemHeader != NULL)
	{
		TiXmlElement *ItemElement = ItemHeader->FirstChildElement();

		int count = 0;

		while (ItemElement)
		{
			if (count < LIST_MAX_COUNT)
			{
				CString item = ItemElement->Attribute("text");

				CString toAdd = item.Left(MAX_SAVED_SEARCH_LENGTH);
				if (toAdd != _T(""))
				{
					m_searches.Add(toAdd);
				}

				ItemElement = ItemElement->NextSiblingElement();
			}
			else
			{
				break;
			}

			count++;
		}
	}
}

void CSymbolEdit::AddToSearchHistory()
{
	CString cs;
	this->GetWindowText(cs);
	if (cs != _T(""))
	{
		//only save up to 50, had reports of somehow getting extremely large amounts of junk text
		//save and causing memory issues.
		cs = cs.Left(MAX_SAVED_SEARCH_LENGTH);

		if (m_searches.GetCount() >= LIST_MAX_COUNT)
		{
			m_searches.RemoveAt(0);
		}

		bool existing = false;
		int count = (int)m_searches.GetCount();
		for (int i = 0; i < count; i++)
		{
			if (m_searches[i] == cs)
			{
				m_searches.RemoveAt(i);
				m_searches.Add(cs);
				existing = true;
				break;
			}
		}

		if (existing == false)
		{
			m_searches.Add(cs);
		}
	}
}

bool CSymbolEdit::ShowSearchHistoryMenu()
{
	if (m_searches.GetCount() == 0)
	{
		return false;
	}

	CMenu cmPopUp;
	cmPopUp.CreatePopupMenu();

	int count = min((int)m_searches.GetCount(), LIST_MAX_COUNT);
	for (int i = count-1; i >= 0; i--)
	{
		CString text = m_searches[i];

		if (i == count - 1 &&
			m_lastSearchShortCut.Key > 0)
		{
			CString cmdShortcutText = CHotKey::GetHotKeyDisplayStatic(m_lastSearchShortCut.Key);
			if (m_lastSearchShortCut.Key2 != 0)
			{
				CString cmdShortcutText2 = CHotKey::GetHotKeyDisplayStatic(m_lastSearchShortCut.Key2);

				if (cmdShortcutText2.GetLength() > 0)
				{
					cmdShortcutText += _T(" - ");
					cmdShortcutText += cmdShortcutText2;
				}
			}

			text += "\t";
			text += cmdShortcutText;
		}

		cmPopUp.AppendMenuW(MF_STRING, (RANGE_START + i), text);
	}

	cmPopUp.AppendMenu(MF_SEPARATOR);
	cmPopUp.AppendMenuW(MF_STRING, CLEAR_LIST, _T("Clear List"));

	CRect windowRect;
	this->GetWindowRect(&windowRect);
	POINT pp;
	GetCursorPos(&pp);
	POINT x = this->GetCaretPos();
	ClientToScreen(&x);
	x.y += windowRect.Height();

	cmPopUp.TrackPopupMenu(TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON, x.x, x.y, this, NULL);
	
	Invalidate();
	
	return true;
}

void CSymbolEdit::DestroyIcon()
{
	// if icon was loaded internally, destroy it
	if (m_bInternalIcon || m_hSymbolIcon != NULL)
		::DestroyIcon(m_hSymbolIcon);
}

void CSymbolEdit::PreSubclassWindow()
{
	RecalcLayout();
}

void CSymbolEdit::SetSymbolIcon(HICON hIcon, BOOL redraw)
{
	DestroyIcon();

	m_hSymbolIcon = hIcon;

	// icon was not loaded internally
	m_bInternalIcon = false;

	RecalcLayout();

	if (redraw)
		Invalidate(TRUE);
}

void CSymbolEdit::SetSymbolIcon(UINT id, BOOL redraw)
{
	DestroyIcon();

	CDPI dpi(m_hWnd);
	int iconSize = dpi.Scale(UiTheme().IconSizeSmall());
	m_hSymbolIcon = (HICON)::LoadImage(
		AfxGetResourceHandle(),
		MAKEINTRESOURCE(id),
		IMAGE_ICON,
		iconSize,
		iconSize,
		LR_DEFAULTCOLOR | LR_LOADTRANSPARENT);

	ASSERT(m_hSymbolIcon != NULL);

	// icon was loaded internally
	m_bInternalIcon = true;

	RecalcLayout();

	if (redraw)
		Invalidate(TRUE);
}

void CSymbolEdit::SetPromptText(CString text, BOOL redraw)
{
	m_strPromptText = text;

	if (redraw)
		Invalidate(TRUE);
}

void CSymbolEdit::SetPromptText(LPCTSTR szText, BOOL redraw)
{
	m_strPromptText = szText;

	if (redraw)
		Invalidate(TRUE);
}

void CSymbolEdit::SetPromptTextColor(COLORREF color, BOOL redraw)
{
	m_colorPromptText = color;

	if (redraw)
		Invalidate(TRUE);
}

void CSymbolEdit::SetPromptFont(CFont& font, BOOL redraw)
{
	LOGFONT lf;
	memset(&lf, 0, sizeof(LOGFONT));

	font.GetLogFont(&lf);
	SetPromptFont(&lf);

	if (redraw)
		Invalidate(TRUE);
}

void CSymbolEdit::SetPromptFont(const LOGFONT* lpLogFont, BOOL redraw)
{
	m_fontPrompt.DeleteObject();
	m_fontPrompt.CreateFontIndirect(lpLogFont);

	if (redraw)
		Invalidate(TRUE);
}

void CSymbolEdit::RecalcLayout()
{
	if (m_windowDpi == NULL)
		return;

	const CUiTheme& theme = UiTheme();
	const int iconSize = m_windowDpi->Scale(theme.IconSizeSmall());
	const int buttonSize = m_windowDpi->Scale(theme.CaptionButtonSize());
	const int left = m_windowDpi->Scale(theme.InputPaddingX()) + iconSize +
		m_windowDpi->Scale(theme.FieldGap() / 2);
	const int right = buttonSize * 2 + m_windowDpi->Scale(theme.InputPaddingX());
	SetMargins(left, right);
}

void CSymbolEdit::UpdateWindowRegion()
{
	if (!IsWindow(m_hWnd))
		return;

	CRect rect;
	GetClientRect(&rect);
	if (rect.Width() <= 0 || rect.Height() <= 0)
		return;

	int radius = m_windowDpi->Scale(UiTheme().RadiusControl());
	CRgn region;
	region.CreateRoundRectRgn(0, 0, rect.Width() + 1, rect.Height() + 1,
		radius * 2, radius * 2);
	SetWindowRgn(region, TRUE);
	region.Detach();
}

void CSymbolEdit::OnPaint()
{
	Default();

	CClientDC dc(this);
	CRect rect;
	GetClientRect(rect);
	if (rect.Width() <= 0 || rect.Height() <= 0 || m_windowDpi == NULL)
		return;

	const CUiTheme& theme = UiTheme();
	const int currentDpi = m_windowDpi->GetDPI();
	const int buttonSize = m_windowDpi->Scale(theme.CaptionButtonSize());
	const int iconSize = m_windowDpi->Scale(theme.IconSizeSmall());
	const int glyphSize = m_windowDpi->Scale(theme.IconSizeMedium());
	const int inset = m_windowDpi->Scale(theme.InputPaddingX());

	CRect searchIconRect(inset, rect.CenterPoint().y - iconSize / 2,
		inset + iconSize, rect.CenterPoint().y + (iconSize + 1) / 2);
	UiDraw::DrawGlyph(&dc, searchIconRect, UiDraw::Glyph::Search,
		this == GetFocus() ? theme.Accent() : theme.TextSecondary(),
		currentDpi, theme.IconStrokeTenths());

	CRect clearButton(rect.right - inset - buttonSize, 0,
		rect.right - inset, rect.bottom);
	CRect historyButton(clearButton.left - buttonSize, 0,
		clearButton.left, rect.bottom);

	CString text;
	GetWindowText(text);
	bool showHistory = text.GetLength() > 0 || this == GetFocus();
	bool showClear = text.GetLength() > 0;

	m_searchesButtonRect = showHistory ? historyButton : CRect(0, 0, 0, 0);
	m_closeButtonRect = showClear ? clearButton : CRect(0, 0, 0, 0);

	if (showHistory || showClear)
	{
		CRect actionArea(historyButton.left, rect.top, rect.right, rect.bottom);
		dc.FillSolidRect(actionArea, theme.SearchTextBoxBg());
	}

	if (showHistory)
	{
		if (m_mouseHoveringOverSearches || m_mouseDownOnSearches)
		{
			CRect hover(historyButton);
			hover.DeflateRect((buttonSize - glyphSize) / 4, (buttonSize - glyphSize) / 4);
			UiDraw::RoundedRect(&dc, hover, m_windowDpi->Scale(theme.RadiusControl()),
				theme.Hover(), CLR_INVALID, 0);
		}
		CRect glyph(historyButton.CenterPoint().x - glyphSize / 2,
			historyButton.CenterPoint().y - glyphSize / 2,
			historyButton.CenterPoint().x + (glyphSize + 1) / 2,
			historyButton.CenterPoint().y + (glyphSize + 1) / 2);
		UiDraw::DrawGlyph(&dc, glyph, UiDraw::Glyph::ChevronDown,
			theme.TextSecondary(), currentDpi, theme.IconStrokeTenths());
	}

	if (showClear)
	{
		if (m_mouseHoveringOverClose || m_mouseDownOnClose)
		{
			CRect hover(clearButton);
			hover.DeflateRect((buttonSize - glyphSize) / 4, (buttonSize - glyphSize) / 4);
			UiDraw::RoundedRect(&dc, hover, m_windowDpi->Scale(theme.RadiusControl()),
				theme.Hover(), CLR_INVALID, 0);
		}
		CRect glyph(clearButton.CenterPoint().x - glyphSize / 2,
			clearButton.CenterPoint().y - glyphSize / 2,
			clearButton.CenterPoint().x + (glyphSize + 1) / 2,
			clearButton.CenterPoint().y + (glyphSize + 1) / 2);
		UiDraw::DrawGlyph(&dc, glyph, UiDraw::Glyph::Close,
			theme.TextSecondary(), currentDpi, theme.IconStrokeTenths());
	}

	if (text.IsEmpty() && m_strPromptText.GetLength() > 0 && this != GetFocus())
	{
		DWORD margins = GetMargins();
		CRect promptRect(rect);
		promptRect.left += LOWORD(margins);
		promptRect.right = historyButton.left - m_windowDpi->Scale(theme.FieldGap() / 2);
		UiDraw::DrawTextEllipsis(&dc, m_strPromptText, promptRect, theme.FontFamilyUi(),
			theme.FontSizeBody(), FW_NORMAL, m_colorPromptText, currentDpi,
			DT_LEFT | DT_VCENTER | DT_SINGLELINE);
	}

	m_lastTextOnPaint = text;
}

void CSymbolEdit::OnSize(UINT nType, int cx, int cy)
{
	CEdit::OnSize(nType, cx, cy);

	RecalcLayout();
	UpdateWindowRegion();
}

LRESULT CSymbolEdit::OnSetFont(WPARAM wParam, LPARAM lParam)
{
	DefWindowProc(WM_SETFONT, wParam, lParam);

	RecalcLayout();

	return 0;
}

HBRUSH CSymbolEdit::CtlColor(CDC* pDC, UINT n)
{
	const CUiTheme& theme = UiTheme();
	pDC->SetTextColor(theme.SearchTextBoxText());
	pDC->SetBkColor(theme.SearchTextBoxBg());
	m_lastBrushColor = theme.SearchTextBoxBg();
	return UiDraw::Brush(theme.SearchTextBoxBg());
}

void CSymbolEdit::OnSetFocus(CWnd* pOldWnd)
{
	//OutputDebugString(_T("OnSetFocus \r\n"));

	//was seeing issues when refreshing non client area inline, do it delayed
	SetTimer(1, 500, NULL);
	
	CWnd *pWnd = GetParent();
	if (pWnd)
	{
		if (CGetSetOptions::m_bFindAsYouType)
		{
			pWnd->SendMessage(NM_FOCUS_ON_SEARCH, 0, 0);
		}
	}

	CEdit::OnSetFocus(pOldWnd);
	Invalidate();
	SetWindowPos(NULL, 0, 0, 0, 0, SWP_DRAWFRAME | SWP_NOSIZE | SWP_NOMOVE |
		SWP_NOZORDER | SWP_NOACTIVATE);
}

void CSymbolEdit::OnKillFocus(CWnd* pNewWnd)
{
	//OutputDebugString(_T("OnKillFocus \r\n"));
	AddToSearchHistory();

	//was seeing issues when refreshing non client area inline, do it delayed
	SetTimer(1, 500, NULL);
	
	CEdit::OnKillFocus(pNewWnd);
	Invalidate();
	SetWindowPos(NULL, 0, 0, 0, 0, SWP_DRAWFRAME | SWP_NOSIZE | SWP_NOMOVE |
		SWP_NOZORDER | SWP_NOACTIVATE);
}

BOOL CSymbolEdit::OnSetCursor(CWnd* pWnd, UINT nHitTest, UINT message)
{
	CPoint pntCursor;
	GetCursorPos(&pntCursor);
	ScreenToClient(&pntCursor);

	if(m_closeButtonRect.PtInRect(pntCursor))
	{
		HCURSOR h = ::LoadCursor(NULL, IDC_ARROW);
		::SetCursor(h);
		return TRUE;
	}

	if (m_searchesButtonRect.PtInRect(pntCursor))
	{
		HCURSOR h = ::LoadCursor(NULL, IDC_ARROW);
		::SetCursor(h);
		return TRUE;
	}

	return CEdit::OnSetCursor(pWnd, nHitTest, message);
}

void CSymbolEdit::OnLButtonUp(UINT nFlags, CPoint point)
{
	bool handled = m_mouseDownOnClose || m_mouseDownOnSearches;

	if (m_mouseDownOnClose)
	{
		ReleaseCapture();
		InvalidateRect(m_closeButtonRect);
	}

	if (m_mouseDownOnSearches)
	{
		ReleaseCapture();
		InvalidateRect(m_searchesButtonRect);
	}

	if (m_closeButtonRect.PtInRect(point))
	{
		if ((GetWindowTextLength() > 0))
		{
			CWnd *pOwner = GetOwner();
			if (pOwner)
			{
				pOwner->SendMessage(NM_CANCEL_SEARCH, 0, 0);
			}
		}		
	}
	else if (m_searchesButtonRect.PtInRect(point))
	{
		this->ShowSearchHistoryMenu();
	}

	m_mouseDownOnClose = false;
	m_mouseDownOnSearches = false;

	if (handled)
		Invalidate();
	else
		CEdit::OnLButtonUp(nFlags, point);
}

void CSymbolEdit::OnLButtonDown(UINT nFlags, CPoint point)
{
	if (m_closeButtonRect.PtInRect(point))
	{
		m_mouseDownOnClose = true;
		SetCapture();
		InvalidateRect(m_closeButtonRect);
		return;
	}
	else
	{
		m_mouseDownOnClose = false;
	}

	if (m_searchesButtonRect.PtInRect(point))
	{
		m_mouseDownOnSearches = true;
		SetCapture();
		InvalidateRect(m_searchesButtonRect);
		return;
	}
	else
	{
		m_mouseDownOnSearches = false;
	}

	CEdit::OnLButtonDown(nFlags, point);

	Invalidate();
}

void CSymbolEdit::OnMouseMove(UINT nFlags, CPoint point)
{
	if (m_closeButtonRect.PtInRect(point))
	{
		if (m_mouseHoveringOverClose == false)
		{
			m_mouseHoveringOverClose = true;
			InvalidateRect(m_closeButtonRect);
		}
	}
	else if(m_mouseHoveringOverClose)
	{
		m_mouseHoveringOverClose = false;
		InvalidateRect(m_closeButtonRect);
	}

	if (m_searchesButtonRect.PtInRect(point))
	{
		if (m_mouseHoveringOverSearches == false)
		{
			m_mouseHoveringOverSearches = true;
			InvalidateRect(m_searchesButtonRect);
		}
	}
	else if (m_mouseHoveringOverSearches)
	{
		m_mouseHoveringOverSearches = false;
		InvalidateRect(m_searchesButtonRect);
	}

	CEdit::OnMouseMove(nFlags, point);
}

void CSymbolEdit::OnSelectSearchString(UINT idIn)
{
	int index = idIn - RANGE_START;

	if (idIn == CLEAR_LIST)
	{
		m_searches.RemoveAll();
	}
	else if (index >= 0 &&
		index < m_searches.GetCount())
	{
		CString cs = m_searches[index];
		this->SetWindowTextW(cs);

		this->SetFocus();
		this->SetSel(-1);

		this->Invalidate();

		m_searches.RemoveAt(index);
		m_searches.Add(cs);
	}
}

bool CSymbolEdit::ApplyLastSearch()
{
	bool ret = false;
	if (m_searches.GetCount() > 0)
	{
		CString cs = m_searches[m_searches.GetCount()-1];
		this->SetWindowTextW(cs);

		this->SetFocus();
		this->SetSel(-1);

		this->Invalidate();

		ret = true;
	}

	return ret;
}

void CSymbolEdit::OnDpiChanged()
{
	SetDpiInfo(m_windowDpi);
}

void CSymbolEdit::RefreshTheme()
{
	m_colorPromptText = UiTheme().TextSecondary();
	RecalcLayout();
	UpdateWindowRegion();
	Invalidate(TRUE);
	SetWindowPos(NULL, 0, 0, 0, 0, SWP_DRAWFRAME | SWP_NOSIZE | SWP_NOMOVE |
		SWP_NOZORDER | SWP_NOACTIVATE);
}

void CSymbolEdit::SetDpiInfo(CDPI *dpi)
{ 
	m_windowDpi = dpi; 

	RecalcLayout();
	UpdateWindowRegion();

	Invalidate(TRUE);
	SetWindowPos(NULL, 0, 0, 0, 0, SWP_DRAWFRAME | SWP_NOSIZE | SWP_NOMOVE |
		SWP_NOZORDER | SWP_NOACTIVATE);
}

BOOL CSymbolEdit::OnEraseBkgnd(CDC* pDC)
{
	// TODO: Add your message handler code here and/or call default

	//return CEdit::OnEraseBkgnd(pDC);
	return FALSE;
}


void CSymbolEdit::OnNcCalcSize(BOOL bCalcValidRects, NCCALCSIZE_PARAMS* lpncsp)
{
	if (m_windowDpi != NULL)
	{
		int border = m_windowDpi->Scale(UiTheme().BorderWidth());
		lpncsp->rgrc[0].left += border;
		lpncsp->rgrc[0].top += border;
		lpncsp->rgrc[0].right -= border;
		lpncsp->rgrc[0].bottom -= border;
	}
}


void CSymbolEdit::OnNcPaint()
{
	CWindowDC dc(this);
	if (m_windowDpi == NULL)
		return;

	const CUiTheme& theme = UiTheme();
	CRect rect;
	GetWindowRect(rect);
	rect.OffsetRect(-rect.left, -rect.top);

	const int radius = m_windowDpi->Scale(theme.RadiusControl());
	const int border = m_windowDpi->Scale(theme.BorderWidth());
	const bool focused = this == GetFocus();
	UiDraw::RoundedRect(&dc, rect, radius, CLR_INVALID,
		focused ? theme.FocusRing() : theme.Border(), border);

	if (focused)
	{
		CRect focusRect(rect);
		focusRect.DeflateRect(border, border);
		UiDraw::FocusRing(&dc, focusRect, max(0, radius - border),
			theme.FocusRing(), m_windowDpi->Scale(theme.FocusRingWidth()));
	}
}

void CSymbolEdit::OnTimer(UINT_PTR nIDEvent)
{
	switch (nIDEvent)
	{
	case 1:
			KillTimer(1);
			Invalidate(TRUE);
			SetWindowPos(NULL, 0, 0, 0, 0, SWP_DRAWFRAME | SWP_NOSIZE | SWP_NOMOVE |
				SWP_NOZORDER | SWP_NOACTIVATE);
			break;
	}

	CEdit::OnTimer(nIDEvent);
}
