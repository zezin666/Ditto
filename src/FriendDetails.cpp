// FriendDetails.cpp : implementation file
//

#include "stdafx.h"
#include "cp_main.h"
#include "FriendDetails.h"
#include "DPI.h"
#include "UiDraw.h"
#include "UiTheme.h"
#include <algorithm>

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// CFriendDetails dialog

namespace
{
	void FitCheckBoxToText(CWnd* pDialog, UINT controlId)
	{
		CWnd* pControl = pDialog->GetDlgItem(controlId);
		if (pControl == NULL || pControl->GetSafeHwnd() == NULL)
			return;

		CString text;
		pControl->GetWindowText(text);
		CDPI dpi(pDialog->GetSafeHwnd());
		CClientDC dc(pControl);
		const CSize textSize = UiDraw::MeasureText(&dc, text,
			UiTheme().FontFamilyUi(), UiTheme().FontSizeBody(), FW_NORMAL,
			dpi.GetDPI());
		CRect controlRect;
		pControl->GetWindowRect(controlRect);
		pDialog->ScreenToClient(controlRect);
		CRect clientRect;
		pDialog->GetClientRect(clientRect);
		const int desiredWidth = std::max<int>(controlRect.Width(),
			dpi.Scale(16 + 8 + 12) + textSize.cx);
		const int padding = dpi.Scale(7);
		const int extraWidth = std::max<int>(0,
			controlRect.left + desiredWidth - (clientRect.right - padding));
		if (extraWidth > 0)
		{
			CRect windowRect;
			pDialog->GetWindowRect(windowRect);
			pDialog->SetWindowPos(NULL, 0, 0, windowRect.Width() + extraWidth,
				windowRect.Height(), SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
			for (UINT buttonId : { static_cast<UINT>(IDOK), static_cast<UINT>(IDCANCEL) })
			{
				CWnd* pButton = pDialog->GetDlgItem(buttonId);
				if (pButton != NULL && pButton->GetSafeHwnd() != NULL)
				{
					CRect buttonRect;
					pButton->GetWindowRect(buttonRect);
					pDialog->ScreenToClient(buttonRect);
					pButton->SetWindowPos(NULL, buttonRect.left + extraWidth,
						buttonRect.top, 0, 0,
						SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
				}
			}
			clientRect.right += extraWidth;
		}
		const int desiredRight = controlRect.left + desiredWidth;
		if (desiredRight > controlRect.right)
		{
			pControl->SetWindowPos(NULL, 0, 0, desiredRight - controlRect.left,
				controlRect.Height(), SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
		}
	}

	void FitLabelAndMoveEdit(CWnd* pDialog, UINT labelId, UINT editId)
	{
		CWnd* pLabel = pDialog->GetDlgItem(labelId);
		CWnd* pEdit = pDialog->GetDlgItem(editId);
		if (pLabel == NULL || pEdit == NULL ||
			pLabel->GetSafeHwnd() == NULL || pEdit->GetSafeHwnd() == NULL)
			return;

		CString text;
		pLabel->GetWindowText(text);
		CClientDC dc(pLabel);
		CFont* pFont = pLabel->GetFont();
		CFont* pOldFont = pFont != NULL ? dc.SelectObject(pFont) : NULL;
		const CSize textSize = dc.GetTextExtent(text);
		if (pOldFont != NULL)
			dc.SelectObject(pOldFont);

		CDPI dpi(pDialog->GetSafeHwnd());
		CRect labelRect;
		pLabel->GetWindowRect(labelRect);
		pDialog->ScreenToClient(labelRect);
		const int labelWidth = std::max<int>(labelRect.Width(),
			textSize.cx + dpi.Scale(3));
		if (labelWidth > labelRect.Width())
		{
			pLabel->SetWindowPos(NULL, 0, 0, labelWidth, labelRect.Height(),
				SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
			labelRect.right = labelRect.left + labelWidth;
		}

		CRect editRect;
		pEdit->GetWindowRect(editRect);
		pDialog->ScreenToClient(editRect);
		const int desiredLeft = labelRect.right + dpi.Scale(8);
		if (desiredLeft > editRect.left)
		{
			const int width = std::max<int>(dpi.Scale(48),
				editRect.right - desiredLeft);
			pEdit->SetWindowPos(NULL, desiredLeft, editRect.top, width,
				editRect.Height(), SWP_NOZORDER | SWP_NOACTIVATE);
		}
	}
}


CFriendDetails::CFriendDetails(CWnd* pParent /*=NULL*/)
	: CThemedDialog(CFriendDetails::IDD, pParent)
{
	//{{AFX_DATA_INIT(CFriendDetails)
	m_checkSendAll = FALSE;
	m_csDescription = _T("");
	m_csIP = _T("");
	//}}AFX_DATA_INIT
}


void CFriendDetails::DoDataExchange(CDataExchange* pDX)
{
	CThemedDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(CFriendDetails)
	DDX_Check(pDX, IDC_CHECK_SEND_ALL, m_checkSendAll);
	DDX_Text(pDX, IDC_EDIT_DESCRIPTION, m_csDescription);
	DDX_Text(pDX, IDC_EDIT_IP, m_csIP);
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(CFriendDetails, CThemedDialog)
	//{{AFX_MSG_MAP(CFriendDetails)
		// NOTE: the ClassWizard will add message map macros here
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// CFriendDetails message handlers

BOOL CFriendDetails::OnInitDialog() 
{
	CThemedDialog::OnInitDialog();
	
	UpdateData(FALSE);

	theApp.m_Language.UpdateOptionFriendsDetail(this);
	CThemedControls::FinishLayout(this);
	FitCheckBoxToText(this, IDC_CHECK_SEND_ALL);
	FitLabelAndMoveEdit(this, IDC_STATIC_1, IDC_EDIT_IP);
	FitLabelAndMoveEdit(this, IDC_STATIC_2, IDC_EDIT_DESCRIPTION);
	
	return TRUE;  // return TRUE unless you set the focus to a control
	              // EXCEPTION: OCX Property Pages should return FALSE
}

void CFriendDetails::OnOK() 
{
	UpdateData();
	
	CThemedDialog::OnOK();
}
