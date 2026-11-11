// CopyProperties.cpp : implementation file
//

#include "stdafx.h"
#include "cp_main.h"
#include "CopyProperties.h"
#include ".\copyproperties.h"
#include "Md5.h"
#include "DPI.h"
#include "UiDraw.h"
#include "UiTheme.h"
#include "..\Shared\TextConvert.h"
#include <algorithm>

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// CCopyProperties dialog

namespace
{
	void FitCheckBoxToText(CWnd* pDialog, UINT controlId)
	{
		CWnd* pControl = pDialog->GetDlgItem(controlId);
		if (pControl == NULL || pControl->GetSafeHwnd() == NULL)
			return;

		CString text;
		pControl->GetWindowText(text);
		if (text.IsEmpty())
			return;

		CRect controlRect;
		pControl->GetWindowRect(controlRect);
		pDialog->ScreenToClient(controlRect);

		CDPI dpi(pDialog->GetSafeHwnd());
		CClientDC dc(pControl);
		const CSize textSize = UiDraw::MeasureText(&dc, text,
			UiTheme().FontFamilyUi(), UiTheme().FontSizeBody(), FW_NORMAL,
			dpi.GetDPI());
		const int desiredWidth = std::max<int>(controlRect.Width(),
			dpi.Scale(16 + 8 + 12) + textSize.cx);

		CRect clientRect;
		pDialog->GetClientRect(clientRect);
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
		}
		const int desiredRight = controlRect.left + desiredWidth;
		if (desiredRight > controlRect.right)
		{
			const int delta = desiredRight - controlRect.right;
			HWND child = ::GetWindow(pDialog->GetSafeHwnd(), GW_CHILD);
			while (child != NULL)
			{
				if (child != pControl->GetSafeHwnd())
				{
					CRect siblingRect;
					::GetWindowRect(child, &siblingRect);
					pDialog->ScreenToClient(siblingRect);
					if (siblingRect.top < controlRect.bottom &&
						siblingRect.bottom > controlRect.top &&
						siblingRect.left >= controlRect.right)
					{
						::SetWindowPos(child, NULL, siblingRect.left + delta,
							siblingRect.top, 0, 0,
							SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
					}
				}
				child = ::GetWindow(child, GW_HWNDNEXT);
			}
			pControl->SetWindowPos(NULL, 0, 0, desiredRight - controlRect.left,
				controlRect.Height(), SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
		}
	}

	void FitLabelAndMoveControl(CWnd* pDialog, UINT labelId, UINT controlId)
	{
		CWnd* pLabel = pDialog->GetDlgItem(labelId);
		CWnd* pControl = pDialog->GetDlgItem(controlId);
		if (pLabel == NULL || pControl == NULL)
			return;

		CString text;
		pLabel->GetWindowText(text);
		CClientDC dc(pLabel);
		CFont* pFont = pLabel->GetFont();
		CFont* pOldFont = pFont != NULL ? dc.SelectObject(pFont) : NULL;
		const CSize textSize = dc.GetTextExtent(text);
		if (pOldFont != NULL)
			dc.SelectObject(pOldFont);

		CRect labelRect;
		pLabel->GetWindowRect(labelRect);
		pDialog->ScreenToClient(labelRect);
		CDPI dpi(pDialog->GetSafeHwnd());
		const int desiredWidth = std::max<int>(labelRect.Width(),
			textSize.cx + dpi.Scale(3));
		if (desiredWidth <= labelRect.Width())
			return;

		CRect controlRect;
		pControl->GetWindowRect(controlRect);
		pDialog->ScreenToClient(controlRect);
		const int desiredLeft = labelRect.left + desiredWidth + dpi.Scale(5);
		const int delta = std::max<int>(0, desiredLeft - controlRect.left);
		pLabel->SetWindowPos(NULL, 0, 0, desiredWidth, labelRect.Height(),
			SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
		if (delta > 0)
		{
			pControl->SetWindowPos(NULL, controlRect.left + delta, controlRect.top,
				0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
		}
	}
}

CCopyProperties::CCopyProperties(long lCopyID, CWnd* pParent, CClip *pMemoryClip)
	: CThemedDialog(CCopyProperties::IDD, pParent)
{
	m_lCopyID = lCopyID;
	m_bDeletedData = false;
	m_bChangedText = false;
	m_bHandleKillFocus = false;
	m_bHideOnKillFocus = false;
	m_lGroupChangedTo = -1;
	m_pMemoryClip = pMemoryClip;
	m_bSetToTopMost = true;

	//{{AFX_DATA_INIT(CCopyProperties)
	m_eDate = _T("");
	m_lastPasteDate = _T("");
	m_bNeverAutoDelete = FALSE;
	//}}AFX_DATA_INIT
}


void CCopyProperties::DoDataExchange(CDataExchange* pDX)
{
	CThemedDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(CCopyProperties)
	DDX_Control(pDX, IDC_EDIT_QUICK_PASTE, m_QuickPasteText);
	DDX_Control(pDX, IDC_EDIT_PROPERTIES, m_description);
	DDX_Control(pDX, IDC_COMBO1, m_GroupCombo);
	DDX_Control(pDX, IDC_HOTKEY, m_HotKey);
	DDX_Control(pDX, IDC_COPY_DATA, m_lCopyData);
	DDX_Text(pDX, IDC_DATE, m_eDate);
	DDX_Text(pDX, IDC_DATE_LAST_USED, m_lastPasteDate);
	DDX_Check(pDX, IDC_NEVER_AUTO_DELETE, m_bNeverAutoDelete);
	DDX_Check(pDX, IDC_HOT_KEY_GLOBAL, m_hotKeyGlobal);
	DDX_Control(pDX, IDC_HOTKEY_MOVE_TO_GROUP, m_MoveToGrouHotKey);
	DDX_Check(pDX, IDC_HOT_KEY_GLOBAL_MOVE_TO_GROUP, m_moveToGroupHotKeyGlobal);
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(CCopyProperties, CThemedDialog)
	//{{AFX_MSG_MAP(CCopyProperties)
	ON_BN_CLICKED(IDC_DELETE_COPY_DATA, OnDeleteCopyData)
	ON_WM_ACTIVATE()
	ON_WM_SIZE()
	//}}AFX_MSG_MAP
	ON_WM_CTLCOLOR()
	ON_LBN_SELCHANGE(IDC_COPY_DATA, &CCopyProperties::OnLbnSelchangeCopyData)
	ON_WM_NCLBUTTONDOWN()
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// CCopyProperties message handlers

BOOL CCopyProperties::OnInitDialog() 
{
	CThemedDialog::OnInitDialog();

	//remove the default icon in top left of window
	int extendedStyle = GetWindowLong(m_hWnd, GWL_EXSTYLE);
	SetWindowLong(m_hWnd, GWL_EXSTYLE, extendedStyle | WS_EX_DLGMODALFRAME);
	SetWindowPos(NULL, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);

	m_GroupCombo.FillCombo();

	try
	{
		if(m_lCopyID == -1 && m_pMemoryClip != NULL)
		{
			LoadDataFromCClip(*m_pMemoryClip);
		}
		else
		{
			if(m_clip.LoadMainTable(m_lCopyID))
			{
				m_clip.LoadFormats(m_lCopyID);
				LoadDataFromCClip(m_clip);
			}
		}
	}
	CATCH_SQLITE_EXCEPTION

	UpdateData(FALSE);

	if(m_bSetToTopMost)
		SetWindowPos(&CWnd::wndTopMost, 0, 0, 0, 0, SWP_NOMOVE|SWP_NOSIZE);

	if(m_lCopyID == -1 && m_pMemoryClip != NULL)
	{
		GetDlgItem(IDOK)->SetFocus();
	}
	else
	{
		m_description.SetFocus();
	}
	
	m_Resize.SetParent(m_hWnd);
	m_Resize.AddControl(IDC_EDIT_PROPERTIES, DR_SizeHeight | DR_SizeWidth);
	m_Resize.AddControl(IDC_STATIC_FORMATS, DR_MoveTop);
	m_Resize.AddControl(IDC_COPY_DATA, DR_MoveTop | DR_SizeWidth);
	m_Resize.AddControl(IDC_DELETE_COPY_DATA, DR_MoveTop);
	m_Resize.AddControl(IDOK, DR_MoveTop | DR_MoveLeft);
	m_Resize.AddControl(IDCANCEL, DR_MoveTop | DR_MoveLeft);
	m_Resize.AddControl(IDC_EDIT_QUICK_PASTE, DR_SizeWidth);
	m_Resize.AddControl(IDC_COMBO1, DR_SizeWidth);
	m_Resize.AddControl(IDC_STATIC_MD5, DR_MoveTop | DR_MoveLeft);
	m_Resize.AddControl(IDC_EDIT_MD5, DR_MoveTop | DR_MoveLeft);
	
	theApp.m_Language.UpdateClipProperties(this);
	CThemedControls::FinishLayout(this);
	FitCheckBoxToText(this, IDC_CHECK_WIN);
	FitCheckBoxToText(this, IDC_CHECK_WIN_MOVE_TO_GROUP);
	FitCheckBoxToText(this, IDC_HOT_KEY_GLOBAL);
	FitCheckBoxToText(this, IDC_HOT_KEY_GLOBAL_MOVE_TO_GROUP);
	FitLabelAndMoveControl(this, IDC_STATIC_MD5, IDC_EDIT_MD5);

	if (m_clip.ID() > 0)
	{
		CString text;
		this->GetWindowText(text);
		CString newText;
		newText.Format(_T("%s - %d"), text, m_clip.ID());
		this->SetWindowText(newText);
	}

	return FALSE;
}

void CCopyProperties::LoadDataFromCClip(CClip &Clip)
{
	COleDateTime dtTime(Clip.m_Time.GetTime());
	m_eDate = dtTime.Format();
	
	COleDateTime lastPasteDate(Clip.m_lastPasteDate.GetTime());
	m_lastPasteDate = lastPasteDate.Format();

	m_description.SetWindowText(Clip.m_Desc);
	
	if(Clip.m_dontAutoDelete)
	{
		m_bNeverAutoDelete = TRUE;
	}
	else
	{
		m_bNeverAutoDelete = FALSE;
	}

	m_hotKeyGlobal = Clip.m_globalShortCut;

	m_GroupCombo.SetCurSelOnItemData(Clip.m_parentId);

	DWORD shortcut = ACCEL_MAKEKEY(LOBYTE(Clip.m_shortCut), ((HIBYTE(Clip.m_shortCut)) &~HOTKEYF_EXT));

	switch (LOBYTE(shortcut))
	{
		case VK_LEFT:
		case VK_UP:
		case VK_RIGHT:
		case VK_DOWN: // arrow keys
		case VK_PRIOR:
		case VK_NEXT: // page up and page down
		case VK_END:
		case VK_HOME:
		case VK_INSERT:
		case VK_DELETE:
		case VK_DIVIDE: // numpad slash
		case VK_NUMLOCK:
		{
			shortcut = ACCEL_MAKEKEY(LOBYTE(shortcut), (HIBYTE(shortcut) | HOTKEYF_EXT));
		}
		break;
	}

	m_HotKey.SetHotKey(LOBYTE(shortcut), (HIBYTE(shortcut)));
	m_HotKey.SetRules(HKCOMB_A, 0);
	if(HIBYTE(Clip.m_shortCut) & HOTKEYF_EXT)
	{
		::CheckDlgButton(m_hWnd, IDC_CHECK_WIN, BST_CHECKED);
	}

	m_moveToGroupHotKeyGlobal = Clip.m_globalMoveToGroupShortCut;

	m_MoveToGrouHotKey.SetHotKey(LOBYTE(Clip.m_moveToGroupShortCut), (HIBYTE(Clip.m_moveToGroupShortCut) & ~HOTKEYF_EXT));
	m_MoveToGrouHotKey.SetRules(HKCOMB_A, 0);
	if(HIBYTE(Clip.m_moveToGroupShortCut) & HOTKEYF_EXT)
	{
		::CheckDlgButton(m_hWnd, IDC_CHECK_WIN_MOVE_TO_GROUP, BST_CHECKED);
	}

	m_QuickPasteText.SetWindowText(Clip.m_csQuickPaste);

	CString cs;
	CClipFormat* pCF;
	INT_PTR count = Clip.m_Formats.GetSize();
	for(int i = 0; i < count; i++)
	{
		pCF = &Clip.m_Formats.GetData()[i];
		if(pCF)
		{
			const int MAX_SIZE_BUFFER = 255;
			TCHAR size[MAX_SIZE_BUFFER];
			StrFormatByteSize(GlobalSize(pCF->m_hgData), size, MAX_SIZE_BUFFER);

			cs.Format(_T("%s, %s"), GetFormatName(pCF->m_cfType), size);
			int nIndex = m_lCopyData.AddString(cs);
			
			if(m_lCopyID == -1 && pCF->m_dataId == -1)
				m_lCopyData.SetItemData(nIndex, i);
			else
				m_lCopyData.SetItemData(nIndex, pCF->m_dataId);
		}
	}
	
	int selectedRow = m_lCopyData.GetCount()-1;
	if(selectedRow >= 0 && selectedRow < m_lCopyData.GetCount())
	{
		m_lCopyData.SetSel(selectedRow);
		m_lCopyData.SetCurSel(selectedRow);
		m_lCopyData.SetCaretIndex(selectedRow);
		m_lCopyData.SetAnchorIndex(selectedRow);
	}

	//show the selected data md5
	OnLbnSelchangeCopyData();
	
	if(Clip.m_bIsGroup == FALSE)
	{
		::ShowWindow(::GetDlgItem(m_hWnd, IDC_STATIC_HOT_KEY_MOVE_TO_GROUP), SW_HIDE);
		::ShowWindow(::GetDlgItem(m_hWnd, IDC_HOTKEY_MOVE_TO_GROUP), SW_HIDE);
		::ShowWindow(::GetDlgItem(m_hWnd, IDC_CHECK_WIN_MOVE_TO_GROUP), SW_HIDE);
		::ShowWindow(::GetDlgItem(m_hWnd, IDC_HOT_KEY_GLOBAL_MOVE_TO_GROUP), SW_HIDE);

		CRect anchorRect;
		::GetWindowRect(::GetDlgItem(m_hWnd, IDC_STATIC_HOT_KEY_MOVE_TO_GROUP), &anchorRect);
		ScreenToClient(&anchorRect);
		

		HWND hwnd = ::GetTopWindow(this->GetSafeHwnd());
		// while we have a valid hwnd, 
		// loop through all child windows
		while (hwnd)
		{
			CRect rect;
			::GetWindowRect(hwnd, &rect);
			ScreenToClient(&rect);

			if(rect.top > anchorRect.bottom)
			{
				CDPI dpi(m_hWnd);
				const int gap = dpi.Scale(4);
				::MoveWindow(hwnd, rect.left, 
					rect.top - (anchorRect.Height()+gap), rect.Width(), 
					rect.Height(), TRUE);
			}

			// do something with the hwnd
			// and get the next child control's hwnd
			hwnd = ::GetNextWindow(hwnd, GW_HWNDNEXT);
		}

		CRect rect2;
		::GetWindowRect(m_hWnd, &rect2);

		::MoveWindow(m_hWnd, rect2.left, 
				rect2.top, rect2.Width(), 
				rect2.Height() - (anchorRect.Height()+CDPI(m_hWnd).Scale(4)), TRUE);
	}
}

void CCopyProperties::OnActivate(UINT nState, CWnd* pWndOther, BOOL bMinimized) 
{
	CThemedDialog::OnActivate(nState, pWndOther, bMinimized);

	if (nState == WA_INACTIVE)
	{
		if(m_bHideOnKillFocus)
		{
			if(!m_bHandleKillFocus)
			{
				EndDialog(-1);
				m_bHandleKillFocus = false;
			}
		}
	}
	else if (nState == WA_ACTIVE)
	{
		SetFocus();
		::SetForegroundWindow(m_hWnd);
	}

}
void CCopyProperties::OnOK() 
{
	UpdateData();

	try
	{
		if(m_lCopyID == -1 && m_pMemoryClip != NULL)
		{
			LoadDataIntoCClip(*m_pMemoryClip);

			m_DeletedData.SortDescending();
			INT_PTR count = m_DeletedData.GetSize();
			for(int i = 0; i < count; i++)
			{
				m_pMemoryClip->m_Formats.RemoveAt(m_DeletedData[i]);
			}
		}
		else
		{
			CClip clip;
			if(clip.LoadMainTable(m_lCopyID))
			{
				LoadDataIntoCClip(clip);

				if(CheckGlobalHotKey(clip) == FALSE)
				{
					if(MessageBox(_T("Error registering global hot key\n\nContinue?"), _T(""), MB_OKCANCEL |MB_ICONWARNING) != IDOK)
					{
						return;
					}
				}

				if(CheckMoveToGroupGlobalHotKey(clip) == FALSE)
				{
					if(MessageBox(_T("Error registering global move to group hot key\n\nContinue?"), _T(""), MB_OKCANCEL |MB_ICONWARNING) != IDOK)
					{
						return;
					}
				}

				if(clip.ModifyMainTable())
				{
					if(m_bDeletedData)
					{    
						DeleteFormats(m_lCopyID, m_DeletedData);
					}
				}
			}
		}

		m_bHandleKillFocus = true;
	}
	CATCH_SQLITE_EXCEPTION

	CThemedDialog::OnOK();
}

BOOL CCopyProperties::CheckGlobalHotKey(CClip &clip)
{
	BOOL ret = FALSE;

	if(clip.m_globalShortCut)
	{
		ret = g_HotKeys.ValidateClip(clip.m_id, clip.m_shortCut, clip.m_Desc, CHotKey::PASTE_OPEN_CLIP);
	}
	else
	{
		g_HotKeys.Remove(clip.m_id, CHotKey::PASTE_OPEN_CLIP);
		ret = TRUE;
	}

	return ret;
}

BOOL CCopyProperties::CheckMoveToGroupGlobalHotKey(CClip &clip)
{
	BOOL ret = FALSE;

	if(clip.m_globalMoveToGroupShortCut)
	{
		ret = g_HotKeys.ValidateClip(clip.m_id, clip.m_moveToGroupShortCut, clip.m_Desc, CHotKey::MOVE_TO_GROUP);
	}
	else
	{
		g_HotKeys.Remove(clip.m_id, CHotKey::MOVE_TO_GROUP);
		ret = TRUE;
	}

	return ret;
}

void CCopyProperties::LoadDataIntoCClip(CClip &Clip)
{
	long lHotKey = m_HotKey.GetHotKey();

	short sKeyKode = LOBYTE(m_HotKey.GetHotKey());
	short sModifers = (HIBYTE(m_HotKey.GetHotKey())) & ~HOTKEYF_EXT;

	if(sKeyKode && ::IsDlgButtonChecked(m_hWnd, IDC_CHECK_WIN))
	{
		sModifers |= HOTKEYF_EXT;
	}

	Clip.m_shortCut = MAKEWORD(sKeyKode, sModifers); 

	long moveToGroupHotKey = m_MoveToGrouHotKey.GetHotKey();

	short moveToGroupKeyKode = LOBYTE(m_MoveToGrouHotKey.GetHotKey());
	short moveToGroupModifers = HIBYTE(m_MoveToGrouHotKey.GetHotKey());

	if(moveToGroupKeyKode && ::IsDlgButtonChecked(m_hWnd, IDC_CHECK_WIN_MOVE_TO_GROUP))
	{
		moveToGroupModifers |= HOTKEYF_EXT;
	}

	Clip.m_moveToGroupShortCut = MAKEWORD(moveToGroupKeyKode, moveToGroupModifers); 

	//remove any others that have the same hot key
	if(Clip.m_shortCut > 0)
	{
		theApp.m_db.execDMLEx(_T("UPDATE Main SET lShortCut = 0 where lShortCut = %d AND lID <> %d;"), Clip.m_shortCut, m_lCopyID);
	}

	if(Clip.m_moveToGroupShortCut > 0)
	{
		theApp.m_db.execDMLEx(_T("UPDATE Main SET MoveToGroupShortCut = 0 where MoveToGroupShortCut = %d AND lID <> %d;"), Clip.m_shortCut, m_lCopyID);
	}

	m_description.GetWindowText(Clip.m_Desc);

	m_QuickPasteText.GetWindowText(Clip.m_csQuickPaste);	

	Clip.m_parentId = m_GroupCombo.GetItemDataFromCursel();

	//If we are going from no group to a group or the
	//don't auto delete check box is checked
	if(m_bNeverAutoDelete)
	{
		Clip.m_dontAutoDelete = (int)CTime::GetCurrentTime().GetTime();
	}
	else if(m_bNeverAutoDelete == FALSE)
	{
		Clip.m_dontAutoDelete = FALSE;
	}

	Clip.m_globalShortCut = m_hotKeyGlobal;

	Clip.m_globalMoveToGroupShortCut = m_moveToGroupHotKeyGlobal;
}

void CCopyProperties::OnDeleteCopyData() 
{
	int nCount = m_lCopyData.GetSelCount();
	if(nCount)
	{
		m_bDeletedData = true;

		//Get the selected indexes
		ARRAY items;
		items.SetSize(nCount);
		m_lCopyData.GetSelItems(nCount, items.GetData()); 

		items.SortDescending();

		//Get the selected itemdata
		for(int i = 0; i < nCount; i++)
		{
			int row = items[i];
			m_DeletedData.Add((int)m_lCopyData.GetItemData(row));
			m_lCopyData.DeleteString(row);

			int newRow = row-1;
			if(newRow < 0)
			{
				newRow = 0;
			}

			if(newRow >= 0 && newRow < m_lCopyData.GetCount())
			{
				m_lCopyData.SetSel(newRow);
				m_lCopyData.SetCurSel(newRow);
				m_lCopyData.SetCaretIndex(newRow);
				m_lCopyData.SetAnchorIndex(newRow);
			}
		}		
	}
}

void CCopyProperties::OnCancel() 
{
	m_bHandleKillFocus = true;
		
	CThemedDialog::OnCancel();
}

void CCopyProperties::OnSize(UINT nType, int cx, int cy) 
{
	CThemedDialog::OnSize(nType, cx, cy);
	
	if (((GetKeyState(VK_LBUTTON) & 0x100) != 0) &&
		m_mouseDownOnCaption == false)
	{
		m_Resize.MoveControls(CSize(cx, cy));
	}
	else
	{
		m_Resize.SetParent(m_hWnd);
	}
}

HBRUSH CCopyProperties::OnCtlColor(CDC* pDC, CWnd* pWnd, UINT nCtlColor)
{
	return CThemedControls::OnCtlColor(this, pDC, pWnd, nCtlColor);
}


void CCopyProperties::OnLbnSelchangeCopyData()
{
	int selCount = m_lCopyData.GetSelCount();
	if (selCount > 0)
	{
		m_bDeletedData = true;

		//Get the selected indexes
		ARRAY items;
		items.SetSize(selCount);
		m_lCopyData.GetSelItems(selCount, items.GetData());

		items.SortDescending();

		for (int i = 0; i < selCount; i++)
		{
			int row = items[i];
			int itemData = (int)m_lCopyData.GetItemData(row);

			CClip *pClip = NULL;
			if (m_lCopyID == -1 && m_pMemoryClip != NULL)
			{
				pClip = m_pMemoryClip;
			}
			else
			{
				pClip = &m_clip;
			}

			if (pClip != NULL)
			{
				CClipFormat* pCF;
				INT_PTR dataCount = pClip->m_Formats.GetSize();
				for (int i = 0; i < dataCount; i++)
				{
					pCF = &pClip->m_Formats.GetData()[i];
					if (pCF)
					{
						if (pCF->m_dataId == itemData)
						{
							CMd5 md5;
							md5.MD5Init();

							SIZE_T size = ::GlobalSize(pCF->Data());
							void* pv = GlobalLock(pCF->Data());
							if (pv != NULL)
							{
								md5.MD5Update((unsigned char*)pv, (unsigned int)size);

								GlobalUnlock(pCF->Data());

								CStringA md5String = md5.MD5FinalToString();

								this->SetDlgItemText(IDC_EDIT_MD5, CTextConvert::AnsiToUnicode(md5String));
							}
						}
					}
				}
			}

			break;
		}
	}
}


void CCopyProperties::OnNcLButtonDown(UINT nHitTest, CPoint point)
{
	m_mouseDownOnCaption = false;

	if (nHitTest == HTCAPTION)
	{
		m_mouseDownOnCaption = true;
	}

	CThemedDialog::OnNcLButtonDown(nHitTest, point);
}
