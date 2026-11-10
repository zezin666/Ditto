#include "stdafx.h"
#include "ModernScrollBar.h"
#include "CP_Main.h"
#include "QListCtrl.h"
#include "DarkMode.h"
#include "UiDraw.h"
#include "UiTheme.h"
#include "memdc.h"
#include <gdiplus.h>

#pragma comment(lib, "gdiplus.lib")

using namespace Gdiplus;

IMPLEMENT_DYNAMIC(CModernScrollBar, CWnd)

CModernScrollBar::CModernScrollBar()
	: m_pListCtrl(NULL)
	, m_pParentWnd(NULL)
	, m_pDPI(NULL)
	, m_orientation(ScrollBarOrientation::Vertical)
	, m_trackColor(UiTheme().ScrollBarTrack())
	, m_thumbColor(UiTheme().ScrollBarThumb())
	, m_thumbHoverColor(UiTheme().ScrollBarThumbHover())
	, m_scrollBarWidth(UiTheme().ScrollBarThumbWidth())
	, m_scrollBarHoverWidth(UiTheme().ScrollBarThumbHoverWidth())
	, m_cornerRadius(UiTheme().ScrollBarRadius())
	, m_minThumbSize(UiTheme().ScrollBarMinThumb())
	, m_isMouseOver(false)
	, m_isDragging(false)
	, m_dragStartPos(0)
	, m_dragStartScrollPos(0)
	, m_isVisible(false)
	, m_trackingMouse(false)
	, m_opacity(0.0)
	, m_fadeStartOpacity(0.0)
	, m_fadeStartTick(0)
	, m_fadeIn(false)
{
}

CModernScrollBar::~CModernScrollBar()
{
}

BEGIN_MESSAGE_MAP(CModernScrollBar, CWnd)
	ON_WM_PAINT()
	ON_WM_ERASEBKGND()
	ON_WM_MOUSEMOVE()
	ON_WM_MOUSELEAVE()
	ON_WM_LBUTTONDOWN()
	ON_WM_LBUTTONUP()
	ON_WM_TIMER()
	ON_MESSAGE(WM_MOUSEHOVER, OnMouseHover)
END_MESSAGE_MAP()

BOOL CModernScrollBar::Create(CWnd* pParentWnd, CListCtrl* pListCtrl, ScrollBarOrientation orientation)
{
	m_pParentWnd = pParentWnd;
	m_pListCtrl = pListCtrl;
	m_orientation = orientation;

	// Create as a child window with no border
	CString className = AfxRegisterWndClass(CS_HREDRAW | CS_VREDRAW, 
		::LoadCursor(NULL, IDC_ARROW), NULL, NULL);
	
	DWORD dwStyle = WS_CHILD | WS_CLIPSIBLINGS;
	DWORD dwExStyle = 0;

	CRect rect(0, 0, 10, 100);
	
	if (!CWnd::CreateEx(dwExStyle, className, _T(""), dwStyle, rect, pParentWnd, 0))
		return FALSE;

	return TRUE;
}

void CModernScrollBar::SetColors(COLORREF trackColor, COLORREF thumbColor, COLORREF thumbHoverColor)
{
	m_trackColor = trackColor;
	m_thumbColor = thumbColor;
	m_thumbHoverColor = thumbHoverColor;
	
	if (m_hWnd && IsWindowVisible())
		Invalidate();
}

