#include "stdafx.h"
#include ".\dittowindow.h"
#include "CP_Main.h"
#include "Options.h"
#include "UiDraw.h"
#include "UiTheme.h"
#include "UiWindow.h"
#include <ShellScalingAPI.h>

CDittoWindow::CDittoWindow(void)
{
	const CUiTheme& theme = UiTheme();
	m_captionHeightLogical = theme.CaptionHeight();
	m_captionFontSize = theme.FontSizeWindowTitle();
	m_captionBorderWidth = m_dpi.Scale(m_captionHeightLogical);

	m_borderSize = m_dpi.Scale(theme.BorderWidth());
	m_bMouseOverChevron = false;
	m_bMouseDownOnChevron = false;
	m_bMouseDownOnClose = false;
	m_bMouseOverClose = false;
	m_bMouseDownOnMinimize = false;
	m_bMouseOverMinimize = false;
	m_bMouseDownOnMaximize = false;
	m_bMouseOverMaximize = false;
	m_bDrawClose = true;
	m_bDrawChevron = true;
	m_bDrawMaximize = true;
	m_bDrawMinimize = true;
	m_bMinimized = false;
	m_crCloseBT.SetRectEmpty();
	m_crChevronBT.SetRectEmpty();
	m_crMaximizeBT.SetRectEmpty();
	m_crMinimizeBT.SetRectEmpty();
	m_CaptionColorLeft = theme.CaptionLeft();
	m_CaptionColorRight = theme.CaptionRight();
	m_CaptionTextColor = theme.CaptionText();
	m_border = theme.Border();
	m_sendWMClose = true;
	m_customWindowTitle = _T("");
	m_useCustomWindowTitle = false;
	m_buttonDownOnCaption = false;
	m_crFullSizeWindow.SetRectEmpty();	
	m_captionPosition = CAPTION_RIGHT;
	
}

CDittoWindow::~CDittoWindow(void)
{
}

void CDittoWindow::DoCreate(CWnd *pWnd)
{
	m_dpi.SetHwnd(pWnd->m_hWnd);
	m_borderSize = m_dpi.Scale(UiTheme().BorderWidth());
	m_captionBorderWidth = m_dpi.Scale(m_captionHeightLogical);

	LOGFONT lf = { 0 };
	lf.lfHeight = -m_dpi.Scale(m_captionFontSize);
	lf.lfWeight = FW_SEMIBOLD;
	lf.lfCharSet = DEFAULT_CHARSET;
	lf.lfQuality = CLEARTYPE_QUALITY;
	_tcsncpy_s(lf.lfFaceName, UiTheme().FontFamilyUi(), _TRUNCATE);
	m_HorFont.CreateFontIndirect(&lf);

	lf.lfEscapement = -900;
	m_VertFont.CreateFontIndirect(&lf);

	SetTitleTextHeight(pWnd);
	
	m_closeButton.LoadStdImageDPI(m_dpi.GetDPI(), Close_Black_16_16, Close_Black_20_20, Close_Black_24_24, Close_Black_28, Close_Black_32_32, _T("PNG"), close_36, close_40, close_44, close_48, close_52, close_56);
	m_chevronRightButton.LoadStdImageDPI(m_dpi.GetDPI(), ChevronRight_Black_16_16, ChevronRight_Black_20_20, ChevronRight_Black_24_24, ChevronRight_Black_28, ChevronRight_Black_32_32, _T("PNG"), ChevronRight_Black_36, ChevronRight_Black_40, ChevronRight_Black_44, ChevronRight_Black_48, ChevronRight_Black_52, ChevronRight_Black_56);
	m_chevronLeftButton.LoadStdImageDPI(m_dpi.GetDPI(), ChevronLeft_Black_16_16, ChevronLeft_Black_20_20, ChevronLeft_Black_24_24, ChevronLeft_Black_28, ChevronLeft_Black_32_32, _T("PNG"), ChevronLeft_Black_36, ChevronLeft_Black_40, ChevronLeft_Black_44, ChevronLeft_Black_48, ChevronLeft_Black_52, ChevronLeft_Black_56);
	m_maximizeButton.LoadStdImageDPI(m_dpi.GetDPI(), IDB_MAXIMIZE_16_16, maximize_20, maximize_24, maximize_28, maximize_32, _T("PNG"), maximize_36, maximize_40, maximize_44, maximize_48, maximize_52, maximize_56);
	m_minimizeButton.LoadStdImageDPI(m_dpi.GetDPI(), minimize_16, minimize_20, minimize_24, minimize_28, minimize_32, _T("PNG"), minimize_36, minimize_40, minimize_44, minimize_48, minimize_52, minimize_56);
	//m_windowIcon.LoadStdImageDPI(NewWindowIcon_24_14, NewWindowIcon_30, NewWindowIcon_36, NewWindowIcon_48, _T("PNG"));
}

