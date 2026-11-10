// GroupStatic.cpp : implementation file
//

#include "stdafx.h"
#include "cp_main.h"
#include "GroupStatic.h"
#include "UiDraw.h"
#include "UiTheme.h"

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// CGroupStatic

CGroupStatic::CGroupStatic()
{
	m_dwTextColor = 0;
	m_dwBkColor = RGB(255, 255, 255);
	m_toggleCursorToHand = false;
	m_pFont = NULL;
	m_uiStyle = STYLE_DEFAULT;
	m_bActionHover = false;
	m_bTracking = false;
	m_actionRect.SetRectEmpty();
}

CGroupStatic::~CGroupStatic()
{
	if (IsWindow(m_toolTip.GetSafeHwnd()))
		m_toolTip.DestroyWindow();
}


BEGIN_MESSAGE_MAP(CGroupStatic, CStatic)
	//{{AFX_MSG_MAP(CGroupStatic)
	ON_WM_SETCURSOR()
	ON_WM_PAINT()
	ON_WM_ERASEBKGND()
	ON_WM_MOUSEMOVE()
	ON_MESSAGE(WM_MOUSELEAVE, OnMouseLeave)
	ON_WM_LBUTTONUP()
	ON_WM_SIZE()
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// CGroupStatic message handlers

BOOL CGroupStatic::OnChildNotify(UINT message, WPARAM wParam, LPARAM lParam, LRESULT* pLResult) 
{  
#ifndef WIN32
      return CStatic::OnChildNotify(message, wParam, lParam, pLResult);
#else
   // If not setting static control color, do default processing
   if( message != WM_CTLCOLORSTATIC )
      return CStatic::OnChildNotify(message, wParam, lParam, pLResult);

   HDC hdcChild = (HDC)wParam;

   // Set the foreground color
   ::SetTextColor( hdcChild, m_dwTextColor );

   // If a background color is pre-determined
   if(m_dwBkColor != -1)
   {  
      ::SetBkMode(hdcChild, TRANSPARENT);
      ::SetBkColor(hdcChild, m_dwBkColor);  
	   m_brush.DeleteObject();
	   m_brush.CreateSolidBrush(m_dwBkColor);
      *pLResult = (LRESULT)(m_brush.GetSafeHandle());
   }
   else
   {
   // Determine the current background color based on my parent window
      static COLORREF clrPrevValid = -1;
      HWND hParent = ::GetParent(m_hWnd);
      HDC  hParentDc = ::GetDC(hParent);

      // Get the color based on the 0, 0 reference
      COLORREF clrParentBkground = ::GetPixel(hParentDc, 0, 0);
      ::ReleaseDC(hParent, hParentDc);

      // If found (not off of the screen or under another window)
      // set my current color to it
      if(clrParentBkground == -1)
      {  clrParentBkground = clrPrevValid;  }
      else
      {  clrPrevValid = clrParentBkground;  }

      // If either the current, or previous color found was not valid
      // allow to perform default processing
      if(clrParentBkground == -1)
      {  return FALSE;  }


      // Set the background mode to transparent
      ::SetBkMode(hdcChild, TRANSPARENT);

      // Set the background color and brush based on my parent's color
      ::SetBkColor(hdcChild, clrParentBkground);  
	   m_brush.DeleteObject();
	   m_brush.CreateSolidBrush(clrParentBkground);
      *pLResult = (LRESULT)(m_brush.GetSafeHandle());
   }

   // Return TRUE to indicate that the message was handled
   return TRUE;
#endif
}


/*************************************************************************
*
*************************************************************************/
void CGroupStatic::SetFont( int nPointSize, LPCTSTR lpszFaceName, CDC* pDC )
{
   // If a font has been allocated, delete it
   if( m_pFont )
      delete m_pFont;

   m_pFont = new CFont;

   // Create a font using the given attributes
   m_pFont->CreatePointFont( nPointSize, lpszFaceName, pDC );

   // Set the window's current font to the specified font
   CStatic::SetFont( m_pFont );
}

void CGroupStatic::SetWindowText(LPCTSTR text)
{
	CStatic::SetWindowText(text);
	UpdateToolTip();
}

void CGroupStatic::SetUiStyle(UiStyle style)
{
	m_uiStyle = style;
	UpdateToolTip();
	Invalidate();
}

void CGroupStatic::SetEmptyState(const CString& title, const CString& detail, const CString& action)
{
	m_emptyTitle = title;
	m_emptyDetail = detail;
	m_emptyAction = action;
	SetWindowText(title);
	Invalidate();
}