void CModernScrollBar::UpdateScrollBar()
{
	if (!m_pListCtrl || !m_pListCtrl->m_hWnd)
		return;

	// Get scroll info for the appropriate orientation
	int scrollBarType = (m_orientation == ScrollBarOrientation::Vertical) ? SB_VERT : SB_HORZ;
	
	SCROLLINFO si = { sizeof(SCROLLINFO) };
	si.fMask = SIF_ALL;
	m_pListCtrl->GetScrollInfo(scrollBarType, &si);

	// Check if scrollbar is needed
	bool needsScrollBar = (si.nMax > si.nMin &&
		si.nPage > 0 &&
		(si.nMax - si.nMin + 1) > (int)si.nPage);
	
	if (!needsScrollBar)
	{
		m_opacity = 0.0;
		KillTimer(TIMER_FADE);
		ShowWindow(SW_HIDE);
		return;
	}

	// Get the list control position relative to parent
	CRect listRectInParent;
	m_pListCtrl->GetWindowRect(&listRectInParent);
	m_pParentWnd->ScreenToClient(&listRectInParent);

	// Get parent client rect to ensure we stay within visible area
	CRect parentClientRect;
	m_pParentWnd->GetClientRect(&parentClientRect);

	// Keep the hit area stable while the visible thumb changes width on hover.
	int scrollSize = m_scrollBarHoverWidth + UiTheme().ScrollBarInset();
	if (m_pDPI)
	{
		scrollSize = m_pDPI->Scale(scrollSize);
	}

	// The shell clips the list to reserve its bottom command bar. Use that
	// region instead of duplicating the shell's current layout metric here.
	int visibleBottom = listRectInParent.bottom;
	int visibleRight = listRectInParent.right;
	CRgn listRegion;
	if (listRegion.CreateRectRgn(0, 0, 0, 0) != ERROR)
	{
		int regionType = m_pListCtrl->GetWindowRgn(listRegion);
		if (regionType != ERROR)
		{
			RECT regionBox = { 0 };
			if (GetRgnBox(listRegion, &regionBox) != ERROR)
			{
				visibleBottom = min(visibleBottom,
					listRectInParent.top + regionBox.bottom);
				visibleRight = min(visibleRight,
					listRectInParent.left + regionBox.right);
			}
		}
	}

	visibleBottom = min(visibleBottom, parentClientRect.bottom);
	visibleRight = min(visibleRight, parentClientRect.right);
	int inset = m_pDPI ?
		m_pDPI->Scale(UiTheme().ScrollBarInset()) :
		UiTheme().ScrollBarInset();

	CRect scrollRect;
	
	if (m_orientation == ScrollBarOrientation::Vertical)
	{
		scrollRect.left = visibleRight - scrollSize;
		scrollRect.top = listRectInParent.top + inset;
		scrollRect.right = visibleRight;
		scrollRect.bottom = visibleBottom - inset;
	}
	else
	{
		scrollRect.left = listRectInParent.left + inset;
		scrollRect.top = visibleBottom - scrollSize;
		scrollRect.right = visibleRight - scrollSize;
		scrollRect.bottom = visibleBottom;
	}

	// Only move if position changed
	CRect currentRect;
	GetWindowRect(&currentRect);
	m_pParentWnd->ScreenToClient(&currentRect);

	if (currentRect != scrollRect)
	{
		MoveWindow(&scrollRect);
	}	

	if (m_isVisible && !IsWindowVisible())
	{
		ShowWindow(SW_SHOWNA);
	}

	// Redraw
	Invalidate();
	
	if (m_isVisible)
	{
		SetWindowPos(&CWnd::wndTop, 0, 0, 0, 0,
			SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
	}
}

CRect CModernScrollBar::GetThumbRect()
{
	CRect thumbRect(0, 0, 0, 0);
	
	if (!m_pListCtrl || !m_pListCtrl->m_hWnd)
		return thumbRect;

	CRect clientRect;
	GetClientRect(&clientRect);

	// Get scroll info for appropriate orientation
	int scrollBarType = (m_orientation == ScrollBarOrientation::Vertical) ? SB_VERT : SB_HORZ;
	
	SCROLLINFO si = { sizeof(SCROLLINFO) };
	si.fMask = SIF_ALL;
	m_pListCtrl->GetScrollInfo(scrollBarType, &si);

	if (si.nMax <= si.nMin || si.nPage <= 0)
		return thumbRect;

	int inset = m_pDPI ?
		m_pDPI->Scale(UiTheme().ScrollBarInset()) :
		UiTheme().ScrollBarInset();
	int trackSize = ((m_orientation == ScrollBarOrientation::Vertical) ?
		clientRect.Height() : clientRect.Width()) - inset * 2;
	if (trackSize <= 0)
		return thumbRect;

	int totalRange = si.nMax - si.nMin + 1;
	
	// Calculate thumb size proportionally
	int thumbSize = (int)((double)si.nPage / totalRange * trackSize);
	
	// Apply minimum thumb size
	int minSize = m_minThumbSize;
	if (m_pDPI)
		minSize = m_pDPI->Scale(m_minThumbSize);
	
	if (thumbSize < minSize)
		thumbSize = minSize;
	
	if (thumbSize > trackSize)
		thumbSize = trackSize;

	// Calculate thumb position
	int scrollableRange = totalRange - si.nPage;
	double scrollRatio = 0;
	if (scrollableRange > 0)
		scrollRatio = (double)(si.nPos - si.nMin) / scrollableRange;
	
	int thumbPos = (int)(scrollRatio * (trackSize - thumbSize));
	
	// Clamp to valid range
	if (thumbPos < 0) thumbPos = 0;
	if (thumbPos + thumbSize > trackSize)
		thumbPos = trackSize - thumbSize;

	if (m_orientation == ScrollBarOrientation::Vertical)
	{
		int thickness = (m_isMouseOver || m_isDragging) ?
			m_scrollBarHoverWidth : m_scrollBarWidth;
		if (m_pDPI)
			thickness = m_pDPI->Scale(thickness);

		thumbRect.left = (clientRect.Width() - thickness) / 2;
		thumbRect.right = thumbRect.left + thickness;
		thumbRect.top = inset + thumbPos;
		thumbRect.bottom = inset + thumbPos + thumbSize;
	}
	else
	{
		int thickness = (m_isMouseOver || m_isDragging) ?
			m_scrollBarHoverWidth : m_scrollBarWidth;
		if (m_pDPI)
			thickness = m_pDPI->Scale(thickness);

		thumbRect.left = inset + thumbPos;
		thumbRect.right = inset + thumbPos + thumbSize;
		thumbRect.top = (clientRect.Height() - thickness) / 2;
		thumbRect.bottom = thumbRect.top + thickness;
	}

	return thumbRect;
}

void CModernScrollBar::DrawRoundedRect(CDC* pDC, CRect rect, int radius, COLORREF color)
{
	UiDraw::RoundedRect(pDC, rect, radius, color, CLR_INVALID, 0);
}

void CModernScrollBar::OnPaint()
{
	CPaintDC dc(this);
	
	CRect clientRect;
	GetClientRect(&clientRect);
	
	CMemDCEx memDC(&dc, clientRect);
	
	COLORREF trackColor = UiTheme().IsHighContrast() ?
		UiTheme().ScrollBarTrack() : m_trackColor;
	memDC.FillSolidRect(&clientRect, trackColor);
	
	// Get thumb rect
	CRect thumbRect = GetThumbRect();
	
	if (!thumbRect.IsRectEmpty())
	{
		int radius = m_cornerRadius;
		if (m_pDPI)
			radius = m_pDPI->Scale(m_cornerRadius);
		if (m_isMouseOver || m_isDragging)
			radius = max(radius, min(thumbRect.Width(), thumbRect.Height()) / 2);
		
		COLORREF thumbColor = (m_isMouseOver || m_isDragging) ?
			m_thumbHoverColor : m_thumbColor;
		if (UiTheme().IsHighContrast())
		{
			thumbColor = (m_isMouseOver || m_isDragging) ?
				UiTheme().ScrollBarThumbHover() : UiTheme().ScrollBarThumb();
		}
		thumbColor = BlendColor(thumbColor, trackColor, m_opacity);
		
		DrawRoundedRect(&memDC, thumbRect, radius, thumbColor);
	}
}

BOOL CModernScrollBar::OnEraseBkgnd(CDC* pDC)
{
	// Don't erase - we handle it in OnPaint
	return TRUE;
}

void CModernScrollBar::OnMouseMove(UINT nFlags, CPoint point)
{
	if (!m_trackingMouse)
	{
		TRACKMOUSEEVENT tme = { sizeof(TRACKMOUSEEVENT) };
		tme.dwFlags = TME_LEAVE | TME_HOVER;
		tme.hwndTrack = m_hWnd;
		tme.dwHoverTime = 1; // Immediate hover detection
		TrackMouseEvent(&tme);
		m_trackingMouse = true;
	}

	CRect clientRect;
	GetClientRect(&clientRect);
	bool wasMouseOver = m_isMouseOver;
	m_isMouseOver = clientRect.PtInRect(point);
	
	if (wasMouseOver != m_isMouseOver)
	{
		// Update scrollbar size when hover state changes
		UpdateScrollBar();
	}

	if (m_isDragging && m_pListCtrl)
	{
		if (m_orientation == ScrollBarOrientation::Vertical)
		{
			int deltaY = point.y - m_dragStartPos;
			ScrollToPosition(m_dragStartScrollPos + deltaY);
		}
		else
		{
			int deltaX = point.x - m_dragStartPos;
			ScrollToPosition(m_dragStartScrollPos + deltaX);
		}
	}

	CWnd::OnMouseMove(nFlags, point);
}

void CModernScrollBar::OnMouseLeave()
{
	m_trackingMouse = false;
	
	if (m_isMouseOver)
	{
		m_isMouseOver = false;
		// Update scrollbar size when hover state changes (shrink back)
		UpdateScrollBar();

		// If we leave the scrollbar area and we are in auto-hide mode, 
		// restart the timer to hide it eventually
		if (m_isVisible && !CGetSetOptions::m_showScrollBar && !m_isDragging)
		{
			SetTimer(TIMER_AUTO_HIDE, UiTheme().ScrollBarAutoHideMs(), NULL);
		}
	}

	CWnd::OnMouseLeave();
}

LRESULT CModernScrollBar::OnMouseHover(WPARAM wParam, LPARAM lParam)
{
	return 0;
}

void CModernScrollBar::OnLButtonDown(UINT nFlags, CPoint point)
{
	CRect thumbRect = GetThumbRect();
	
	if (thumbRect.PtInRect(point))
	{
		// Start dragging the thumb
		m_isDragging = true;
		m_dragStartPos = (m_orientation == ScrollBarOrientation::Vertical) ? point.y : point.x;
		
		// Get current scroll position in thumb coordinates
		m_dragStartScrollPos = (m_orientation == ScrollBarOrientation::Vertical) ? thumbRect.top : thumbRect.left;
		
		SetCapture();
		Invalidate();
	}
	else
	{
		// Click on track - page scroll
		if (m_orientation == ScrollBarOrientation::Vertical)
		{
			if (point.y < thumbRect.top)
			{
				m_pListCtrl->SendMessage(WM_VSCROLL, SB_PAGEUP, 0);
			}
			else if (point.y > thumbRect.bottom)
			{
				m_pListCtrl->SendMessage(WM_VSCROLL, SB_PAGEDOWN, 0);
			}
		}
		else
		{
			// For horizontal, use Scroll with page width
			CRect clientRect;
			m_pListCtrl->GetClientRect(&clientRect);
			int pageWidth = clientRect.Width();
			
			if (point.x < thumbRect.left)
			{
				m_pListCtrl->Scroll(CSize(-pageWidth, 0));
			}
			else if (point.x > thumbRect.right)
			{
				m_pListCtrl->Scroll(CSize(pageWidth, 0));
			}
		}
		
		UpdateScrollBar();
	}

	CWnd::OnLButtonDown(nFlags, point);
}

void CModernScrollBar::OnLButtonUp(UINT nFlags, CPoint point)
{
	if (m_isDragging)
	{
		m_isDragging = false;
		ReleaseCapture();
		Invalidate();
		if (m_isVisible && !CGetSetOptions::m_showScrollBar)
		{
			SetTimer(TIMER_AUTO_HIDE, UiTheme().ScrollBarAutoHideMs(), NULL);
		}
	}

	CWnd::OnLButtonUp(nFlags, point);
}

void CModernScrollBar::ScrollToPosition(int thumbPos)
{
	if (!m_pListCtrl)
		return;

	CRect clientRect;
	GetClientRect(&clientRect);

	int scrollBarType = (m_orientation == ScrollBarOrientation::Vertical) ? SB_VERT : SB_HORZ;
	
	SCROLLINFO si = { sizeof(SCROLLINFO) };
	si.fMask = SIF_ALL;
	m_pListCtrl->GetScrollInfo(scrollBarType, &si);

	if (si.nMax <= si.nMin || si.nPage <= 0)
		return;

	int inset = m_pDPI ?
		m_pDPI->Scale(UiTheme().ScrollBarInset()) :
		UiTheme().ScrollBarInset();
	int trackSize = ((m_orientation == ScrollBarOrientation::Vertical) ?
		clientRect.Height() : clientRect.Width()) - inset * 2;
	if (trackSize <= 0)
		return;
	int totalRange = si.nMax - si.nMin + 1;
	
	// Calculate thumb size
	int thumbSize = (int)((double)si.nPage / totalRange * trackSize);
	int minSize = m_minThumbSize;
	if (m_pDPI)
		minSize = m_pDPI->Scale(m_minThumbSize);
	if (thumbSize < minSize)
		thumbSize = minSize;
	if (thumbSize > trackSize)
		thumbSize = trackSize;

	// Clamp thumb position
	if (thumbPos < 0) thumbPos = 0;
	if (thumbPos > trackSize - thumbSize)
		thumbPos = trackSize - thumbSize;

	// Calculate new scroll position
	int scrollableTrack = trackSize - thumbSize;
	int scrollableRange = totalRange - si.nPage;
	
	int newPos = si.nMin;
	if (scrollableTrack > 0)
		newPos = si.nMin + (int)((double)thumbPos / scrollableTrack * scrollableRange);

	if (m_orientation == ScrollBarOrientation::Vertical)
	{
		// Use Scroll method for smoother scrolling with virtual lists
		CQListCtrl* pQListCtrl = (CQListCtrl*)m_pListCtrl;
		int rowHeight = pQListCtrl->GetRowHeight();
		int currentTop = m_pListCtrl->GetTopIndex();
		
		if (rowHeight > 0)
		{
			int deltaRows = newPos - currentTop;
			int deltaPixels = deltaRows * rowHeight;
			
			if (deltaPixels != 0)
			{
				m_pListCtrl->Scroll(CSize(0, deltaPixels));
			}
		}
		else
		{
			// Fallback if row height is not available
			si.fMask = SIF_POS;
			si.nPos = newPos;
			m_pListCtrl->SetScrollInfo(SB_VERT, &si);
			m_pListCtrl->SendMessage(WM_VSCROLL, MAKEWPARAM(SB_THUMBPOSITION, newPos), 0);
		}
	}
	else
	{
		// Horizontal scrolling - use Scroll method for more reliable scrolling
		int currentScrollPos = m_pListCtrl->GetScrollPos(SB_HORZ);
		int deltaPixels = newPos - currentScrollPos;
		
		if (deltaPixels != 0)
		{
			m_pListCtrl->Scroll(CSize(deltaPixels, 0));
		}
	}
	
	Invalidate();
}

void CModernScrollBar::Show(bool animate)
{
	m_isVisible = true;
	ShowWindow(SW_SHOWNA);
	KillTimer(TIMER_AUTO_HIDE);

	if (!CGetSetOptions::m_showScrollBar)
	{
		SetTimer(TIMER_AUTO_HIDE, UiTheme().ScrollBarAutoHideMs(), NULL);
	}

	if (animate && AnimationsEnabled())
	{
		if (m_opacity < 1.0)
		{
			BeginFade(true);
		}
	}
	else
	{
		KillTimer(TIMER_FADE);
		m_opacity = 1.0;
	}

	UpdateScrollBar();
}

void CModernScrollBar::Hide(bool animate)
{
	m_isVisible = false;
	KillTimer(TIMER_AUTO_HIDE);

	if (animate && AnimationsEnabled() && IsWindowVisible() && m_opacity > 0.0)
	{
		BeginFade(false);
	}
	else
	{
		KillTimer(TIMER_FADE);
		m_opacity = 0.0;
		ShowWindow(SW_HIDE);
	}
}

void CModernScrollBar::OnTimer(UINT_PTR nIDEvent)
{
	if (nIDEvent == TIMER_AUTO_HIDE)
	{
		// Don't hide if mouse is over or dragging
		if (!m_isMouseOver && !m_isDragging)
		{
			if (!CGetSetOptions::m_showScrollBar)
			{
				Hide(true);
			}
		}
		KillTimer(TIMER_AUTO_HIDE);
	}
	else if (nIDEvent == TIMER_FADE)
	{
		DWORD elapsed = GetTickCount() - m_fadeStartTick;
		int duration = max(1, UiTheme().MotionFastMs());
		double progress = min(1.0, (double)elapsed / duration);
		double eased = 1.0 - (1.0 - progress) * (1.0 - progress);

		if (m_fadeIn)
		{
			m_opacity = m_fadeStartOpacity +
				(1.0 - m_fadeStartOpacity) * eased;
		}
		else
		{
			m_opacity = m_fadeStartOpacity * (1.0 - eased);
		}

		Invalidate();
		if (progress >= 1.0)
		{
			KillTimer(TIMER_FADE);
			m_opacity = m_fadeIn ? 1.0 : 0.0;
			if (!m_fadeIn)
			{
				ShowWindow(SW_HIDE);
			}
		}
	}

	CWnd::OnTimer(nIDEvent);
}

bool CModernScrollBar::AnimationsEnabled() const
{
	if (CDarkMode::IsHighContrast() ||
		GetSystemMetrics(SM_REMOTESESSION))
	{
		return false;
	}

	BOOL enabled = TRUE;
	if (!SystemParametersInfo(SPI_GETCLIENTAREAANIMATION, 0, &enabled, 0))
	{
		enabled = TRUE;
	}
	return enabled != FALSE;
}

void CModernScrollBar::BeginFade(bool fadeIn)
{
	m_fadeIn = fadeIn;
	m_fadeStartOpacity = m_opacity;
	m_fadeStartTick = GetTickCount();
	KillTimer(TIMER_FADE);
	SetTimer(TIMER_FADE, 16, NULL);
}

COLORREF CModernScrollBar::BlendColor(
	COLORREF foreground,
	COLORREF background,
	double opacity)
{
	double amount = max(0.0, min(1.0, opacity));
	auto blendChannel = [amount](int foreground, int background)
	{
		return static_cast<int>(
			background + (foreground - background) * amount + 0.5);
	};

	return RGB(
		blendChannel(GetRValue(foreground), GetRValue(background)),
		blendChannel(GetGValue(foreground), GetGValue(background)),
		blendChannel(GetBValue(foreground), GetBValue(background)));
}
