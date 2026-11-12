// FriendPromptDlg.cpp : implementation file
//

#include "stdafx.h"
#include "CP_Main.h"
#include "FriendPromptDlg.h"
#include "afxdialogex.h"
#include "DPI.h"
#include "UiDraw.h"
#include "UiTheme.h"
#include "..\Shared\Tokenizer.h"
#include <algorithm>


// CFriendPromptDlg dialog

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
		const int desiredRight = std::min<int>(clientRect.right - dpi.Scale(7),
			controlRect.left + desiredWidth);
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

IMPLEMENT_DYNAMIC(CFriendPromptDlg, CThemedDialog)

CFriendPromptDlg::CFriendPromptDlg(CWnd* pParent /*=NULL*/)
	: CThemedDialog(IDD_DIALOG_FREIND_PROMPT, pParent)
{
	m_save = false;
	m_clearList = false;
}

CFriendPromptDlg::~CFriendPromptDlg()
{
}

void CFriendPromptDlg::DoDataExchange(CDataExchange* pDX)
{
	CThemedDialog::DoDataExchange(pDX);
	DDX_Control(pDX, IDC_EDIT_NAME, m_textBox);
	DDX_Control(pDX, IDC_EDIT_DESC, m_description);
}


BEGIN_MESSAGE_MAP(CFriendPromptDlg, CThemedDialog)
	ON_BN_CLICKED(IDOK, &CFriendPromptDlg::OnBnClickedOk)
	ON_WM_SIZE()
	ON_WM_CTLCOLOR()
	ON_BN_CLICKED(IDCANCEL, &CFriendPromptDlg::OnBnClickedCancel)
	ON_BN_CLICKED(IDC_BUTTON_CLEAR, &CFriendPromptDlg::OnBnClickedButtonClear)
END_MESSAGE_MAP()

void CFriendPromptDlg::OnBnClickedOk()
{
	m_textBox.GetWindowTextW(m_name);
	m_description.GetWindowTextW(m_desc);

	if (::IsDlgButtonChecked(m_hWnd, IDC_CHECK_SAVE) == BST_CHECKED)
	{
		m_save = true;
	}

	CThemedDialog::OnOK();
}

BOOL CFriendPromptDlg::OnInitDialog()
{
	CThemedDialog::OnInitDialog();

	::SendMessage(this->m_hWnd, WM_SETICON, 0, NULL);

	//remove the default icon in top left of window
	int extendedStyle = GetWindowLong(m_hWnd, GWL_EXSTYLE);
	SetWindowLong(m_hWnd, GWL_EXSTYLE, extendedStyle | WS_EX_DLGMODALFRAME);
	SetWindowPos(NULL, 0, 0, 0, 0, SWP_NOMOVE |	SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);

	::CheckDlgButton(m_hWnd, IDC_CHECK_SAVE, BST_CHECKED);

	m_textBox.SetFocus();
	CThemedControls::FinishLayout(this);
	FitCheckBoxToText(this, IDC_CHECK_SAVE);
	FitLabelAndMoveEdit(this, IDC_STATIC_FRIEND_PROMPT, IDC_EDIT_NAME);
	FitLabelAndMoveEdit(this, IDC_STATIC_FRIEND_DESC, IDC_EDIT_DESC);
	
	return FALSE;
}

void CFriendPromptDlg::OnSize(UINT nType, int cx, int cy)
{
	CThemedDialog::OnSize(nType, cx, cy);
}

HBRUSH CFriendPromptDlg::OnCtlColor(CDC* pDC, CWnd* pWnd, UINT nCtlColor)
{
	return CThemedControls::OnCtlColor(this, pDC, pWnd, nCtlColor);
}


void CFriendPromptDlg::OnBnClickedCancel()
{
	// TODO: Add your control notification handler code here
	CThemedDialog::OnCancel();
}


void CFriendPromptDlg::OnBnClickedButtonClear()
{
	m_clearList = true;
	CThemedDialog::OnOK();
}
