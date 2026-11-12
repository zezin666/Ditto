#include "stdafx.h"
#include "ThemedControls.h"
#include "../UiTheme.h"
#include "../UiDraw.h"
#include "../UiWindow.h"
#include "../DarkMode.h"
#include "../DPI.h"
#include "../HotKeys.h"

#include <algorithm>
#include <vector>

#include <afxpropertygridctrl.h>
#include "../../resource.h"

#ifdef min
#undef min
#endif
#ifdef max
#undef max
#endif

namespace
{
	const TCHAR* const DIALOG_STATE_PROP = _T("Ditto.Ui.DialogState");
	const TCHAR* const DIALOG_HOST_PROP = _T("Ditto.Ui.DialogHost");
	const TCHAR* const BUTTON_STATE_PROP = _T("Ditto.Ui.ButtonState");
	const TCHAR* const FRAME_STATE_PROP = _T("Ditto.Ui.FrameState");
	const TCHAR* const TAB_STATE_PROP = _T("Ditto.Ui.TabState");
	const TCHAR* const HOTKEY_STATE_PROP = _T("Ditto.Ui.HotkeyState");
	const TCHAR* const COMBO_STATE_PROP = _T("Ditto.Ui.ComboState");
	const TCHAR* const HEADER_STATE_PROP = _T("Ditto.Ui.HeaderState");

	enum class ButtonKind
	{
		Push,
		Check,
		Radio,
		Group
	};

	struct DialogThemeState
	{
		HFONT font;
		int dpi;
		bool refreshing;
	};

	struct ButtonThemeState
	{
		ButtonKind kind;
		bool primary;
		bool danger;
		bool hover;
		bool pressed;
	};

	struct FrameThemeState
	{
		int radius;
	};

	struct TabThemeState
	{
		bool hover;
		int hoverItem;
		int pressedItem;
	};

	struct HotkeyThemeState
	{
		bool hover;
	};

	struct ChildCollectContext
	{
		HWND parent;
		std::vector<HWND>* children;
	};

	int GetDpi(HWND hWnd)
	{
		CDPI dpi(hWnd);
		return dpi.GetDPI();
	}

	bool IsWindowEnabledSafe(HWND hWnd)
	{
		return hWnd != NULL && IsWindowEnabled(hWnd) != FALSE;
	}

	bool IsVisibleForLayout(HWND hWnd)
	{
		return hWnd != NULL &&
			(GetWindowLongPtr(hWnd, GWL_STYLE) & WS_VISIBLE) != 0;
	}

	CString WindowText(HWND hWnd)
	{
		CString text;
		if (hWnd != NULL)
		{
			int length = GetWindowTextLength(hWnd);
			if (length > 0)
			{
				GetWindowText(hWnd, text.GetBuffer(length + 1), length + 1);
				text.ReleaseBuffer(length);
			}
		}
		return text;
	}

	CString ClassName(HWND hWnd)
	{
		TCHAR className[128] = { 0 };
		if (hWnd != NULL)
		{
			GetClassName(hWnd, className, _countof(className));
		}
		return CString(className);
	}

	HFONT CreateUiFont(HWND hWnd)
	{
		const CUiTheme& theme = UiTheme();
		const int dpi = GetDpi(hWnd);

		LOGFONT lf = { 0 };
		lf.lfHeight = -UiDraw::Scale(theme.FontSizeDialogBody(), dpi);
		lf.lfWeight = FW_NORMAL;
		lf.lfCharSet = DEFAULT_CHARSET;
		lf.lfQuality = CLEARTYPE_QUALITY;
		_tcsncpy_s(lf.lfFaceName, theme.FontFamilyUi(), _TRUNCATE);
		return CreateFontIndirect(&lf);
	}

	void SetControlFont(HWND hWnd, HFONT font)
	{
		if (hWnd == NULL || font == NULL)
			return;

		SendMessage(hWnd, WM_SETFONT, reinterpret_cast<WPARAM>(font), TRUE);

		CString className = ClassName(hWnd);
		if (className.CompareNoCase(_T("SysListView32")) == 0)
		{
			HWND header = ListView_GetHeader(hWnd);
			if (header != NULL)
				SendMessage(header, WM_SETFONT, reinterpret_cast<WPARAM>(font), TRUE);
		}
	}

	void FillWindowBackground(CDC* pDC, CWnd* pWindow)
	{
		if (pDC == NULL || pWindow == NULL)
			return;

		CRect rect;
		pWindow->GetClientRect(&rect);
		pDC->FillSolidRect(rect, UiTheme().Canvas());
	}

	void DrawTextCore(CDC* pDC, const CString& text, const CRect& rect, COLORREF color,
		int dpi, UINT flags, int weight = FW_NORMAL, int logicalPixels = 0)
	{
		if (pDC == NULL || rect.IsRectEmpty())
			return;

		const CUiTheme& theme = UiTheme();
		const int fontSize = logicalPixels > 0 ? logicalPixels : theme.FontSizeBody();
		HFONT font = UiDraw::Font(theme.FontFamilyUi(), fontSize, weight, dpi);
		HFONT oldFont = static_cast<HFONT>(pDC->SelectObject(font));
		COLORREF oldColor = pDC->SetTextColor(color);
		int oldMode = pDC->SetBkMode(TRANSPARENT);

		CRect textRect(rect);
		pDC->DrawText(text, &textRect, flags);

		pDC->SetBkMode(oldMode);
		pDC->SetTextColor(oldColor);
		pDC->SelectObject(oldFont);
	}

	void DrawCheckGlyph(CDC* pDC, const CRect& box, COLORREF color, int dpi)
	{
		CRect glyph(box);
		int inset = UiDraw::Scale(3, dpi);
		glyph.DeflateRect(inset, inset);
		UiDraw::DrawGlyph(pDC, glyph, UiDraw::Glyph::Check, color, dpi);
	}

	void DrawRadioMark(CDC* pDC, const CRect& box, COLORREF fill, COLORREF border,
		bool checked, bool enabled, int dpi)
	{
		CPen pen(PS_SOLID, std::max(1, UiDraw::Scale(1, dpi)), border);
		CPen* oldPen = pDC->SelectObject(&pen);
		CBrush fillBrush(fill);
		CBrush* oldBrush = pDC->SelectObject(&fillBrush);
		pDC->Ellipse(box);

		if (checked)
		{
			COLORREF dotColor = enabled ? UiTheme().AccentFill() : UiTheme().TextDisabled();
			CBrush dotBrush(dotColor);
			pDC->SelectObject(&dotBrush);
			pDC->SelectStockObject(NULL_PEN);
			CRect dot(box);
			int inset = std::max(UiDraw::Scale(4, dpi), box.Width() / 4);
			dot.DeflateRect(inset, inset);
			pDC->Ellipse(dot);
		}

		pDC->SelectObject(oldBrush);
		pDC->SelectObject(oldPen);
	}

	void DrawGroupBox(CDC* pDC, HWND hWnd, int dpi)
	{
		const CUiTheme& theme = UiTheme();
		CRect client;
		GetClientRect(hWnd, &client);
		pDC->FillSolidRect(client, theme.Canvas());

		CString text = WindowText(hWnd);
		CSize textSize(0, 0);
		if (!text.IsEmpty())
		{
			HFONT font = UiDraw::Font(theme.FontFamilyUi(), theme.FontSizeSecondary(), FW_SEMIBOLD, dpi);
			HFONT oldFont = static_cast<HFONT>(pDC->SelectObject(font));
			textSize = pDC->GetTextExtent(text);
			pDC->SelectObject(oldFont);
		}

		int labelGap = UiDraw::Scale(8, dpi);
		int top = std::max<int>(UiDraw::Scale(7, dpi), textSize.cy / 2);
		CRect border(client);
		border.top = top;
		UiDraw::RoundedRect(pDC, border, UiDraw::Scale(theme.RadiusPanel(), dpi),
			CLR_INVALID, theme.Border(), 1);

		if (!text.IsEmpty())
		{
			CRect labelRect(static_cast<int>(border.left) + labelGap, 0,
				std::min<int>(static_cast<int>(border.right) - labelGap,
					static_cast<int>(border.left) + labelGap + textSize.cx + UiDraw::Scale(4, dpi)),
				std::max<int>(textSize.cy, UiDraw::Scale(16, dpi)));
			pDC->FillSolidRect(labelRect, theme.Canvas());
			labelRect.DeflateRect(UiDraw::Scale(2, dpi), 0);
			DrawTextCore(pDC, text, labelRect, theme.TextSecondary(), dpi,
				DT_LEFT | DT_VCENTER | DT_SINGLELINE, FW_SEMIBOLD,
				theme.FontSizeSecondary());
		}
	}