void CDittoWindow::DoNcCalcSize(BOOL bCalcValidRects, NCCALCSIZE_PARAMS FAR* lpncsp)
{
	//Decrease the client area	
	if (m_captionPosition == CAPTION_LEFT)
		lpncsp->rgrc[0].left += m_captionBorderWidth;
	else
		lpncsp->rgrc[0].left += m_borderSize;

	if (m_captionPosition == CAPTION_TOP)
		lpncsp->rgrc[0].top += m_captionBorderWidth;
	else
		lpncsp->rgrc[0].top += m_borderSize;

	if (m_captionPosition == CAPTION_RIGHT)
		lpncsp->rgrc[0].right -= m_captionBorderWidth;
	else
		lpncsp->rgrc[0].right -= m_borderSize;

	if (m_captionPosition == CAPTION_BOTTOM)
		lpncsp->rgrc[0].bottom -= m_captionBorderWidth;
	else
		lpncsp->rgrc[0].bottom -= m_borderSize;
}

UINT CDittoWindow::DoNcHitTest(CWnd *pWnd, CPoint point) 
{
	CRect crWindow;
	pWnd->GetWindowRect(crWindow);

	if(crWindow.PtInRect(point) == false)
	{
		return -1;
	}
	
	int x = point.x - crWindow.left;
	int y = point.y - crWindow.top;

	CPoint myLocal(x, y);

	//http://stackoverflow.com/questions/521147/the-curious-problem-of-the-missing-wm-nclbuttonup-message-when-a-window-isnt-ma
	//workaround for l button up not coming after a lbutton down
	if (m_crCloseBT.PtInRect(myLocal) ||
		m_crChevronBT.PtInRect(myLocal) ||
		m_crMinimizeBT.PtInRect(myLocal) ||
		m_crMaximizeBT.PtInRect(myLocal))
	{
		return HTBORDER;;
	}

	if(m_bMinimized == false)
	{
		if ((point.y < crWindow.top + m_borderSize * 4) &&
			(point.x < crWindow.left + m_borderSize * 4))
			return HTTOPLEFT;
		else if ((point.y < crWindow.top + m_borderSize * 4) &&
			(point.x > crWindow.right - m_borderSize * 4))
			return HTTOPRIGHT;
		else if ((point.y > crWindow.bottom - m_borderSize * 4) &&
			(point.x > crWindow.right - m_borderSize * 4))
			return HTBOTTOMRIGHT;
		else if ((point.y > crWindow.bottom - m_borderSize * 4) &&
			(point.x < crWindow.left + m_borderSize * 4))
			return HTBOTTOMLEFT;
	}

	if((((m_captionPosition == CAPTION_TOP) || (m_captionPosition == CAPTION_BOTTOM)) &&
		(m_bMinimized)) == false)
	{
		if (point.y < crWindow.top + m_borderSize * 2)
			return HTTOP;
		if (point.y > crWindow.bottom - m_borderSize * 2)
			return HTBOTTOM;
	}

	if((((m_captionPosition == CAPTION_LEFT) || (m_captionPosition == CAPTION_RIGHT)) &&
		(m_bMinimized)) == false)
	{
		if (point.x > crWindow.right - m_borderSize * 2)
			return HTRIGHT;
		if (point.x < crWindow.left + m_borderSize * 2)
			return HTLEFT;
	}

	if (m_captionPosition == CAPTION_RIGHT)
	{
		if (point.x > crWindow.right - m_captionBorderWidth)
			return HTCAPTION;
	}
	if (m_captionPosition == CAPTION_BOTTOM)
	{
		if(point.y > crWindow.bottom - m_captionBorderWidth)
			return HTCAPTION;
	}
	if (m_captionPosition == CAPTION_LEFT)
	{
		if (point.x < crWindow.left + m_captionBorderWidth)
			return HTCAPTION;
	}
	if (m_captionPosition == CAPTION_TOP)
	{
		if (point.y < crWindow.top + m_captionBorderWidth)
			return HTCAPTION;
	}

	return -1;
}

