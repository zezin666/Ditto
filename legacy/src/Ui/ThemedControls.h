#pragma once

class CPropertySheet;

// Shared MFC/GDI theme helpers for dialogs, property pages, and property
// sheets. The helpers subclass native controls for painting only; their native
// input, notification, DDX, and validation behavior stays intact.
class CThemedControls
{
public:
	static void PrepareDialog(CWnd* pDialog);
	static void PreparePropertySheet(CPropertySheet* pSheet);
	static void RefreshTheme(CWnd* pWindow);
	static void FinishLayout(CWnd* pDialog);

	static HBRUSH OnCtlColor(CWnd* pDialog, CDC* pDC, CWnd* pControl, UINT nCtlColor);
	static BOOL OnEraseBkgnd(CWnd* pWindow, CDC* pDC);
	static BOOL OnDrawItem(CWnd* pDialog, LPDRAWITEMSTRUCT pDrawItem);
	static BOOL OnCustomDraw(CWnd* pWindow, NMHDR* pHeader, LRESULT* pResult);

	static int DpiForWindow(HWND hWnd);
};