	void DrawCheckButton(CDC* pDC, HWND hWnd, const ButtonThemeState& state, int dpi)
	{
		const CUiTheme& theme = UiTheme();
		CRect client;
		GetClientRect(hWnd, &client);
		pDC->FillSolidRect(client, theme.Canvas());

		const bool enabled = IsWindowEnabledSafe(hWnd);
		const int boxSize = UiDraw::Scale(16, dpi);
		const int gap = UiDraw::Scale(8, dpi);
		CRect box(client.left + UiDraw::Scale(1, dpi),
			client.top + std::max(0, (client.Height() - boxSize) / 2),
			client.left + UiDraw::Scale(1, dpi) + boxSize,
			client.top + std::max(0, (client.Height() - boxSize) / 2) + boxSize);

		LRESULT check = SendMessage(hWnd, BM_GETCHECK, 0, 0);
		bool checked = check == BST_CHECKED;
		bool indeterminate = check == BST_INDETERMINATE;
		bool focused = GetFocus() == hWnd;
		COLORREF textColor = enabled ? theme.TextPrimary() : theme.TextDisabled();
		COLORREF borderColor = enabled ? theme.BorderStrong() : theme.Border();

		if (state.kind == ButtonKind::Radio)
		{
			DrawRadioMark(pDC, box, enabled ? theme.Surface() : theme.SurfaceAlt(),
				checked ? theme.AccentFill() : borderColor, checked, enabled, dpi);
		}
		else
		{
			COLORREF fill = theme.Surface();
			if (checked || indeterminate)
				fill = enabled ? theme.AccentFill() : theme.SurfaceAlt();
			else if (!enabled)
				fill = theme.SurfaceAlt();

			COLORREF outline = checked ? (enabled ? theme.AccentFill() : theme.Border()) : borderColor;
			UiDraw::RoundedRect(pDC, box, UiDraw::Scale(3, dpi), fill, outline, 1);

			if (checked)
				DrawCheckGlyph(pDC, box, theme.AccentOn(), dpi);
			else if (indeterminate)
			{
				CRect dash(box);
				dash.DeflateRect(UiDraw::Scale(4, dpi), UiDraw::Scale(7, dpi));
				pDC->FillSolidRect(dash, theme.AccentOn());
			}
		}

		if (focused)
		{
			CRect focus(box);
			focus.InflateRect(UiDraw::Scale(2, dpi), UiDraw::Scale(2, dpi));
			UiDraw::FocusRing(pDC, focus, UiDraw::Scale(5, dpi), theme.FocusRing(),
				std::max(1, UiDraw::Scale(theme.FocusRingWidth(), dpi)));
		}

		CRect textRect(client);
		textRect.left = box.right + gap;
		textRect.right = std::max(textRect.left, textRect.right - UiDraw::Scale(2, dpi));
		DrawTextCore(pDC, WindowText(hWnd), textRect, textColor, dpi,
			DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
	}

	void DrawPushButton(CDC* pDC, HWND hWnd, const ButtonThemeState& state, int dpi)
	{
		const CUiTheme& theme = UiTheme();
		CRect client;
		GetClientRect(hWnd, &client);
		pDC->FillSolidRect(client, theme.Canvas());

		const bool enabled = IsWindowEnabledSafe(hWnd);
		const bool focused = GetFocus() == hWnd;
		const LRESULT nativeState = SendMessage(hWnd, BM_GETSTATE, 0, 0);
		const bool pressed = state.pressed || (nativeState & BST_PUSHED) != 0;

		COLORREF fill = theme.SurfaceAlt();
		COLORREF border = theme.Border();
		COLORREF text = theme.TextPrimary();

		if (!enabled)
		{
			fill = theme.SurfaceAlt();
			border = theme.Border();
			text = theme.TextDisabled();
		}
		else if (state.primary)
		{
			fill = pressed ? theme.AccentFillPressed() :
				(state.hover ? theme.AccentFillHover() : theme.AccentFill());
			border = pressed ? theme.AccentPressed() : theme.AccentFill();
			text = theme.AccentOn();
		}
		else if (state.danger)
		{
			fill = pressed ? theme.DangerFill() :
				(state.hover ? theme.Hover() : theme.SurfaceAlt());
			border = state.hover ? theme.Danger() : theme.Border();
			text = pressed ? theme.DangerOn() : theme.Danger();
		}
		else
		{
			fill = pressed ? theme.Selection() :
				(state.hover ? theme.Hover() : theme.SurfaceAlt());
			border = state.hover || pressed ? theme.BorderStrong() : theme.Border();
			text = theme.TextPrimary();
		}

		CRect buttonRect(client);
		buttonRect.DeflateRect(UiDraw::Scale(1, dpi), UiDraw::Scale(1, dpi));
		UiDraw::RoundedRect(pDC, buttonRect, UiDraw::Scale(theme.ButtonRadius(), dpi),
			fill, border, 1);

		if (focused)
		{
			CRect focus(buttonRect);
			focus.DeflateRect(UiDraw::Scale(2, dpi), UiDraw::Scale(2, dpi));
			UiDraw::FocusRing(pDC, focus, UiDraw::Scale(theme.ButtonRadius(), dpi),
				theme.FocusRing(), std::max(1, UiDraw::Scale(theme.FocusRingWidth(), dpi)));
		}

		CRect textRect(buttonRect);
		CString buttonText = WindowText(hWnd);
		CSize textSize = UiDraw::MeasureText(pDC, buttonText, theme.FontFamilyUi(),
			theme.FontSizeBody(), FW_NORMAL, dpi);
		const int buttonPadding = std::min<int>(UiDraw::Scale(theme.ButtonPaddingX(), dpi),
			std::max<int>(UiDraw::Scale(4, dpi),
				static_cast<int>(buttonRect.Width() - textSize.cx) / 2));
		textRect.DeflateRect(buttonPadding, 0);
		DrawTextCore(pDC, buttonText, textRect, text, dpi,
			DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
	}

	void DrawThemedButton(HDC hDC, HWND hWnd, const ButtonThemeState& state)
	{
		if (hDC == NULL || hWnd == NULL)
			return;

		CDC dc;
		dc.Attach(hDC);
		const int dpi = GetDpi(hWnd);

		switch (state.kind)
		{
		case ButtonKind::Group:
			DrawGroupBox(&dc, hWnd, dpi);
			break;
		case ButtonKind::Check:
		case ButtonKind::Radio:
			DrawCheckButton(&dc, hWnd, state, dpi);
			break;
		default:
			DrawPushButton(&dc, hWnd, state, dpi);
			break;
		}

		dc.Detach();
	}

	void DrawHotkeyControl(HDC hDC, HWND hWnd, const HotkeyThemeState& state)
	{
		if (hDC == NULL || hWnd == NULL)
			return;

		CDC dc;
		dc.Attach(hDC);
		const CUiTheme& theme = UiTheme();
		const int dpi = GetDpi(hWnd);
		CRect client;
		GetClientRect(hWnd, &client);
		dc.FillSolidRect(client, theme.Canvas());

		const bool enabled = IsWindowEnabledSafe(hWnd);
		const bool focused = GetFocus() == hWnd;
		COLORREF fill = !enabled ? theme.SurfaceAlt() :
			(focused ? theme.Surface() : (state.hover ? theme.Hover() : theme.Surface()));
		COLORREF border = focused ? theme.FocusRing() :
			(state.hover ? theme.BorderStrong() : theme.Border());
		CRect frame(client);
		frame.DeflateRect(UiDraw::Scale(1, dpi), UiDraw::Scale(1, dpi));
		UiDraw::RoundedRect(&dc, frame, UiDraw::Scale(theme.InputRadius(), dpi),
			fill, border, 1);

		WORD hotkey = static_cast<WORD>(SendMessage(hWnd, HKM_GETHOTKEY, 0, 0));
		CString text;
		if (hotkey != 0)
		{
			const DWORD key = (static_cast<DWORD>(HIBYTE(hotkey)) << 8) |
				LOBYTE(hotkey);
			text = CHotKey::GetHotKeyDisplayStatic(key);
		}

		CRect textRect(frame);
		textRect.DeflateRect(UiDraw::Scale(theme.InputPaddingX(), dpi), 0);
		DrawTextCore(&dc, text, textRect,
			enabled ? theme.TextPrimary() : theme.TextDisabled(), dpi,
			DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);

		if (focused)
		{
			CRect focus(frame);
			focus.DeflateRect(UiDraw::Scale(2, dpi), UiDraw::Scale(2, dpi));
			UiDraw::FocusRing(&dc, focus, UiDraw::Scale(theme.InputRadius(), dpi),
				theme.FocusRing(), std::max(1, UiDraw::Scale(theme.FocusRingWidth(), dpi)));
		}

		dc.Detach();
	}

	LRESULT CALLBACK HotkeySubclassProc(HWND hWnd, UINT message, WPARAM wParam,
		LPARAM lParam, UINT_PTR subclassId, DWORD_PTR refData)
	{
		HotkeyThemeState* state = reinterpret_cast<HotkeyThemeState*>(refData);
		if (state == NULL)
			return DefSubclassProc(hWnd, message, wParam, lParam);

		switch (message)
		{
		case WM_ERASEBKGND:
			return 1;
		case WM_PAINT:
		{
			PAINTSTRUCT paint = { 0 };
			HDC hDC = BeginPaint(hWnd, &paint);
			DrawHotkeyControl(hDC, hWnd, *state);
			EndPaint(hWnd, &paint);
			return 0;
		}
		case WM_PRINTCLIENT:
			DrawHotkeyControl(reinterpret_cast<HDC>(wParam), hWnd, *state);
			return 0;
		case WM_MOUSEMOVE:
			if (!state->hover)
			{
				TRACKMOUSEEVENT track = { sizeof(track), TME_LEAVE, hWnd, 0 };
				TrackMouseEvent(&track);
				state->hover = true;
				InvalidateRect(hWnd, NULL, FALSE);
			}
			break;
		case WM_MOUSELEAVE:
			state->hover = false;
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_SETFOCUS:
		case WM_KILLFOCUS:
		case WM_ENABLE:
		case WM_THEMECHANGED:
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_NCDESTROY:
			RemoveProp(hWnd, HOTKEY_STATE_PROP);
			RemoveWindowSubclass(hWnd, HotkeySubclassProc, subclassId);
			delete state;
			return DefSubclassProc(hWnd, message, wParam, lParam);
		default:
			break;
		}

		return DefSubclassProc(hWnd, message, wParam, lParam);
	}

	void DrawComboControl(HDC hDC, HWND hWnd, const HotkeyThemeState& state)
	{
		if (hDC == NULL || hWnd == NULL)
			return;

		CDC dc;
		dc.Attach(hDC);
		const CUiTheme& theme = UiTheme();
		const int dpi = GetDpi(hWnd);
		CRect client;
		GetClientRect(hWnd, &client);
		dc.FillSolidRect(client, theme.Canvas());

		const bool enabled = IsWindowEnabledSafe(hWnd);
		const bool focused = GetFocus() == hWnd;
		const COLORREF fill = !enabled ? theme.SurfaceAlt() :
			(focused || state.hover ? theme.Hover() : theme.Surface());
		const COLORREF border = focused ? theme.FocusRing() :
			(state.hover ? theme.BorderStrong() : theme.Border());
		CRect frame(client);
		frame.DeflateRect(UiDraw::Scale(1, dpi), UiDraw::Scale(1, dpi));
		UiDraw::RoundedRect(&dc, frame, UiDraw::Scale(theme.InputRadius(), dpi),
			fill, border, 1);

		CString text;
		const int selected = static_cast<int>(SendMessage(hWnd, CB_GETCURSEL, 0, 0));
		if (selected >= 0)
		{
			int length = static_cast<int>(SendMessage(hWnd, CB_GETLBTEXTLEN,
				static_cast<WPARAM>(selected), 0));
			if (length >= 0)
			{
				SendMessage(hWnd, CB_GETLBTEXT, static_cast<WPARAM>(selected),
					reinterpret_cast<LPARAM>(text.GetBuffer(length + 1)));
				text.ReleaseBuffer(length);
			}
		}

		CRect textRect(frame);
		textRect.left += UiDraw::Scale(theme.InputPaddingX(), dpi);
		textRect.right -= UiDraw::Scale(theme.IconSizeSmall() + theme.InputPaddingX(), dpi);
		DrawTextCore(&dc, text, textRect,
			enabled ? theme.TextPrimary() : theme.TextDisabled(), dpi,
			DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);

		CRect glyphRect(frame.right - UiDraw::Scale(theme.InputPaddingX() +
			theme.IconSizeSmall(), dpi),
			client.top + (client.Height() - UiDraw::Scale(theme.IconSizeSmall(), dpi)) / 2,
			frame.right - UiDraw::Scale(theme.InputPaddingX(), dpi),
			client.top + (client.Height() + UiDraw::Scale(theme.IconSizeSmall(), dpi)) / 2);
		UiDraw::DrawGlyph(&dc, glyphRect, UiDraw::Glyph::ChevronDown,
			enabled ? theme.TextSecondary() : theme.TextDisabled(), dpi,
			theme.IconStrokeTenths());

		if (focused)
		{
			CRect focus(frame);
			focus.DeflateRect(UiDraw::Scale(2, dpi), UiDraw::Scale(2, dpi));
			UiDraw::FocusRing(&dc, focus, UiDraw::Scale(theme.InputRadius(), dpi),
				theme.FocusRing(), std::max(1, UiDraw::Scale(theme.FocusRingWidth(), dpi)));
		}

		dc.Detach();
	}

	LRESULT CALLBACK ComboSubclassProc(HWND hWnd, UINT message, WPARAM wParam,
		LPARAM lParam, UINT_PTR subclassId, DWORD_PTR refData)
	{
		HotkeyThemeState* state = reinterpret_cast<HotkeyThemeState*>(refData);
		if (state == NULL)
			return DefSubclassProc(hWnd, message, wParam, lParam);

		switch (message)
		{
		case WM_ERASEBKGND:
			return 1;
		case WM_PAINT:
		{
			PAINTSTRUCT paint = { 0 };
			HDC hDC = BeginPaint(hWnd, &paint);
			DrawComboControl(hDC, hWnd, *state);
			EndPaint(hWnd, &paint);
			return 0;
		}
		case WM_PRINTCLIENT:
			DrawComboControl(reinterpret_cast<HDC>(wParam), hWnd, *state);
			return 0;
		case WM_MOUSEMOVE:
			if (!state->hover)
			{
				TRACKMOUSEEVENT track = { sizeof(track), TME_LEAVE, hWnd, 0 };
				TrackMouseEvent(&track);
				state->hover = true;
				InvalidateRect(hWnd, NULL, FALSE);
			}
			break;
		case WM_MOUSELEAVE:
			state->hover = false;
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_SETFOCUS:
		case WM_KILLFOCUS:
		case WM_ENABLE:
		case WM_THEMECHANGED:
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_NCDESTROY:
			RemoveProp(hWnd, COMBO_STATE_PROP);
			RemoveWindowSubclass(hWnd, ComboSubclassProc, subclassId);
			delete state;
			return DefSubclassProc(hWnd, message, wParam, lParam);
		default:
			break;
		}

		return DefSubclassProc(hWnd, message, wParam, lParam);
	}

	void DrawHeaderControl(HDC hDC, HWND hWnd)
	{
		if (hDC == NULL || hWnd == NULL)
			return;

		CDC dc;
		dc.Attach(hDC);
		const CUiTheme& theme = UiTheme();
		const int dpi = GetDpi(hWnd);
		CRect client;
		GetClientRect(hWnd, &client);
		dc.FillSolidRect(client, theme.SurfaceAlt());

		const int count = Header_GetItemCount(hWnd);
		for (int index = 0; index < count; ++index)
		{
			RECT itemRect = { 0 };
			if (!Header_GetItemRect(hWnd, index, &itemRect))
				continue;

			CRect rect(itemRect);
			dc.FillSolidRect(rect, theme.SurfaceAlt());
			dc.FillSolidRect(CRect(rect.right - 1, rect.top + UiDraw::Scale(6, dpi),
				rect.right, rect.bottom - UiDraw::Scale(6, dpi)), theme.Border());
			dc.FillSolidRect(CRect(rect.left, rect.bottom - 1, rect.right, rect.bottom),
				theme.Border());

			TCHAR text[512] = { 0 };
			HDITEM item = { 0 };
			item.mask = HDI_TEXT;
			item.pszText = text;
			item.cchTextMax = _countof(text);
			if (Header_GetItem(hWnd, index, &item))
			{
				rect.DeflateRect(UiDraw::Scale(8, dpi), 0);
				DrawTextCore(&dc, text, rect, theme.TextSecondary(), dpi,
					DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
					FW_SEMIBOLD, theme.FontSizeSecondary());
			}
		}

		dc.Detach();
	}

	LRESULT CALLBACK HeaderSubclassProc(HWND hWnd, UINT message, WPARAM wParam,
		LPARAM lParam, UINT_PTR subclassId, DWORD_PTR refData)
	{
		UNREFERENCED_PARAMETER(refData);
		switch (message)
		{
		case WM_ERASEBKGND:
			return 1;
		case WM_PAINT:
		{
			PAINTSTRUCT paint = { 0 };
			HDC hDC = BeginPaint(hWnd, &paint);
			DrawHeaderControl(hDC, hWnd);
			EndPaint(hWnd, &paint);
			return 0;
		}
		case WM_PRINTCLIENT:
			DrawHeaderControl(reinterpret_cast<HDC>(wParam), hWnd);
			return 0;
		case WM_THEMECHANGED:
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_NCDESTROY:
			RemoveProp(hWnd, HEADER_STATE_PROP);
			RemoveWindowSubclass(hWnd, HeaderSubclassProc, subclassId);
			break;
		default:
			break;
		}

		return DefSubclassProc(hWnd, message, wParam, lParam);
	}

	LRESULT CALLBACK ButtonSubclassProc(HWND hWnd, UINT message, WPARAM wParam,
		LPARAM lParam, UINT_PTR subclassId, DWORD_PTR refData)
	{
		ButtonThemeState* state = reinterpret_cast<ButtonThemeState*>(refData);
		if (state == NULL)
			return DefSubclassProc(hWnd, message, wParam, lParam);

		switch (message)
		{
		case WM_ERASEBKGND:
			return 1;
		case WM_PAINT:
		{
			PAINTSTRUCT paint = { 0 };
			HDC hDC = BeginPaint(hWnd, &paint);
			DrawThemedButton(hDC, hWnd, *state);
			EndPaint(hWnd, &paint);
			return 0;
		}
		case WM_PRINTCLIENT:
			DrawThemedButton(reinterpret_cast<HDC>(wParam), hWnd, *state);
			return 0;
		case WM_MOUSEMOVE:
			if (!state->hover)
			{
				TRACKMOUSEEVENT track = { sizeof(track), TME_LEAVE, hWnd, 0 };
				TrackMouseEvent(&track);
				state->hover = true;
				InvalidateRect(hWnd, NULL, FALSE);
			}
			break;
		case WM_MOUSELEAVE:
			state->hover = false;
			state->pressed = false;
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_LBUTTONDOWN:
			state->pressed = true;
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_LBUTTONUP:
		case WM_CAPTURECHANGED:
			state->pressed = false;
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_SETFOCUS:
		case WM_KILLFOCUS:
		case WM_ENABLE:
		case WM_THEMECHANGED:
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_NCDESTROY:
			RemoveProp(hWnd, BUTTON_STATE_PROP);
			RemoveWindowSubclass(hWnd, ButtonSubclassProc, subclassId);
			delete state;
			return DefSubclassProc(hWnd, message, wParam, lParam);
		default:
			break;
		}

		return DefSubclassProc(hWnd, message, wParam, lParam);
	}

	void DrawFrame(HWND hWnd, int radius)
	{
		const CUiTheme& theme = UiTheme();
		const int dpi = GetDpi(hWnd);
		HDC hDC = GetWindowDC(hWnd);
		if (hDC == NULL)
			return;

		RECT windowRect = { 0 };
		GetWindowRect(hWnd, &windowRect);
		const int width = windowRect.right - windowRect.left;
		const int height = windowRect.bottom - windowRect.top;

		CPen pen(PS_SOLID, std::max(1, UiDraw::Scale(1, dpi)),
			GetFocus() == hWnd ? theme.FocusRing() : theme.BorderStrong());
		HPEN oldPen = static_cast<HPEN>(SelectObject(hDC, pen));
		HBRUSH oldBrush = static_cast<HBRUSH>(SelectObject(hDC, GetStockObject(NULL_BRUSH)));
		RoundRect(hDC, 0, 0, width, height, radius * 2, radius * 2);
		SelectObject(hDC, oldBrush);
		SelectObject(hDC, oldPen);
		ReleaseDC(hWnd, hDC);
	}

	void UpdateFrameRegion(HWND hWnd, int radius)
	{
		RECT windowRect = { 0 };
		GetWindowRect(hWnd, &windowRect);
		const int width = std::max(1, static_cast<int>(windowRect.right - windowRect.left));
		const int height = std::max(1, static_cast<int>(windowRect.bottom - windowRect.top));
		HRGN region = CreateRoundRectRgn(0, 0, width + 1, height + 1,
			radius * 2, radius * 2);
		if (region != NULL)
			SetWindowRgn(hWnd, region, TRUE);
	}

	LRESULT CALLBACK FrameSubclassProc(HWND hWnd, UINT message, WPARAM wParam,
		LPARAM lParam, UINT_PTR subclassId, DWORD_PTR refData)
	{
		FrameThemeState* state = reinterpret_cast<FrameThemeState*>(refData);
		if (state == NULL)
			return DefSubclassProc(hWnd, message, wParam, lParam);

		switch (message)
		{
		case WM_NCPAINT:
			DrawFrame(hWnd, state->radius);
			return 0;
		case WM_SIZE:
		{
			LRESULT result = DefSubclassProc(hWnd, message, wParam, lParam);
			UpdateFrameRegion(hWnd, state->radius);
			return result;
		}
		case WM_SETFOCUS:
		case WM_KILLFOCUS:
		case WM_ENABLE:
			DrawFrame(hWnd, state->radius);
			break;
		case WM_NCDESTROY:
			RemoveProp(hWnd, FRAME_STATE_PROP);
			RemoveWindowSubclass(hWnd, FrameSubclassProc, subclassId);
			delete state;
			return DefSubclassProc(hWnd, message, wParam, lParam);
		default:
			break;
		}

		return DefSubclassProc(hWnd, message, wParam, lParam);
	}

	void DrawTabControl(HDC hDC, HWND hWnd, const TabThemeState& state)
	{
		if (hDC == NULL)
			return;

		CDC dc;
		dc.Attach(hDC);
		const CUiTheme& theme = UiTheme();
		const int dpi = GetDpi(hWnd);
		CRect client;
		GetClientRect(hWnd, &client);
		dc.FillSolidRect(client, theme.Canvas());

		int count = TabCtrl_GetItemCount(hWnd);
		int selected = TabCtrl_GetCurSel(hWnd);
		const int inset = UiDraw::Scale(4, dpi);
		const int paddingX = UiDraw::Scale(theme.TabPaddingX(), dpi);
		const int minItemWidth = UiDraw::Scale(64, dpi);
		const int rowGap = UiDraw::Scale(2, dpi);
		const int maxRowWidth = std::max<int>(1, client.Width() - inset * 2);

		std::vector<CString> labels;
		std::vector<int> widths;
		std::vector<int> rows;
		int row = 0;
		int rowWidth = 0;
		for (int index = 0; index < count; ++index)
		{
			TCHAR text[512] = { 0 };
			TCITEM item = { 0 };
			item.mask = TCIF_TEXT;
			item.pszText = text;
			item.cchTextMax = _countof(text);
			TabCtrl_GetItem(hWnd, index, &item);
			CString label(text);
			CSize textSize = UiDraw::MeasureText(&dc, label, theme.FontFamilyUi(),
				theme.FontSizeSecondary(), FW_SEMIBOLD, dpi);
			int width = std::max<int>(textSize.cx + paddingX * 2, minItemWidth);
			width = std::min<int>(width, maxRowWidth);
			if (rowWidth > 0 && rowWidth + width > maxRowWidth)
			{
				++row;
				rowWidth = 0;
			}
			labels.push_back(label);
			widths.push_back(width);
			rows.push_back(row);
			rowWidth += width + rowGap;
		}

		const int rowCount = row + 1;
		const int rowHeight = UiDraw::Scale(theme.TabHeight(), dpi);
		std::vector<int> rowOffsets(rowCount, 0);
		for (int index = 0; index < count; ++index)
		{
			const int rowIndex = rows[index];
			CRect tab(inset + rowOffsets[rowIndex], rowIndex * rowHeight,
				inset + rowOffsets[rowIndex] + widths[index],
				(rowIndex + 1) * rowHeight - rowGap);
			rowOffsets[rowIndex] += widths[index] + rowGap;

			tab.DeflateRect(UiDraw::Scale(1, dpi), 0);

			bool active = index == selected;
			bool hover = state.hoverItem == index;
			COLORREF fill = active ? theme.Surface() :
				(hover ? theme.Hover() : theme.SurfaceAlt());
			COLORREF textColor = active ? theme.TextPrimary() : theme.TextSecondary();
			UiDraw::RoundedRect(&dc, tab, UiDraw::Scale(theme.RadiusControl(), dpi),
				fill, theme.Border(), 1);

			if (active)
			{
				CRect merge(tab);
				merge.top = tab.bottom - UiDraw::Scale(theme.TabUnderlineHeight(), dpi);
				dc.FillSolidRect(merge, theme.Accent());
				merge.top = tab.bottom - 1;
				merge.left += UiDraw::Scale(theme.RadiusControl(), dpi);
				merge.right -= UiDraw::Scale(theme.RadiusControl(), dpi);
				dc.FillSolidRect(merge, theme.Surface());
			}

			CRect textRect(tab);
			CSize textSize = UiDraw::MeasureText(&dc, labels[index], theme.FontFamilyUi(),
				theme.FontSizeSecondary(), active ? FW_SEMIBOLD : FW_NORMAL, dpi);
			const int padding = std::min<int>(UiDraw::Scale(theme.TabPaddingX(), dpi),
				std::max<int>(UiDraw::Scale(4, dpi),
					static_cast<int>(tab.Width() - textSize.cx) / 2));
			textRect.DeflateRect(padding, 0);
			DrawTextCore(&dc, labels[index], textRect, textColor, dpi,
				DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
				active ? FW_SEMIBOLD : FW_NORMAL, theme.FontSizeSecondary());

			if (active && GetFocus() == hWnd)
			{
				CRect focus(tab);
				focus.DeflateRect(UiDraw::Scale(2, dpi), UiDraw::Scale(2, dpi));
				UiDraw::FocusRing(&dc, focus, UiDraw::Scale(theme.RadiusControl(), dpi),
					theme.FocusRing(), std::max(1, UiDraw::Scale(theme.FocusRingWidth(), dpi)));
			}
		}

		const int tabBottom = std::min<int>(client.bottom, rowCount * rowHeight);
		if (tabBottom > 0 && tabBottom < client.bottom)
		{
			CRect remainder(client);
			remainder.top = tabBottom;
			dc.FillSolidRect(remainder, theme.Canvas());
		}

		dc.Detach();
	}

	int HitTestCustomTab(HWND hWnd, const CPoint& point)
	{
		const CUiTheme& theme = UiTheme();
		const int dpi = GetDpi(hWnd);
		CRect client;
		GetClientRect(hWnd, &client);
		CClientDC dc(CWnd::FromHandle(hWnd));
		const int inset = UiDraw::Scale(4, dpi);
		const int paddingX = UiDraw::Scale(theme.TabPaddingX(), dpi);
		const int minItemWidth = UiDraw::Scale(64, dpi);
		const int rowGap = UiDraw::Scale(2, dpi);
		const int maxRowWidth = std::max<int>(1, client.Width() - inset * 2);
		const int count = TabCtrl_GetItemCount(hWnd);

		std::vector<int> widths;
		std::vector<int> rows;
		int row = 0;
		int rowWidth = 0;
		for (int index = 0; index < count; ++index)
		{
			TCHAR text[512] = { 0 };
			TCITEM item = { 0 };
			item.mask = TCIF_TEXT;
			item.pszText = text;
			item.cchTextMax = _countof(text);
			TabCtrl_GetItem(hWnd, index, &item);
			CSize textSize = UiDraw::MeasureText(&dc, CString(text), theme.FontFamilyUi(),
				theme.FontSizeSecondary(), FW_SEMIBOLD, dpi);
			int width = std::max<int>(textSize.cx + paddingX * 2, minItemWidth);
			width = std::min<int>(width, maxRowWidth);
			if (rowWidth > 0 && rowWidth + width > maxRowWidth)
			{
				++row;
				rowWidth = 0;
			}
			widths.push_back(width);
			rows.push_back(row);
			rowWidth += width + rowGap;
		}

		const int rowCount = row + 1;
		const int rowHeight = UiDraw::Scale(theme.TabHeight(), dpi);
		std::vector<int> rowOffsets(rowCount, 0);
		for (int index = 0; index < count; ++index)
		{
			const int rowIndex = rows[index];
			CRect tab(inset + rowOffsets[rowIndex], rowIndex * rowHeight,
				inset + rowOffsets[rowIndex] + widths[index],
				(rowIndex + 1) * rowHeight - rowGap);
			rowOffsets[rowIndex] += widths[index] + rowGap;
			if (tab.PtInRect(point))
				return index;
		}

		return -1;
	}

	void SelectCustomTab(HWND hWnd, int index)
	{
		if (index < 0 || index >= TabCtrl_GetItemCount(hWnd))
			return;

		TabCtrl_SetCurSel(hWnd, index);
		NMHDR change = { hWnd, static_cast<UINT_PTR>(GetDlgCtrlID(hWnd)), TCN_SELCHANGE };
		SendMessage(GetParent(hWnd), WM_NOTIFY, change.idFrom,
			reinterpret_cast<LPARAM>(&change));
		SetFocus(hWnd);
		InvalidateRect(hWnd, NULL, FALSE);
	}

	LRESULT CALLBACK TabSubclassProc(HWND hWnd, UINT message, WPARAM wParam,
		LPARAM lParam, UINT_PTR subclassId, DWORD_PTR refData)
	{
		TabThemeState* state = reinterpret_cast<TabThemeState*>(refData);
		if (state == NULL)
			return DefSubclassProc(hWnd, message, wParam, lParam);

		switch (message)
		{
		case WM_ERASEBKGND:
			return 1;
		case WM_PAINT:
		{
			PAINTSTRUCT paint = { 0 };
			HDC hDC = BeginPaint(hWnd, &paint);
			DrawTabControl(hDC, hWnd, *state);
			EndPaint(hWnd, &paint);
			return 0;
		}
		case WM_PRINTCLIENT:
			DrawTabControl(reinterpret_cast<HDC>(wParam), hWnd, *state);
			return 0;
		case WM_MOUSEMOVE:
		{
			CPoint point;
			GetCursorPos(&point);
			ScreenToClient(hWnd, &point);
			const int item = HitTestCustomTab(hWnd, point);
			if (state->hoverItem != item)
			{
				state->hoverItem = item;
				TRACKMOUSEEVENT track = { sizeof(track), TME_LEAVE, hWnd, 0 };
				TrackMouseEvent(&track);
				InvalidateRect(hWnd, NULL, FALSE);
			}
			break;
		}
		case WM_MOUSELEAVE:
			state->hoverItem = -1;
			state->pressedItem = -1;
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_LBUTTONDOWN:
		{
			CPoint point(static_cast<short>(LOWORD(lParam)),
				static_cast<short>(HIWORD(lParam)));
			state->pressedItem = HitTestCustomTab(hWnd, point);
			if (state->pressedItem >= 0)
			{
				SetCapture(hWnd);
				InvalidateRect(hWnd, NULL, FALSE);
				return 0;
			}
			break;
		}
		case WM_LBUTTONUP:
		{
			if (state->pressedItem >= 0)
			{
				CPoint point(static_cast<short>(LOWORD(lParam)),
					static_cast<short>(HIWORD(lParam)));
				const int item = HitTestCustomTab(hWnd, point);
				if (item == state->pressedItem)
					SelectCustomTab(hWnd, item);
				state->pressedItem = -1;
				ReleaseCapture();
				InvalidateRect(hWnd, NULL, FALSE);
				return 0;
			}
			break;
		}
		case WM_KEYUP:
		case WM_SETFOCUS:
		case WM_KILLFOCUS:
		case WM_THEMECHANGED:
			InvalidateRect(hWnd, NULL, FALSE);
			break;
		case WM_NCDESTROY:
			RemoveProp(hWnd, TAB_STATE_PROP);
			RemoveWindowSubclass(hWnd, TabSubclassProc, subclassId);
			delete state;
			return DefSubclassProc(hWnd, message, wParam, lParam);
		default:
			break;
		}

		return DefSubclassProc(hWnd, message, wParam, lParam);
	}

	void ApplyNativeControlColors(HWND hWnd)
	{
		const CUiTheme& theme = UiTheme();
		CString className = ClassName(hWnd);
		if (className.CompareNoCase(_T("SysListView32")) == 0)
		{
			ListView_SetBkColor(hWnd, theme.Surface());
			ListView_SetTextBkColor(hWnd, theme.Surface());
			ListView_SetTextColor(hWnd, theme.TextPrimary());
		}
		else if (className.CompareNoCase(_T("SysTreeView32")) == 0)
		{
			TreeView_SetBkColor(hWnd, theme.Surface());
			TreeView_SetTextColor(hWnd, theme.TextPrimary());
		}
		else if (className.CompareNoCase(_T("RichEdit20W")) == 0 ||
			className.CompareNoCase(_T("RichEdit50W")) == 0)
		{
			SetWindowTheme(hWnd, L"", L"");
			LONG_PTR exStyle = GetWindowLongPtr(hWnd, GWL_EXSTYLE);
			if ((exStyle & WS_EX_TRANSPARENT) == 0)
			{
				SetWindowLongPtr(hWnd, GWL_EXSTYLE, exStyle | WS_EX_TRANSPARENT);
			}
			SendMessage(hWnd, EM_SETBKGNDCOLOR, 0,
				static_cast<LPARAM>(theme.SurfaceSunken()));
			CHARFORMAT2 format = { 0 };
			format.cbSize = sizeof(format);
			format.dwMask = CFM_COLOR;
			format.crTextColor = theme.TextPrimary();
			SendMessage(hWnd, EM_SETCHARFORMAT, SCF_DEFAULT,
				reinterpret_cast<LPARAM>(&format));
		}
		else if (className.CompareNoCase(_T("MfcPropertyGrid")) == 0)
		{
			CMFCPropertyGridCtrl* grid =
				dynamic_cast<CMFCPropertyGridCtrl*>(CWnd::FromHandlePermanent(hWnd));
			if (grid != NULL)
			{
				grid->SetCustomColors(theme.Surface(), theme.TextPrimary(),
					theme.SurfaceAlt(), theme.TextPrimary(), theme.Surface(),
					theme.TextSecondary(), theme.Border());
				grid->Invalidate(FALSE);
			}
		}
	}

	void FitPropertySheetTabs(CPropertySheet* pSheet)
	{
		if (pSheet == NULL || pSheet->GetSafeHwnd() == NULL)
			return;

		CTabCtrl* tab = pSheet->GetTabControl();
		if (tab == NULL || tab->GetSafeHwnd() == NULL)
			return;

		const int dpi = GetDpi(tab->GetSafeHwnd());
		const CUiTheme& theme = UiTheme();
		const int paddingX = UiDraw::Scale(theme.TabPaddingX(), dpi);
		const int minWidth = UiDraw::Scale(64, dpi);
		const int count = tab->GetItemCount();
		if (count <= 0)
			return;

		CClientDC dc(tab);
		int wantedWidth = 0;
		for (int index = 0; index < count; ++index)
		{
			TCHAR text[512] = { 0 };
			TCITEM item = { 0 };
			item.mask = TCIF_TEXT;
			item.pszText = text;
			item.cchTextMax = _countof(text);
			tab->GetItem(index, &item);

			CSize textSize = UiDraw::MeasureText(&dc, text, theme.FontFamilyUi(),
				theme.FontSizeSecondary(), FW_SEMIBOLD, dpi);
			wantedWidth += std::max<int>(textSize.cx + paddingX * 2, minWidth);
		}

		wantedWidth += UiDraw::Scale(48, dpi) + (count - 1) * UiDraw::Scale(2, dpi);

		CRect tabRect;
		tab->GetClientRect(&tabRect);
		CRect tabWindowRect;
		tab->GetWindowRect(&tabWindowRect);
		const int extraWidth = wantedWidth - tabRect.Width();
		if (extraWidth <= UiDraw::Scale(8, dpi))
			return;

		CRect sheetRect;
		pSheet->GetWindowRect(&sheetRect);
		MONITORINFO monitorInfo = { sizeof(monitorInfo) };
		HMONITOR monitor = MonitorFromWindow(pSheet->GetSafeHwnd(), MONITOR_DEFAULTTONEAREST);
		int maxWidth = sheetRect.Width() + extraWidth;
		int newLeft = sheetRect.left;
		int newTop = sheetRect.top;
		if (monitor != NULL && GetMonitorInfo(monitor, &monitorInfo))
		{
			maxWidth = std::min<int>(maxWidth,
				monitorInfo.rcWork.right - monitorInfo.rcWork.left - UiDraw::Scale(16, dpi));
			const int rightLimit = monitorInfo.rcWork.right - UiDraw::Scale(8, dpi);
			const int leftLimit = monitorInfo.rcWork.left + UiDraw::Scale(8, dpi);
			newLeft = std::min<int>(newLeft, rightLimit - maxWidth);
			newLeft = std::max<int>(leftLimit, newLeft);
		}

		const int newWidth = std::max<int>(sheetRect.Width(), maxWidth);
		if (newWidth > sheetRect.Width())
		{
			pSheet->SetWindowPos(NULL, newLeft, newTop, newWidth, sheetRect.Height(),
				SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
			tab->SetWindowPos(NULL, 0, 0, std::max<int>(wantedWidth, tabWindowRect.Width()),
				tabWindowRect.Height(), SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE |
				SWP_NOOWNERZORDER);
		}

	}

	void PrepareControl(HWND hWnd, HFONT font)
	{
		if (hWnd == NULL || !IsWindow(hWnd))
			return;

		SetControlFont(hWnd, font);
		CDarkMode::ApplyToControl(hWnd);
		ApplyNativeControlColors(hWnd);

		CString className = ClassName(hWnd);
		LONG_PTR style = GetWindowLongPtr(hWnd, GWL_STYLE);
		const int type = static_cast<int>(style & BS_TYPEMASK);

		if (className.CompareNoCase(_T("Button")) == 0 &&
			GetProp(hWnd, BUTTON_STATE_PROP) == NULL)
		{
			ButtonThemeState* state = new ButtonThemeState();
			state->hover = false;
			state->pressed = false;
			state->primary = false;
			state->danger = false;

			const UINT id = GetDlgCtrlID(hWnd);
			state->primary = id == IDOK || id == IDC_BUTTON_APPLY ||
				type == BS_DEFPUSHBUTTON;
			state->danger = id == IDC_BUTTON_DELETE_SCRIPT;

			if (type == BS_GROUPBOX)
				state->kind = ButtonKind::Group;
			else if (type == BS_AUTOCHECKBOX || type == BS_CHECKBOX ||
				type == BS_3STATE || type == BS_AUTO3STATE)
				state->kind = ButtonKind::Check;
			else if (type == BS_AUTORADIOBUTTON || type == BS_RADIOBUTTON)
				state->kind = ButtonKind::Radio;
			else
				state->kind = ButtonKind::Push;

			SetProp(hWnd, BUTTON_STATE_PROP, state);
			SetWindowSubclass(hWnd, ButtonSubclassProc, 1,
				reinterpret_cast<DWORD_PTR>(state));
		}
		else if ((className.CompareNoCase(_T("Edit")) == 0 ||
			className.CompareNoCase(_T("msctls_hotkey32")) == 0 ||
			(className.CompareNoCase(_T("ComboBox")) == 0 &&
				(style & 0x0003) != CBS_DROPDOWNLIST)) &&
			(style & WS_BORDER) != 0 &&
			GetProp(hWnd, FRAME_STATE_PROP) == NULL)
		{
			FrameThemeState* state = new FrameThemeState();
			state->radius = UiDraw::Scale(UiTheme().InputRadius(), GetDpi(hWnd));
			SetProp(hWnd, FRAME_STATE_PROP, state);
			SetWindowSubclass(hWnd, FrameSubclassProc, 1,
				reinterpret_cast<DWORD_PTR>(state));
			UpdateFrameRegion(hWnd, state->radius);
		}
		if (className.CompareNoCase(_T("msctls_hotkey32")) == 0 &&
			GetProp(hWnd, HOTKEY_STATE_PROP) == NULL)
		{
			HotkeyThemeState* state = new HotkeyThemeState();
			state->hover = false;
			SetProp(hWnd, HOTKEY_STATE_PROP, state);
			SetWindowSubclass(hWnd, HotkeySubclassProc, 1,
				reinterpret_cast<DWORD_PTR>(state));
		}
		if (className.CompareNoCase(_T("ComboBox")) == 0 &&
			(style & 0x0003) == CBS_DROPDOWNLIST &&
			GetProp(hWnd, COMBO_STATE_PROP) == NULL)
		{
			HotkeyThemeState* state = new HotkeyThemeState();
			state->hover = false;
			SetProp(hWnd, COMBO_STATE_PROP, state);
			SetWindowSubclass(hWnd, ComboSubclassProc, 1,
				reinterpret_cast<DWORD_PTR>(state));
		}
		if (className.CompareNoCase(_T("SysTabControl32")) == 0 &&
			GetProp(hWnd, TAB_STATE_PROP) == NULL)
		{
			TabThemeState* state = new TabThemeState();
			state->hover = false;
			state->hoverItem = -1;
			state->pressedItem = -1;
			SetProp(hWnd, TAB_STATE_PROP, state);
			SetWindowSubclass(hWnd, TabSubclassProc, 1,
				reinterpret_cast<DWORD_PTR>(state));
		}
		if (className.CompareNoCase(_T("SysHeader32")) == 0 &&
			GetProp(hWnd, HEADER_STATE_PROP) == NULL)
		{
			SetProp(hWnd, HEADER_STATE_PROP, reinterpret_cast<HANDLE>(1));
			SetWindowSubclass(hWnd, HeaderSubclassProc, 1, 0);
		}
	}

	BOOL CALLBACK PrepareChildProc(HWND hWnd, LPARAM lParam)
	{
		PrepareControl(hWnd, reinterpret_cast<HFONT>(lParam));
		return TRUE;
	}

	BOOL CALLBACK CollectDirectChildProc(HWND hWnd, LPARAM lParam)
	{
		ChildCollectContext* context = reinterpret_cast<ChildCollectContext*>(lParam);
		if (context != NULL && context->children != NULL &&
			GetParent(hWnd) == context->parent)
		{
			context->children->push_back(hWnd);
		}
		return TRUE;
	}

	void ApplyDialogFont(HWND hWnd, HFONT font)
	{
		SetControlFont(hWnd, font);
		EnumChildWindows(hWnd, PrepareChildProc, reinterpret_cast<LPARAM>(font));
	}

	DialogThemeState* EnsureDialogState(CWnd* pWindow)
	{
		if (pWindow == NULL || pWindow->GetSafeHwnd() == NULL)
			return NULL;

		HWND hWnd = pWindow->GetSafeHwnd();
		DialogThemeState* state =
			reinterpret_cast<DialogThemeState*>(GetProp(hWnd, DIALOG_STATE_PROP));
		if (state == NULL)
		{
			state = new DialogThemeState();
			state->dpi = GetDpi(hWnd);
			state->font = CreateUiFont(hWnd);
			state->refreshing = false;
			SetProp(hWnd, DIALOG_STATE_PROP, state);
		}
		return state;
	}

	HBRUSH BrushForColor(COLORREF color)
	{
		return UiDraw::Brush(color);
	}

	bool DrawItemButton(LPDRAWITEMSTRUCT pDrawItem)
	{
		if (pDrawItem == NULL || pDrawItem->hwndItem == NULL)
			return false;

		ButtonThemeState local = { ButtonKind::Push, false, false, false, false };
		LONG_PTR style = GetWindowLongPtr(pDrawItem->hwndItem, GWL_STYLE);
		const int type = static_cast<int>(style & BS_TYPEMASK);
		if (type == BS_GROUPBOX)
			local.kind = ButtonKind::Group;
		else if (type == BS_AUTOCHECKBOX || type == BS_CHECKBOX ||
			type == BS_3STATE || type == BS_AUTO3STATE)
			local.kind = ButtonKind::Check;
		else if (type == BS_AUTORADIOBUTTON || type == BS_RADIOBUTTON)
			local.kind = ButtonKind::Radio;

		local.pressed = (pDrawItem->itemState & ODS_SELECTED) != 0;
		local.primary = GetDlgCtrlID(pDrawItem->hwndItem) == IDOK;
		DrawThemedButton(pDrawItem->hDC, pDrawItem->hwndItem, local);
		return true;
	}

	LRESULT CALLBACK HostSubclassProc(HWND hWnd, UINT message, WPARAM wParam,
		LPARAM lParam, UINT_PTR subclassId, DWORD_PTR refData)
	{
		DialogThemeState* state = reinterpret_cast<DialogThemeState*>(refData);
		if (state == NULL)
			return DefSubclassProc(hWnd, message, wParam, lParam);

		UINT ctlColor = 0;
		switch (message)
		{
		case WM_CTLCOLORMSGBOX:
			ctlColor = CTLCOLOR_MSGBOX;
			break;
		case WM_CTLCOLOREDIT:
			ctlColor = CTLCOLOR_EDIT;
			break;
		case WM_CTLCOLORLISTBOX:
			ctlColor = CTLCOLOR_LISTBOX;
			break;
		case WM_CTLCOLORBTN:
			ctlColor = CTLCOLOR_BTN;
			break;
		case WM_CTLCOLORDLG:
			ctlColor = CTLCOLOR_DLG;
			break;
		case WM_CTLCOLORSCROLLBAR:
			ctlColor = CTLCOLOR_SCROLLBAR;
			break;
		case WM_CTLCOLORSTATIC:
			ctlColor = CTLCOLOR_STATIC;
			break;
		default:
			break;
		}

		if (ctlColor != 0)
		{
			return reinterpret_cast<LRESULT>(CThemedControls::OnCtlColor(
				CWnd::FromHandlePermanent(hWnd),
				CDC::FromHandle(reinterpret_cast<HDC>(wParam)),
				CWnd::FromHandle(reinterpret_cast<HWND>(lParam)),
				ctlColor));
		}

		switch (message)
		{
		case WM_ERASEBKGND:
			return CThemedControls::OnEraseBkgnd(
				CWnd::FromHandlePermanent(hWnd),
				CDC::FromHandle(reinterpret_cast<HDC>(wParam)));
		case WM_PRINTCLIENT:
		{
			CDC* pDC = CDC::FromHandle(reinterpret_cast<HDC>(wParam));
			FillWindowBackground(pDC, CWnd::FromHandlePermanent(hWnd));
			return 0;
		}
		case WM_DRAWITEM:
			if (CThemedControls::OnDrawItem(CWnd::FromHandlePermanent(hWnd),
				reinterpret_cast<LPDRAWITEMSTRUCT>(lParam)))
				return TRUE;
			break;
		case WM_NOTIFY:
		{
			LRESULT result = 0;
			if (CThemedControls::OnCustomDraw(CWnd::FromHandlePermanent(hWnd),
				reinterpret_cast<NMHDR*>(lParam), &result))
				return result;
			break;
		}
		case WM_DPICHANGED:
		{
			if (state->refreshing)
				break;
			state->refreshing = true;
			HFONT oldFont = state->font;
			state->dpi = HIWORD(wParam);
			state->font = CreateUiFont(hWnd);
			ApplyDialogFont(hWnd, state->font);
			if (oldFont != NULL)
				DeleteObject(oldFont);
			state->refreshing = false;
			break;
		}
		case WM_SETTINGCHANGE:
		case WM_THEMECHANGED:
		{
			if (state->refreshing)
				break;
			state->refreshing = true;
			HFONT oldFont = state->font;
			state->font = CreateUiFont(hWnd);
			ApplyDialogFont(hWnd, state->font);
			if (oldFont != NULL)
				DeleteObject(oldFont);
			CDarkMode::ApplyToWindow(hWnd);
			UiWindow::ApplyWindowEffects(hWnd);
			InvalidateRect(hWnd, NULL, TRUE);
			state->refreshing = false;
			break;
		}
		case WM_NCDESTROY:
			RemoveProp(hWnd, DIALOG_STATE_PROP);
			RemoveProp(hWnd, DIALOG_HOST_PROP);
			RemoveWindowSubclass(hWnd, HostSubclassProc, subclassId);
			if (state->font != NULL)
				DeleteObject(state->font);
			delete state;
			return DefSubclassProc(hWnd, message, wParam, lParam);
		default:
			break;
		}

		return DefSubclassProc(hWnd, message, wParam, lParam);
	}

	CRect ControlRect(HWND parent, HWND child)
	{
		RECT rect = { 0 };
		GetWindowRect(child, &rect);
		MapWindowPoints(NULL, parent, reinterpret_cast<POINT*>(&rect), 2);
		return CRect(rect);
	}

	bool NeedHeightAdjustment(LONG_PTR style, const CString& className)
	{
		if (className.CompareNoCase(_T("Button")) == 0)
		{
			const int type = static_cast<int>(style & BS_TYPEMASK);
			return type != BS_GROUPBOX && type != BS_AUTOCHECKBOX &&
				type != BS_CHECKBOX && type != BS_3STATE &&
				type != BS_AUTO3STATE && type != BS_AUTORADIOBUTTON &&
				type != BS_RADIOBUTTON;
		}
		return className.CompareNoCase(_T("Edit")) == 0 ||
			className.CompareNoCase(_T("ComboBox")) == 0 ||
			className.CompareNoCase(_T("msctls_hotkey32")) == 0;
	}

	int ClampHeightToNeighbor(HWND parent, HWND child, const CRect& current,
		int desiredHeight, int dpi, const std::vector<HWND>& siblings)
	{
		if (desiredHeight <= current.Height())
			return current.Height();

		const int gap = UiDraw::Scale(4, dpi);
		int limit = desiredHeight;
		for (HWND sibling : siblings)
		{
			if (sibling == child || !IsVisibleForLayout(sibling))
				continue;

			CRect other = ControlRect(parent, sibling);
			if (other.top < current.bottom)
				continue;
			if (other.right <= current.left || other.left >= current.right)
				continue;
			limit = std::min<int>(limit,
				static_cast<int>(other.top - current.top) - gap);
		}

		CRect client;
		GetClientRect(parent, &client);
		limit = std::min<int>(limit,
			static_cast<int>(client.bottom - current.top) - gap);
		return std::max<int>(current.Height(), limit);
	}
}

int CThemedControls::DpiForWindow(HWND hWnd)
{
	return GetDpi(hWnd);
}

void CThemedControls::PrepareDialog(CWnd* pDialog)
{
	if (pDialog == NULL || pDialog->GetSafeHwnd() == NULL)
		return;

	DialogThemeState* state = EnsureDialogState(pDialog);
	if (state == NULL)
		return;
	if (state->refreshing)
		return;
	state->refreshing = true;

	CDarkMode::ApplyToWindow(pDialog->GetSafeHwnd());
	UiWindow::ApplyWindowEffects(pDialog->GetSafeHwnd());
	if (GetProp(pDialog->GetSafeHwnd(), DIALOG_HOST_PROP) == NULL)
	{
		SetProp(pDialog->GetSafeHwnd(), DIALOG_HOST_PROP, state);
		SetWindowSubclass(pDialog->GetSafeHwnd(), HostSubclassProc, 1,
			reinterpret_cast<DWORD_PTR>(state));
	}
	ApplyDialogFont(pDialog->GetSafeHwnd(), state->font);
	state->refreshing = false;
}

void CThemedControls::PreparePropertySheet(CPropertySheet* pSheet)
{
	if (pSheet == NULL || pSheet->GetSafeHwnd() == NULL)
		return;

	PrepareDialog(pSheet);

	const int pageCount = pSheet->GetPageCount();
	for (int index = 0; index < pageCount; ++index)
	{
		CPropertyPage* page = pSheet->GetPage(index);
		if (page != NULL && page->GetSafeHwnd() != NULL)
		{
			PrepareDialog(page);
			FinishLayout(page);
		}
	}

	CTabCtrl* tab = pSheet->GetTabControl();
	if (tab != NULL && tab->GetSafeHwnd() != NULL)
	{
		DialogThemeState* state = EnsureDialogState(pSheet);
		if (state != NULL)
			PrepareControl(tab->GetSafeHwnd(), state->font);
	}

	FitPropertySheetTabs(pSheet);
	FinishLayout(pSheet);
}

void CThemedControls::RefreshTheme(CWnd* pWindow)
{
	if (pWindow == NULL || pWindow->GetSafeHwnd() == NULL)
		return;

	DialogThemeState* state = EnsureDialogState(pWindow);
	if (state == NULL)
		return;
	if (state->refreshing)
		return;
	state->refreshing = true;

	HFONT oldFont = state->font;
	state->dpi = GetDpi(pWindow->GetSafeHwnd());
	state->font = CreateUiFont(pWindow->GetSafeHwnd());
	ApplyDialogFont(pWindow->GetSafeHwnd(), state->font);
	if (oldFont != NULL)
		DeleteObject(oldFont);

	state->refreshing = false;
	PrepareDialog(pWindow);
	FinishLayout(pWindow);
	InvalidateRect(pWindow->GetSafeHwnd(), NULL, TRUE);
}

void CThemedControls::FinishLayout(CWnd* pDialog)
{
	if (pDialog == NULL || pDialog->GetSafeHwnd() == NULL)
		return;

	CWnd* pWindow = pDialog;
	const int dpi = GetDpi(pWindow->GetSafeHwnd());
	const CUiTheme& theme = UiTheme();
	CRect client;
	pWindow->GetClientRect(&client);

	std::vector<HWND> children;
	ChildCollectContext collect = { pWindow->GetSafeHwnd(), &children };
	EnumChildWindows(pWindow->GetSafeHwnd(), CollectDirectChildProc,
		reinterpret_cast<LPARAM>(&collect));

	CClientDC dc(pWindow);
	const int layoutPadding = UiDraw::Scale(theme.DialogPadding(), dpi);
	for (HWND child : children)
	{
		if (!IsVisibleForLayout(child))
			continue;

		CString className = ClassName(child);
		if (className.CompareNoCase(_T("Static")) != 0)
			continue;

		CString text = WindowText(child);
		if (text.IsEmpty())
			continue;

		CRect current = ControlRect(pWindow->GetSafeHwnd(), child);
		CSize textSize = UiDraw::MeasureText(&dc, text, theme.FontFamilyUi(),
			theme.FontSizeDialogBody(), FW_NORMAL, dpi);
		const int desiredWidth = textSize.cx + UiDraw::Scale(16, dpi);
		if (desiredWidth <= current.Width())
			continue;

		std::vector<HWND> rowRight;
		int rowRightEdge = current.right;
		int nearestRight = client.right - layoutPadding;
		for (HWND sibling : children)
		{
			if (sibling == child || !IsVisibleForLayout(sibling))
				continue;

			CRect siblingRect = ControlRect(pWindow->GetSafeHwnd(), sibling);
			const bool overlapsRow = siblingRect.top < current.bottom &&
				siblingRect.bottom > current.top;
			if (overlapsRow && siblingRect.left > current.left + UiDraw::Scale(2, dpi))
			{
				rowRight.push_back(sibling);
				rowRightEdge = std::max<int>(rowRightEdge, siblingRect.right);
				nearestRight = std::min<int>(nearestRight, siblingRect.left);
			}
		}

		const int needed = desiredWidth - current.Width();
		const int gap = UiDraw::Scale(4, dpi);
		const int expandInPlace = std::min<int>(needed,
			std::max<int>(0, nearestRight - current.right - gap));
		const int availableForShift = client.right - layoutPadding - rowRightEdge;
		const int shift = std::min<int>(needed - expandInPlace,
			std::max<int>(0, availableForShift));
		const int expand = expandInPlace + shift;
		if (expand <= 0)
			continue;

		for (HWND sibling : rowRight)
		{
			if (shift <= 0)
				break;
			CRect siblingRect = ControlRect(pWindow->GetSafeHwnd(), sibling);
			SetWindowPos(sibling, NULL, siblingRect.left + shift, siblingRect.top,
				0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
		}

		SetWindowPos(child, NULL, 0, 0, current.Width() + expand, current.Height(),
			SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
	}

	for (HWND child : children)
	{
		if (!IsVisibleForLayout(child))
			continue;

		CString className = ClassName(child);
		LONG_PTR style = GetWindowLongPtr(child, GWL_STYLE);
		CRect current = ControlRect(pWindow->GetSafeHwnd(), child);
		CRect adjusted(current);
		CString text = WindowText(child);
		const int padding = UiDraw::Scale(theme.DialogPadding(), dpi);
		const int maxRight = client.right - padding;

		if (className.CompareNoCase(_T("Button")) == 0)
		{
			const int type = static_cast<int>(style & BS_TYPEMASK);
			if (type == BS_GROUPBOX)
			{
				CSize textSize = UiDraw::MeasureText(&dc, text, theme.FontFamilyUi(),
					theme.FontSizeSecondary(), FW_SEMIBOLD, dpi);
				const int desired = textSize.cx + UiDraw::Scale(24, dpi);
				if (desired > adjusted.Width())
					adjusted.right = std::min<int>(maxRight,
						static_cast<int>(adjusted.left) + desired);
			}
			else if (type == BS_AUTOCHECKBOX || type == BS_CHECKBOX ||
				type == BS_3STATE || type == BS_AUTO3STATE ||
				type == BS_AUTORADIOBUTTON || type == BS_RADIOBUTTON)
			{
				CSize textSize = UiDraw::MeasureText(&dc, text, theme.FontFamilyUi(),
					theme.FontSizeBody(), FW_NORMAL, dpi);
				const int desired = UiDraw::Scale(16 + 8 + 2, dpi) + textSize.cx;
				if (desired > adjusted.Width())
					adjusted.right = std::min<int>(maxRight,
						static_cast<int>(adjusted.left) + desired);
			}
			else
			{
				CSize textSize = UiDraw::MeasureText(&dc, text, theme.FontFamilyUi(),
					theme.FontSizeBody(), FW_NORMAL, dpi);
				const bool iconSized = text.GetLength() <= 1;
				int desired = textSize.cx + UiDraw::Scale(theme.ButtonPaddingX() * 2, dpi);
				if (!iconSized)
					desired = std::max<int>(desired, UiDraw::Scale(theme.ButtonMinWidth(), dpi));
				if (desired > adjusted.Width())
				{
					adjusted.right = std::min<int>(maxRight,
						static_cast<int>(adjusted.left) + desired);
					if (adjusted.right - adjusted.left < desired)
						adjusted.left = std::max<int>(padding,
							static_cast<int>(adjusted.right) - desired);
				}

				const int desiredHeight = UiDraw::Scale(theme.ButtonHeight(), dpi);
				const int height = ClampHeightToNeighbor(pWindow->GetSafeHwnd(), child,
					current, desiredHeight, dpi, children);
				adjusted.bottom = adjusted.top + height;
			}
		}
		else if (NeedHeightAdjustment(style, className))
		{
			const int desiredHeight = UiDraw::Scale(theme.InputHeight(), dpi);
			const int height = ClampHeightToNeighbor(pWindow->GetSafeHwnd(), child,
				current, desiredHeight, dpi, children);
			adjusted.bottom = adjusted.top + height;
		}

		if (adjusted != current)
		{
			SetWindowPos(child, NULL, adjusted.left, adjusted.top,
				adjusted.Width(), adjusted.Height(),
				SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
		}
	}
}

HBRUSH CThemedControls::OnCtlColor(CWnd* pDialog, CDC* pDC, CWnd* pControl, UINT nCtlColor)
{
	UNREFERENCED_PARAMETER(pDialog);
	if (pDC == NULL)
		return NULL;

	const CUiTheme& theme = UiTheme();
	const bool enabled = pControl == NULL || pControl->GetSafeHwnd() == NULL ||
		IsWindowEnabledSafe(pControl->GetSafeHwnd());
	pDC->SetBkMode(TRANSPARENT);

	switch (nCtlColor)
	{
	case CTLCOLOR_DLG:
	case CTLCOLOR_MSGBOX:
		pDC->SetBkColor(theme.Canvas());
		pDC->SetTextColor(theme.TextPrimary());
		return BrushForColor(theme.Canvas());

	case CTLCOLOR_STATIC:
		pDC->SetTextColor(enabled ? theme.TextPrimary() : theme.TextDisabled());
		return BrushForColor(theme.Canvas());

	case CTLCOLOR_BTN:
		pDC->SetTextColor(enabled ? theme.TextPrimary() : theme.TextDisabled());
		return BrushForColor(theme.Canvas());

	case CTLCOLOR_EDIT:
	{
		LONG_PTR style = pControl != NULL && pControl->GetSafeHwnd() != NULL ?
			GetWindowLongPtr(pControl->GetSafeHwnd(), GWL_STYLE) : 0;
		const bool readOnly = (style & ES_READONLY) != 0;
		COLORREF background = !enabled ? theme.SurfaceAlt() :
			(readOnly ? theme.SurfaceSunken() : theme.Surface());
		pDC->SetBkColor(background);
		pDC->SetTextColor(enabled ? theme.TextPrimary() : theme.TextDisabled());
		return BrushForColor(background);
	}

	case CTLCOLOR_LISTBOX:
		pDC->SetBkColor(theme.Surface());
		pDC->SetTextColor(enabled ? theme.TextPrimary() : theme.TextDisabled());
		return BrushForColor(theme.Surface());

	case CTLCOLOR_SCROLLBAR:
		pDC->SetBkColor(theme.SurfaceAlt());
		return BrushForColor(theme.SurfaceAlt());

	default:
		pDC->SetBkColor(theme.Canvas());
		pDC->SetTextColor(enabled ? theme.TextPrimary() : theme.TextDisabled());
		return BrushForColor(theme.Canvas());
	}
}

BOOL CThemedControls::OnEraseBkgnd(CWnd* pWindow, CDC* pDC)
{
	FillWindowBackground(pDC, pWindow);
	return TRUE;
}

BOOL CThemedControls::OnDrawItem(CWnd* pDialog, LPDRAWITEMSTRUCT pDrawItem)
{
	UNREFERENCED_PARAMETER(pDialog);
	if (pDrawItem == NULL)
		return FALSE;

	if (pDrawItem->CtlType == ODT_BUTTON)
		return DrawItemButton(pDrawItem) ? TRUE : FALSE;

	return FALSE;
}

BOOL CThemedControls::OnCustomDraw(CWnd* pWindow, NMHDR* pHeader, LRESULT* pResult)
{
	if (pWindow == NULL || pHeader == NULL || pResult == NULL ||
		pHeader->code != NM_CUSTOMDRAW)
		return FALSE;

	CString className = ClassName(pHeader->hwndFrom);
	const bool isList = className.CompareNoCase(_T("SysListView32")) == 0;
	const bool isTree = className.CompareNoCase(_T("SysTreeView32")) == 0;
	if (!isList && !isTree)
		return FALSE;

	NMCUSTOMDRAW* custom = reinterpret_cast<NMCUSTOMDRAW*>(pHeader);
	const CUiTheme& theme = UiTheme();
	if (custom->dwDrawStage == CDDS_PREPAINT)
	{
		*pResult = CDRF_NOTIFYITEMDRAW;
		return TRUE;
	}

	if (custom->dwDrawStage == CDDS_ITEMPREPAINT)
	{
		const bool selected = (custom->uItemState & CDIS_SELECTED) != 0;
		const bool disabled = (custom->uItemState & CDIS_DISABLED) != 0;
		COLORREF background = selected ? theme.Selection() : theme.Surface();
		COLORREF foreground = selected ? theme.SelectionText() :
			(disabled ? theme.TextDisabled() : theme.TextPrimary());
		if (isList)
		{
			NMLVCUSTOMDRAW* list = reinterpret_cast<NMLVCUSTOMDRAW*>(pHeader);
			list->clrTextBk = background;
			list->clrText = foreground;
		}
		else
		{
			NMTVCUSTOMDRAW* tree = reinterpret_cast<NMTVCUSTOMDRAW*>(pHeader);
			tree->clrTextBk = background;
			tree->clrText = foreground;
		}
		*pResult = CDRF_DODEFAULT;
		return TRUE;
	}

	return FALSE;
}