void CDittoWindow::DoNcPaint(CWnd *pWnd)
{
	CWindowDC dc(pWnd);
	const CUiTheme& theme = UiTheme();
	CRect windowRect;
	pWnd->GetWindowRect(windowRect);
	windowRect.OffsetRect(-windowRect.left, -windowRect.top);

	const int captionSize = m_captionBorderWidth;
	const int border = m_dpi.Scale(theme.BorderWidth());
	const int buttonSize = m_dpi.Scale(theme.CaptionButtonSize());
	const int iconSize = m_dpi.Scale(theme.CaptionIconSize());
	const int padding = m_dpi.Scale(theme.FieldGap());
	const int gap = m_dpi.Scale(theme.FieldGap() / 2);
	const bool enabled = pWnd->IsWindowEnabled() != FALSE;

	CRect captionRect;
	switch (m_captionPosition)
	{
	case CAPTION_LEFT:
		captionRect.SetRect(0, 0, captionSize, windowRect.bottom);
		break;
	case CAPTION_BOTTOM:
		captionRect.SetRect(0, windowRect.bottom - captionSize, windowRect.right,
			windowRect.bottom);
		break;
	case CAPTION_RIGHT:
		captionRect.SetRect(windowRect.right - captionSize, 0, windowRect.right,
			windowRect.bottom);
		break;
	case CAPTION_TOP:
	default:
		captionRect.SetRect(0, 0, windowRect.right, captionSize);
		break;
	}

	if (m_captionPosition == CAPTION_TOP || m_captionPosition == CAPTION_BOTTOM)
	{
		CRect leftHalf(captionRect);
		leftHalf.right = captionRect.CenterPoint().x;
		dc.FillSolidRect(leftHalf, m_CaptionColorLeft);
		dc.FillSolidRect(CRect(leftHalf.right, captionRect.top, captionRect.right,
			captionRect.bottom), m_CaptionColorRight);
	}
	else
	{
		CRect topHalf(captionRect);
		topHalf.bottom = captionRect.CenterPoint().y;
		dc.FillSolidRect(topHalf, m_CaptionColorLeft);
		dc.FillSolidRect(CRect(captionRect.left, topHalf.bottom, captionRect.right,
			captionRect.bottom), m_CaptionColorRight);
	}

	m_crCloseBT.SetRectEmpty();
	m_crChevronBT.SetRectEmpty();
	m_crMaximizeBT.SetRectEmpty();
	m_crMinimizeBT.SetRectEmpty();
	m_crWindowIconBT.SetRectEmpty();

	auto drawButton = [&](CRect buttonRect, UiDraw::Glyph glyph, bool hover,
		bool pressed, bool buttonEnabled, bool closeButton)
	{
		CRect glyphRect(buttonRect.CenterPoint().x - iconSize / 2,
			buttonRect.CenterPoint().y - iconSize / 2,
			buttonRect.CenterPoint().x + (iconSize + 1) / 2,
			buttonRect.CenterPoint().y + (iconSize + 1) / 2);
		if (!buttonEnabled)
		{
			UiDraw::DrawGlyph(&dc, glyphRect, glyph, theme.TextDisabled(),
				m_dpi.GetDPI(), theme.IconStrokeTenths());
			return;
		}

		COLORREF background = CLR_INVALID;
		COLORREF foreground = theme.TextSecondary();
		if (pressed)
		{
			background = closeButton ? theme.DangerFill() : theme.SurfaceSunken();
			foreground = closeButton ? theme.DangerOn() : theme.TextPrimary();
		}
		else if (hover)
		{
			background = closeButton ? theme.CloseHoverBg() : theme.Hover();
			foreground = closeButton ? theme.CloseHoverOn() : theme.TextPrimary();
		}

		if (background != CLR_INVALID)
		{
			UiDraw::RoundedRect(&dc, buttonRect, m_dpi.Scale(theme.RadiusControl()),
				background, CLR_INVALID, 0);
		}

		UiDraw::DrawGlyph(&dc, glyphRect, glyph, foreground, m_dpi.GetDPI(),
			theme.IconStrokeTenths());
	};

	BOOL vertical = FALSE;
	if (m_captionPosition == CAPTION_TOP || m_captionPosition == CAPTION_BOTTOM)
	{
		int right = windowRect.right - padding;
		int top = captionRect.top + (captionSize - buttonSize) / 2;
		auto nextButton = [&]()
		{
			CRect result(right - buttonSize, top, right, top + buttonSize);
			right -= buttonSize + gap;
			return result;
		};

		if (m_bDrawClose)
		{
			m_crCloseBT = nextButton();
			drawButton(m_crCloseBT, UiDraw::Glyph::Close, m_bMouseOverClose,
				m_bMouseDownOnClose, enabled && m_sendWMClose, true);
		}
		if (m_bDrawChevron)
		{
			m_crChevronBT = nextButton();
			drawButton(m_crChevronBT,
				m_bMinimized ? UiDraw::Glyph::ChevronLeft : UiDraw::Glyph::ChevronRight,
				m_bMouseOverChevron, m_bMouseDownOnChevron, enabled, false);
		}
		if (m_bDrawMaximize)
		{
			m_crMaximizeBT = nextButton();
			drawButton(m_crMaximizeBT, UiDraw::Glyph::ChevronUp,
				m_bMouseOverMaximize, m_bMouseDownOnMaximize, enabled, false);
		}
		if (m_bDrawMinimize)
		{
			m_crMinimizeBT = nextButton();
			drawButton(m_crMinimizeBT, UiDraw::Glyph::ChevronDown,
				m_bMouseOverMinimize, m_bMouseDownOnMinimize, enabled, false);
		}
	}
	else
	{
		vertical = TRUE;
		int left = captionRect.left + (captionSize - buttonSize) / 2;
		int top = captionRect.top + padding;
		auto nextButton = [&]()
		{
			CRect result(left, top, left + buttonSize, top + buttonSize);
			top += buttonSize + gap;
			return result;
		};

		if (m_bDrawClose)
		{
			m_crCloseBT = nextButton();
			drawButton(m_crCloseBT, UiDraw::Glyph::Close, m_bMouseOverClose,
				m_bMouseDownOnClose, enabled && m_sendWMClose, true);
		}
		if (m_bDrawChevron)
		{
			m_crChevronBT = nextButton();
			drawButton(m_crChevronBT,
				m_bMinimized ? UiDraw::Glyph::ChevronLeft : UiDraw::Glyph::ChevronRight,
				m_bMouseOverChevron, m_bMouseDownOnChevron, enabled, false);
		}
		if (m_bDrawMaximize)
		{
			m_crMaximizeBT = nextButton();
			drawButton(m_crMaximizeBT, UiDraw::Glyph::ChevronUp,
				m_bMouseOverMaximize, m_bMouseDownOnMaximize, enabled, false);
		}
		if (m_bDrawMinimize)
		{
			m_crMinimizeBT = nextButton();
			drawButton(m_crMinimizeBT, UiDraw::Glyph::ChevronDown,
				m_bMouseOverMinimize, m_bMouseDownOnMinimize, enabled, false);
		}
	}

	CString title = m_customWindowTitle;
	if (!m_useCustomWindowTitle)
		pWnd->GetWindowText(title);

	dc.SetBkMode(TRANSPARENT);
	dc.SetTextColor(m_CaptionTextColor);
	CFont* oldFont = dc.SelectObject(&m_HorFont);
	if (!vertical)
	{
		CRect textRect(captionRect);
		textRect.left += padding;
		textRect.right = windowRect.right - padding;
		if (!m_crMinimizeBT.IsRectEmpty())
			textRect.right = min(textRect.right, m_crMinimizeBT.left - padding);
		if (!m_crMaximizeBT.IsRectEmpty())
			textRect.right = min(textRect.right, m_crMaximizeBT.left - padding);
		if (!m_crChevronBT.IsRectEmpty())
			textRect.right = min(textRect.right, m_crChevronBT.left - padding);
		if (!m_crCloseBT.IsRectEmpty())
			textRect.right = min(textRect.right, m_crCloseBT.left - padding);

		UiDraw::DrawTextEllipsis(&dc, title, textRect, theme.FontFamilyUi(),
			theme.FontSizeWindowTitle(), FW_SEMIBOLD, m_CaptionTextColor,
			m_dpi.GetDPI(), DT_LEFT | DT_VCENTER | DT_SINGLELINE);
	}
	else
	{
		CRect textRect(captionRect);
		textRect.top += padding;
		textRect.bottom -= padding;
		if (!m_crMinimizeBT.IsRectEmpty())
			textRect.top = max(textRect.top, m_crMinimizeBT.bottom + padding);
		if (!m_crMaximizeBT.IsRectEmpty())
			textRect.top = max(textRect.top, m_crMaximizeBT.bottom + padding);
		if (!m_crChevronBT.IsRectEmpty())
			textRect.top = max(textRect.top, m_crChevronBT.bottom + padding);
		if (!m_crCloseBT.IsRectEmpty())
			textRect.top = max(textRect.top, m_crCloseBT.bottom + padding);

		Gdiplus::Graphics graphics(dc.GetSafeHdc());
		graphics.SetTextRenderingHint(Gdiplus::TextRenderingHintClearTypeGridFit);
		Gdiplus::FontFamily family(theme.FontFamilyUi());
		Gdiplus::Font font(&family,
			static_cast<Gdiplus::REAL>(m_dpi.Scale(theme.FontSizeWindowTitle())),
			Gdiplus::FontStyleRegular, Gdiplus::UnitPixel);
		Gdiplus::SolidBrush brush(Gdiplus::Color(255, GetRValue(m_CaptionTextColor),
			GetGValue(m_CaptionTextColor), GetBValue(m_CaptionTextColor)));
		Gdiplus::StringFormat format;
		format.SetAlignment(Gdiplus::StringAlignmentNear);
		format.SetLineAlignment(Gdiplus::StringAlignmentCenter);
		format.SetTrimming(Gdiplus::StringTrimmingEllipsisCharacter);
		format.SetFormatFlags(Gdiplus::StringFormatFlagsNoWrap);

		const Gdiplus::REAL length = static_cast<Gdiplus::REAL>(textRect.Height());
		const Gdiplus::REAL textHeight =
			static_cast<Gdiplus::REAL>(m_dpi.Scale(theme.FontSizeWindowTitle()));
		graphics.TranslateTransform(
			static_cast<Gdiplus::REAL>(captionRect.CenterPoint().x),
			m_captionPosition == CAPTION_RIGHT ?
				static_cast<Gdiplus::REAL>(textRect.bottom) :
				static_cast<Gdiplus::REAL>(textRect.top));
		graphics.RotateTransform(m_captionPosition == CAPTION_RIGHT ? -90.0f : 90.0f);
		graphics.DrawString(title, -1, &font,
			Gdiplus::RectF(0.0f, -textHeight / 2.0f, length, textHeight),
			&format, &brush);
	}
	dc.SelectObject(oldFont);

	if (!UiWindow::IsWindows11OrGreater())
	{
		UiWindow::DrawFallbackBorder(&dc, windowRect, m_border);
	}
}