void CGroupStatic::RefreshTheme()
{
	UpdateToolTip();
	Invalidate();
}

BOOL CGroupStatic::PreTranslateMessage(MSG* pMsg)
{
	if (IsWindow(m_toolTip.GetSafeHwnd()))
		m_toolTip.RelayEvent(pMsg);

	return CStatic::PreTranslateMessage(pMsg);
}

void CGroupStatic::UpdateToolTip()
{
	if (!IsWindow(m_hWnd) || m_uiStyle != STYLE_GROUP_HEADER)
		return;

	CString text;
	GetWindowText(text);
	if (text.IsEmpty())
		return;

	if (!IsWindow(m_toolTip.GetSafeHwnd()))
		m_toolTip.Create(this);

	CRect rect;
	GetClientRect(rect);
	if (m_toolTip.GetToolCount() == 0)
		m_toolTip.AddTool(this, text, rect, 1);
	else
	{
		m_toolTip.UpdateTipText(text, this, 1);
		m_toolTip.SetToolRect(this, 1, rect);
	}
}

void CGroupStatic::OnPaint()
{
	CPaintDC dc(this);

	CRect rect;
	GetClientRect(rect);

	switch (m_uiStyle)
	{
	case STYLE_GROUP_HEADER:
		DrawGroupHeader(dc, rect);
		break;
	case STYLE_EMPTY_STATE:
		DrawEmptyState(dc, rect);
		break;
	default:
		CStatic::OnPaint();
		break;
	}
}

void CGroupStatic::DrawGroupHeader(CDC& dc, const CRect& rect)
{
	const CUiTheme& theme = UiTheme();
	CDPI dpi(m_hWnd);
	dc.FillSolidRect(rect, theme.Canvas());

	CString text;
	GetWindowText(text);

	CRect textRect(rect);
	textRect.left += dpi.Scale(theme.FieldGap());
	textRect.right -= dpi.Scale(theme.FieldGap());
	UiDraw::DrawTextEllipsis(&dc, text, textRect, theme.FontFamilyUi(),
		theme.FontSizeTitle(), FW_SEMIBOLD, theme.TextPrimary(), dpi.GetDPI(),
		DT_LEFT | DT_VCENTER | DT_SINGLELINE);
}

void CGroupStatic::DrawEmptyState(CDC& dc, const CRect& rect)
{
	const CUiTheme& theme = UiTheme();
	CDPI dpi(m_hWnd);
	const int currentDpi = dpi.GetDPI();

	dc.FillSolidRect(rect, theme.Canvas());

	const int padding = dpi.Scale(theme.DialogPadding());
	CRect content(rect);
	content.DeflateRect(padding, padding);
	if (content.Width() <= 0 || content.Height() <= 0)
		return;

	const int iconSize = dpi.Scale(theme.IconSizeLarge());
	const int iconCircle = dpi.Scale(theme.IconSizeLarge() + theme.SectionGap());
	const int titleHeight = UiDraw::MeasureText(&dc, m_emptyTitle,
		theme.FontFamilyUi(), theme.FontSizeTitle(), FW_SEMIBOLD, currentDpi).cy;
	const int hintHeight = max(
		UiDraw::MeasureText(&dc, m_emptyDetail,
			theme.FontFamilyUi(), theme.FontSizeSecondary(), FW_NORMAL, currentDpi).cy * 2,
		dpi.Scale(theme.ButtonHeight()));
	const int actionHeight = dpi.Scale(theme.ButtonHeight());
	const int gap = dpi.Scale(theme.FieldGap());
	const int sectionGap = dpi.Scale(theme.SectionGap());
	const int totalHeight = iconCircle + sectionGap + titleHeight + gap +
		hintHeight + sectionGap + actionHeight;

	int top = content.top + max(0, (content.Height() - totalHeight) / 2);
	CRect iconCircleRect;
	iconCircleRect.SetRect(content.CenterPoint().x - iconCircle / 2, top,
		content.CenterPoint().x + (iconCircle - iconCircle / 2), top + iconCircle);
	UiDraw::RoundedRect(&dc, iconCircleRect, iconCircle / 2,
		theme.SurfaceAlt(), CLR_INVALID, 0);

	CRect iconRect(iconCircleRect);
	iconRect.DeflateRect((iconCircle - iconSize) / 2, (iconCircle - iconSize) / 2);
	UiDraw::DrawGlyph(&dc, iconRect, UiDraw::Glyph::Search,
		theme.TextSecondary(), currentDpi, theme.IconStrokeTenths());

	top = iconCircleRect.bottom + sectionGap;
	CRect titleRect(content.left, top, content.right, top + titleHeight);
	UiDraw::DrawTextEllipsis(&dc, m_emptyTitle, titleRect, theme.FontFamilyUi(),
		theme.FontSizeTitle(), FW_SEMIBOLD, theme.TextPrimary(), currentDpi,
		DT_CENTER | DT_VCENTER | DT_SINGLELINE);

	top = titleRect.bottom + gap;
	CRect hintRect(content.left, top, content.right, top + hintHeight);
	UiDraw::DrawTextEllipsis(&dc, m_emptyDetail, hintRect, theme.FontFamilyUi(),
		theme.FontSizeSecondary(), FW_NORMAL, theme.TextSecondary(), currentDpi,
		DT_CENTER | DT_WORDBREAK | DT_NOPREFIX);

	top = hintRect.bottom + sectionGap;
	CSize actionSize = UiDraw::MeasureText(&dc, m_emptyAction,
		theme.FontFamilyUi(), theme.FontSizeBody(), FW_SEMIBOLD, currentDpi);
	int actionWidth = max(dpi.Scale(theme.ButtonMinWidth()),
		actionSize.cx + dpi.Scale(theme.ButtonPaddingX() * 2));
	m_actionRect.SetRect(content.CenterPoint().x - actionWidth / 2, top,
		content.CenterPoint().x + (actionWidth - actionWidth / 2), top + actionHeight);

	UiDraw::RoundedRect(&dc, m_actionRect, dpi.Scale(theme.ButtonRadius()),
		m_bActionHover ? theme.AccentFillHover() : theme.AccentFill(),
		CLR_INVALID, 0);
	UiDraw::DrawTextEllipsis(&dc, m_emptyAction, m_actionRect, theme.FontFamilyUi(),
		theme.FontSizeBody(), FW_SEMIBOLD, theme.AccentOn(), currentDpi,
		DT_CENTER | DT_VCENTER | DT_SINGLELINE);
}

