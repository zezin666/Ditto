#if !defined(AFX_GROUPSTATIC_H__C7039DB6_84EE_4622_8054_6DBA48FA21A9__INCLUDED_)
#define AFX_GROUPSTATIC_H__C7039DB6_84EE_4622_8054_6DBA48FA21A9__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// GroupStatic.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// CGroupStatic window

class CGroupStatic : public CStatic
{
// Construction
public:
	CGroupStatic();

	enum UiStyle
	{
		STYLE_DEFAULT,
		STYLE_GROUP_HEADER,
		STYLE_EMPTY_STATE
	};

// Attributes
public:

// Operations
public:

	void SetTextColor( COLORREF color )		{ m_dwTextColor = color;	}
	void SetBkColor( COLORREF color )		{ m_dwBkColor = color;		}
	void SetFont( int nPointSize, LPCTSTR lpszFaceName, CDC* pDC = NULL );
	void SetFont(CFont *pFont)				{ CStatic::SetFont(pFont);		}
	void SetToggleCursor(bool toggle)		{ m_toggleCursorToHand = toggle; }
	virtual void SetWindowText(LPCTSTR text);
	void SetUiStyle(UiStyle style);
	void SetEmptyState(const CString& title, const CString& detail, const CString& action);
	void RefreshTheme();
	virtual BOOL PreTranslateMessage(MSG* pMsg);

// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(CGroupStatic)
	public:
	virtual BOOL OnChildNotify(UINT message, WPARAM wParam, LPARAM lParam, LRESULT* pLResult);
	//}}AFX_VIRTUAL

// Implementation
public:
	virtual ~CGroupStatic();

protected:
	COLORREF m_dwTextColor;
	COLORREF m_dwBkColor;
	CFont*   m_pFont;
	CBrush   m_brush;
	CString  m_strBuff;        // Holds the static controls contents before & after the control is displayed
	bool m_toggleCursorToHand;
	UiStyle m_uiStyle;
	CString m_emptyTitle;
	CString m_emptyDetail;
	CString m_emptyAction;
	CRect m_actionRect;
	bool m_bActionHover;
	bool m_bTracking;
	CToolTipCtrl m_toolTip;

	void UpdateToolTip();
	void DrawGroupHeader(CDC& dc, const CRect& rect);
	void DrawEmptyState(CDC& dc, const CRect& rect);


	// Generated message map functions
protected:
	//{{AFX_MSG(CGroupStatic)
	afx_msg BOOL OnSetCursor(CWnd* pWnd, UINT nHitTest, UINT message);
	afx_msg void OnPaint();
	afx_msg BOOL OnEraseBkgnd(CDC* pDC);
	afx_msg void OnMouseMove(UINT nFlags, CPoint point);
	afx_msg LRESULT OnMouseLeave(WPARAM wParam, LPARAM lParam);
	afx_msg void OnLButtonDown(UINT nFlags, CPoint point);
	afx_msg void OnLButtonUp(UINT nFlags, CPoint point);
	afx_msg UINT OnGetDlgCode();
	afx_msg void OnKeyDown(UINT nChar, UINT nRepCnt, UINT nFlags);
	afx_msg void OnSetFocus(CWnd* pOldWnd);
	afx_msg void OnKillFocus(CWnd* pNewWnd);
	afx_msg void OnSize(UINT nType, int cx, int cy);
	//}}AFX_MSG

	DECLARE_MESSAGE_MAP()
};

/////////////////////////////////////////////////////////////////////////////

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_GROUPSTATIC_H__C7039DB6_84EE_4622_8054_6DBA48FA21A9__INCLUDED_)