void CDittoWindow::DrawChevronBtn(CWindowDC &dc, CWnd *pWnd)
{
	if(m_bDrawChevron == false)
	{
		return;
	}
		
	if(this->m_bMinimized)
	{
		m_chevronLeftButton.Draw(&dc, m_dpi, pWnd, m_crChevronBT, m_bMouseOverChevron, m_bMouseDownOnChevron);
	}
	else
	{
		m_chevronRightButton.Draw(&dc, m_dpi, pWnd, m_crChevronBT, m_bMouseOverChevron, m_bMouseDownOnChevron);
	}
}

void CDittoWindow::DrawWindowIcon(CWindowDC &dc, CWnd *pWnd)
{
	//m_windowIcon.Draw(&dc, pWnd, m_crWindowIconBT.left, m_crWindowIconBT.top, false, false);
}

void CDittoWindow::DrawCloseBtn(CWindowDC &dc, CWnd *pWnd)
{
	if(m_bDrawClose == false)
	{
		return;
	}
	
	m_closeButton.Draw(&dc, m_dpi, pWnd, m_crCloseBT, m_bMouseOverClose, m_bMouseDownOnClose);
}

void CDittoWindow::DrawMinimizeBtn(CWindowDC &dc, CWnd *pWnd)
{
	if(m_bDrawMinimize == false)
	{
		return;
	}

	m_minimizeButton.Draw(&dc, m_dpi, pWnd, m_crMinimizeBT, m_bMouseOverMinimize, m_bMouseDownOnMinimize);
}