BOOL CGroupStatic::OnEraseBkgnd(CDC* pDC)
{
	if (m_uiStyle != STYLE_DEFAULT)
		return TRUE;

	return CStatic::OnEraseBkgnd(pDC);
}

void CGroupStatic::OnMouseMove(UINT nFlags, CPoint point)
{
	if (!m_bTracking)
	{
		TRACKMOUSEEVENT track = { sizeof(track), TME_LEAVE, m_hWnd, 0 };
		m_bTracking = _TrackMouseEvent(&track) != FALSE;
	}

	bool hover = m_uiStyle == STYLE_EMPTY_STATE && m_actionRect.PtInRect(point);
	if (hover != m_bActionHover)
	{
		m_bActionHover = hover;
		InvalidateRect(m_actionRect);
	}

	CStatic::OnMouseMove(nFlags, point);
}

LRESULT CGroupStatic::OnMouseLeave(WPARAM, LPARAM)
{
	m_bTracking = false;
	if (m_bActionHover)
	{
		m_bActionHover = false;
		InvalidateRect(m_actionRect);
	}

	return 0;
}

void CGroupStatic::OnLButtonUp(UINT nFlags, CPoint point)
{
	if (m_uiStyle == STYLE_EMPTY_STATE && m_actionRect.PtInRect(point))
	{
		if (CWnd* parent = GetParent())
			parent->SendMessage(NM_CANCEL_SEARCH, 0, 0);
		return;
	}

	CStatic::OnLButtonUp(nFlags, point);
}

void CGroupStatic::OnSize(UINT nType, int cx, int cy)
{
	CStatic::OnSize(nType, cx, cy);
	UpdateToolTip();
}

BOOL CGroupStatic::OnSetCursor(CWnd* pWnd, UINT nHitTest, UINT message) 
{
	if (m_uiStyle == STYLE_EMPTY_STATE)
	{
		CPoint point;
		GetCursorPos(&point);
		ScreenToClient(&point);
		if (m_actionRect.PtInRect(point))
		{
			::SetCursor(::LoadCursor(NULL, IDC_HAND));
			return TRUE;
		}
	}

	if (m_toggleCursorToHand)
	{
		HCURSOR h = ::LoadCursor(NULL, IDC_HAND);
		::SetCursor(h);
		return TRUE;
	}

	return FALSE;
}