void CDittoWindow::DrawMaximizeBtn(CWindowDC &dc, CWnd *pWnd)
{
	if(m_bDrawMaximize == false)
	{
		return;
	}

	m_maximizeButton.Draw(&dc, m_dpi, pWnd, m_crMaximizeBT, m_bMouseOverMaximize, m_bMouseDownOnMaximize);
}

int CDittoWindow::DoNcLButtonDown(CWnd *pWnd, UINT nHitTest, CPoint point) 
{
	if (!pWnd->IsWindowEnabled())
		return 0;

	switch (nHitTest)
	{
	case HTCAPTION:
		m_buttonDownOnCaption = true;
		break;
	default:
		m_buttonDownOnCaption = false;
	}

	int buttonPressed = 0;
	//ReleaseCapture();
	CPoint clPoint(point);
	pWnd->ScreenToClient(&clPoint);

	if (m_captionPosition == CAPTION_LEFT)
	{
		clPoint.x += m_captionBorderWidth;
	}
	else
	{
		clPoint.x += m_borderSize;
	}

	if (m_captionPosition == CAPTION_TOP)
	{
		clPoint.y += m_captionBorderWidth;
	}
	else
	{
		clPoint.y += m_borderSize;
	}	

	if(m_crCloseBT.PtInRect(clPoint))
	{
		m_bMouseDownOnClose = true;
		//InvalidateRect(pWnd->m_hWnd, m_crCloseBT, TRUE);
		//pWnd->InvalidateRect(m_crCloseBT);
		//pWnd->UpdateWindow();
		//DoNcPaint(pWnd);
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
		buttonPressed = BUTTON_CLOSE;
	}
	else if(m_crChevronBT.PtInRect(clPoint))
	{
		m_bMouseDownOnChevron = true;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
		buttonPressed = BUTTON_CHEVRON;
	}
	else if(m_crMinimizeBT.PtInRect(clPoint))
	{
		m_bMouseDownOnMinimize = true;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
		buttonPressed = BUTTON_MINIMIZE;
	}
	else if(m_crMaximizeBT.PtInRect(clPoint))
	{
		m_bMouseDownOnMaximize = true;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
		buttonPressed = BUTTON_MAXIMIZE;
	}
	else if(m_bMinimized)
	{
		//MinMaxWindow(FORCE_MAX);
	}

	return buttonPressed;
}

long CDittoWindow::DoNcLButtonUp(CWnd *pWnd, UINT nHitTest, CPoint point) 
{
	m_buttonDownOnCaption = false;

	CRect crWindow;
	pWnd->GetWindowRect(crWindow);

	CPoint localPoint(point.x - crWindow.left, point.y - crWindow.top);

	long lRet = 0;
	if(m_bMouseDownOnClose)
	{
		m_bMouseDownOnClose = false;
		m_bMouseOverClose = false;

		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
		
		if(m_crCloseBT.PtInRect(localPoint))
		{
			if(m_sendWMClose)
			{
				pWnd->SendMessage(WM_CLOSE, 0, 0);
			}
			lRet = BUTTON_CLOSE;
		}
	}
	else if(m_bMouseDownOnChevron)
	{
		m_bMouseDownOnChevron = false;
		m_bMouseOverChevron = false;

		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);

		if(m_crChevronBT.PtInRect(localPoint))
		{
			lRet = BUTTON_CHEVRON;
		}
	}
	else if(m_bMouseDownOnMinimize)
	{
		m_bMouseDownOnMinimize = false;
		m_bMouseOverMinimize = false;

		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);

		if(m_crMinimizeBT.PtInRect(localPoint))
		{
			pWnd->ShowWindow(SW_MINIMIZE);
			lRet = BUTTON_MINIMIZE;
		}
	}
	else if(m_bMouseDownOnMaximize)
	{
		m_bMouseDownOnMaximize = false;
		m_bMouseOverMaximize = false;

		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);

		if(m_crMaximizeBT.PtInRect(localPoint))
		{
			if(pWnd->GetStyle() & WS_MAXIMIZE)
				pWnd->ShowWindow(SW_RESTORE);
			else
				pWnd->ShowWindow(SW_SHOWMAXIMIZED);

			lRet = BUTTON_MAXIMIZE;
		}
	}

	return lRet;
}

void CDittoWindow::DoNcMouseMove(CWnd *pWnd, UINT nHitTest, CPoint point) 
{
	CRect crWindow;
	pWnd->GetWindowRect(crWindow);

	CPoint localPoint(point.x - crWindow.left, point.y - crWindow.top);

	if(pWnd->IsWindowEnabled() && m_crCloseBT.PtInRect(localPoint))
	{
		m_bMouseOverClose = true;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
	}
	else if(m_bMouseOverClose)
	{
		m_bMouseOverClose = false;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
	}

	if(pWnd->IsWindowEnabled() && m_crChevronBT.PtInRect(localPoint))
	{
		m_bMouseOverChevron = true;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
	}
	else if(m_bMouseOverChevron)
	{
		m_bMouseOverChevron = false;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
	}

	if(pWnd->IsWindowEnabled() && m_crMinimizeBT.PtInRect(localPoint))
	{
		m_bMouseOverMinimize = true;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
	}
	else if(m_bMouseOverMinimize)
	{
		m_bMouseOverMinimize = false;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
	}

	if(pWnd->IsWindowEnabled() && m_crMaximizeBT.PtInRect(localPoint))
	{
		m_bMouseOverMaximize = true;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
	}
	else if(m_bMouseOverMaximize)
	{
		m_bMouseOverMaximize = false;
		RedrawWindow(pWnd->m_hWnd, NULL, NULL, RDW_FRAME | RDW_INVALIDATE);
	}
}

bool CDittoWindow::DoPreTranslateMessage(MSG* pMsg) 
{
	return true;
}

void CDittoWindow::SetCaptionOn(CWnd *pWnd, int nPos, bool bOnstartup, int captionSize, int captionFontSize)
{
	m_captionFontSize = captionFontSize;
	m_captionHeightLogical = captionSize;

	m_VertFont.DeleteObject();
	m_HorFont.DeleteObject();

	LOGFONT lf = { 0 };
	lf.lfHeight = -m_dpi.Scale(captionFontSize);
	lf.lfWeight = FW_SEMIBOLD;
	lf.lfCharSet = DEFAULT_CHARSET;
	lf.lfQuality = CLEARTYPE_QUALITY;
	_tcsncpy_s(lf.lfFaceName, UiTheme().FontFamilyUi(), _TRUNCATE);
	m_HorFont.CreateFontIndirect(&lf);

	lf.lfEscapement = -900;
	m_VertFont.CreateFontIndirect(&lf);

	SetTitleTextHeight(pWnd);

	m_captionPosition = nPos;

	int oldWidth = m_captionBorderWidth;
	m_captionBorderWidth = m_dpi.Scale(m_captionHeightLogical);
		
	if(!bOnstartup)
	{
		pWnd->SetWindowPos(NULL, 0, 0, 0, 0, SWP_FRAMECHANGED|SWP_NOMOVE|SWP_NOSIZE|SWP_NOZORDER);
	}

	pWnd->Invalidate();
	pWnd->RedrawWindow();

	if (oldWidth != m_captionBorderWidth)
	{
		::SetWindowPos(pWnd->m_hWnd, NULL, 0, 0, 0, 0, SWP_DRAWFRAME | SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE);
	}
}

void CDittoWindow::SetTitleTextHeight(CWnd *pWnd)
{
	CWindowDC dc(pWnd);
	CFont *pOldFont = dc.SelectObject(&m_HorFont);
	CRect size(0, 0, 0, 0);
	dc.DrawText(_T("W"), size, DT_CALCRECT);
	m_titleTextHeight = size.Height();
	dc.SelectObject(pOldFont);
}

bool CDittoWindow::SetCaptionColors(COLORREF left, COLORREF right, COLORREF border)
{
	m_CaptionColorLeft = left;
	m_CaptionColorRight = right;
	m_border = border;

	return true;
}

void CDittoWindow::SetCaptionTextColor(COLORREF color)
{
	m_CaptionTextColor = color;
}

void CDittoWindow::MinMaxWindow(CWnd *pWnd, long lOption)
{
	if ((m_bMinimized) && (lOption == FORCE_MIN))
		return;

	if ((m_bMinimized == false) && (lOption == FORCE_MAX))
		return;

	if (m_captionPosition == CAPTION_RIGHT)
	{
		if (m_bMinimized == false)
		{
			pWnd->GetWindowRect(m_crFullSizeWindow);
			pWnd->MoveWindow(m_crFullSizeWindow.right - m_captionBorderWidth,
				m_crFullSizeWindow.top, m_captionBorderWidth,
				m_crFullSizeWindow.Height());
			m_bMinimized = true;
			m_TimeMinimized = COleDateTime::GetCurrentTime();
		}
		else
		{
			CRect cr;
			pWnd->GetWindowRect(cr);
			pWnd->MoveWindow(cr.right - m_crFullSizeWindow.Width(),
				cr.top, m_crFullSizeWindow.Width(), cr.Height());

			m_crFullSizeWindow.SetRectEmpty();
			m_bMinimized = false;
			m_TimeMaximized = COleDateTime::GetCurrentTime();
			::SetForegroundWindow(pWnd->GetSafeHwnd());
		}
	}
	if (m_captionPosition == CAPTION_LEFT)
	{
		if (m_bMinimized == false)
		{
			pWnd->GetWindowRect(m_crFullSizeWindow);
			pWnd->MoveWindow(m_crFullSizeWindow.left,
				m_crFullSizeWindow.top, m_captionBorderWidth,
				m_crFullSizeWindow.Height());
			m_bMinimized = true;
			m_TimeMinimized = COleDateTime::GetCurrentTime();
		}
		else
		{
			CRect cr;
			pWnd->GetWindowRect(cr);
			pWnd->MoveWindow(cr.left, cr.top,
				m_crFullSizeWindow.Width(), cr.Height());

			m_crFullSizeWindow.SetRectEmpty();
			m_bMinimized = false;
			m_TimeMaximized = COleDateTime::GetCurrentTime();
			::SetForegroundWindow(pWnd->GetSafeHwnd());
		}
	}
	if (m_captionPosition == CAPTION_TOP)
	{
		if (m_bMinimized == false)
		{
			pWnd->GetWindowRect(m_crFullSizeWindow);
			pWnd->MoveWindow(m_crFullSizeWindow.left,
				m_crFullSizeWindow.top,
				m_crFullSizeWindow.Width(),
				m_captionBorderWidth);
			m_bMinimized = true;
			m_TimeMinimized = COleDateTime::GetCurrentTime();
		}
		else
		{
			CRect cr;
			pWnd->GetWindowRect(cr);
			pWnd->MoveWindow(cr.left, cr.top,
				cr.Width(), m_crFullSizeWindow.Height());

			m_crFullSizeWindow.SetRectEmpty();
			m_bMinimized = false;
			m_TimeMaximized = COleDateTime::GetCurrentTime();
			::SetForegroundWindow(pWnd->GetSafeHwnd());
		}
	}
	if (m_captionPosition == CAPTION_BOTTOM)
	{
		if (m_bMinimized == false)
		{
			pWnd->GetWindowRect(m_crFullSizeWindow);
			pWnd->MoveWindow(m_crFullSizeWindow.left,
				m_crFullSizeWindow.bottom - m_captionBorderWidth,
				m_crFullSizeWindow.Width(),
				m_captionBorderWidth);
			m_bMinimized = true;
			m_TimeMinimized = COleDateTime::GetCurrentTime();
		}
		else
		{
			CRect cr;
			pWnd->GetWindowRect(cr);
			pWnd->MoveWindow(cr.left,
				cr.bottom - m_crFullSizeWindow.Height(),
				cr.Width(), m_crFullSizeWindow.Height());

			m_crFullSizeWindow.SetRectEmpty();
			m_bMinimized = false;
			m_TimeMaximized = COleDateTime::GetCurrentTime();
			::SetForegroundWindow(pWnd->GetSafeHwnd());
		}
	}
}

void CDittoWindow::OnDpiChanged(CWnd *pParent, int dpi)
{
	m_dpi.Update(dpi);

	m_captionBorderWidth = m_dpi.Scale(m_captionHeightLogical);
	m_borderSize = m_dpi.Scale(UiTheme().BorderWidth());

	m_VertFont.DeleteObject();
	m_HorFont.DeleteObject();

	LOGFONT lf = { 0 };
	lf.lfHeight = -m_dpi.Scale(m_captionFontSize);
	lf.lfWeight = FW_SEMIBOLD;
	lf.lfCharSet = DEFAULT_CHARSET;
	lf.lfQuality = CLEARTYPE_QUALITY;
	_tcsncpy_s(lf.lfFaceName, UiTheme().FontFamilyUi(), _TRUNCATE);
	m_HorFont.CreateFontIndirect(&lf);

	lf.lfEscapement = -900;
	m_VertFont.CreateFontIndirect(&lf);

	m_closeButton.Reset();
	m_closeButton.LoadStdImageDPI(m_dpi.GetDPI(), Close_Black_16_16, Close_Black_20_20, Close_Black_24_24, Close_Black_28, Close_Black_32_32, _T("PNG"), close_36, close_40, close_44, close_48, close_52, close_56);

	m_chevronRightButton.Reset();
	m_chevronRightButton.LoadStdImageDPI(m_dpi.GetDPI(), ChevronRight_Black_16_16, ChevronRight_Black_20_20, ChevronRight_Black_24_24, ChevronRight_Black_28, ChevronRight_Black_32_32, _T("PNG"), ChevronRight_Black_36, ChevronRight_Black_40, ChevronRight_Black_44, ChevronRight_Black_48, ChevronRight_Black_52, ChevronRight_Black_56);
	
	m_chevronLeftButton.Reset();
	m_chevronLeftButton.LoadStdImageDPI(m_dpi.GetDPI(), ChevronLeft_Black_16_16, ChevronLeft_Black_20_20, ChevronLeft_Black_24_24, ChevronLeft_Black_28, ChevronLeft_Black_32_32, _T("PNG"), ChevronLeft_Black_36, ChevronLeft_Black_40, ChevronLeft_Black_44, ChevronLeft_Black_48, ChevronLeft_Black_52, ChevronLeft_Black_56);

	m_maximizeButton.Reset();
	m_maximizeButton.LoadStdImageDPI(m_dpi.GetDPI(), IDB_MAXIMIZE_16_16, maximize_20, maximize_24, maximize_28, maximize_32, _T("PNG"), maximize_36, maximize_40, maximize_44, maximize_48, maximize_52, maximize_56);

	m_minimizeButton.Reset();
	m_minimizeButton.LoadStdImageDPI(m_dpi.GetDPI(), minimize_16, minimize_20, minimize_24, minimize_28, minimize_32, _T("PNG"), minimize_36, minimize_40, minimize_44, minimize_48, minimize_52, minimize_56);

	SetTitleTextHeight(pParent);

	/*pParent->SetWindowPos(NULL, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER);

	pParent->Invalidate();
	pParent->RedrawWindow();*/

	//::SetWindowPos(pParent->m_hWnd, NULL, 0, 0, 0, 0, SWP_DRAWFRAME | SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE);
}
