// AddType.cpp : implementation file
//

#include "stdafx.h"
#include "cp_main.h"
#include "Misc.h"
#include "AddType.h"
#include "DPI.h"
#include "UiDraw.h"
#include "UiTheme.h"
#include <algorithm>

namespace
{
	void FitRadioButtonToText(CWnd* pDialog, UINT controlId)
	{
		CWnd* pControl = pDialog->GetDlgItem(controlId);
		if (pControl == NULL || pControl->GetSafeHwnd() == NULL)
			return;

		CString text;
		pControl->GetWindowText(text);
		CDPI dpi(pDialog->GetSafeHwnd());
		CRect controlRect;
		pControl->GetWindowRect(controlRect);
		pDialog->ScreenToClient(controlRect);

		CClientDC dc(pControl);
		const CSize textSize = UiDraw::MeasureText(&dc, text,
			UiTheme().FontFamilyUi(), UiTheme().FontSizeBody(), FW_NORMAL,
			dpi.GetDPI());
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
}

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// CAddType dialog


CAddType::CAddType(CWnd* pParent /*=NULL*/)
	: CThemedDialog(CAddType::IDD, pParent)
{
	//{{AFX_DATA_INIT(CAddType)
	m_eCustomType = _T("");
	//}}AFX_DATA_INIT
}


void CAddType::DoDataExchange(CDataExchange* pDX)
{
	CThemedDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(CAddType)
	DDX_Control(pDX, IDC_LIST1, m_lbCandidateTypes);
	DDX_Text(pDX, IDC_EDIT1, m_eCustomType);
	DDV_MaxChars(pDX, m_eCustomType, 50);
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(CAddType, CThemedDialog)
	//{{AFX_MSG_MAP(CAddType)
	//}}AFX_MSG_MAP
	ON_BN_CLICKED(IDC_RADIO_PRIMARY_TYPES, &CAddType::OnBnClickedRadioPrimaryTypes)
	ON_BN_CLICKED(IDC_RADIO_CURRENT_TYPES, &CAddType::OnBnClickedRadioCurrentTypes)
	ON_BN_CLICKED(IDC_RADIO_CUSTOM_TYPE, &CAddType::OnBnClickedRadioCustomType)
	ON_BN_CLICKED(IDC_ADD_3, &CAddType::OnBnClickedAdd)
	ON_LBN_DBLCLK(IDC_LIST1, &CAddType::OnLbnDblclkList)
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// CAddType message handlers
BOOL CAddType::OnInitDialog() 
{
	CThemedDialog::OnInitDialog();		
	
	::CheckDlgButton(m_hWnd, IDC_RADIO_CURRENT_TYPES, BST_CHECKED);
	OnBnClickedRadioCurrentTypes();
	
	m_lbCandidateTypes.SetFocus();

	theApp.m_Language.UpdateOptionSupportedTypesAdd(this);
	SendMessage(DM_SETDEFID, IDC_ADD_3, 0);
	if (CWnd* addButton = GetDlgItem(IDC_ADD_3))
		addButton->ModifyStyle(BS_TYPEMASK, BS_DEFPUSHBUTTON);
	CThemedControls::FinishLayout(this);
	FitRadioButtonToText(this, IDC_RADIO_PRIMARY_TYPES);
	FitRadioButtonToText(this, IDC_RADIO_CURRENT_TYPES);
	FitRadioButtonToText(this, IDC_RADIO_CUSTOM_TYPE);
	return FALSE;
}

void CAddType::AddCurrentClipboardTypes()
{
	m_lbCandidateTypes.ResetContent();

	COleDataObject oleData;

	if (!oleData.AttachClipboard())
		return;

	oleData.BeginEnumFormats();

	FORMATETC test;

	while (oleData.GetNextFormat(&test))
	{
		BOOL b = oleData.IsDataAvailable(test.cfFormat);
		m_lbCandidateTypes.AddString(GetFormatName(test.cfFormat));
	}

	oleData.Release();
}

void CAddType::AddCommonTypes()
{
	m_lbCandidateTypes.ResetContent();
	for (auto systemClipFormat : GetSystemClipFormats()) {
		m_lbCandidateTypes.AddString(GetFormatName(systemClipFormat));
	}
	m_lbCandidateTypes.AddString(GetFormatName(RegisterClipboardFormat(CF_RTF)));
	m_lbCandidateTypes.AddString(GetFormatName(RegisterClipboardFormat(CF_RTFNOOBJS)));
	m_lbCandidateTypes.AddString(GetFormatName(RegisterClipboardFormat(CF_RETEXTOBJ)));
	m_lbCandidateTypes.AddString(GetFormatName(RegisterClipboardFormat(_T("HTML Format"))));
}

void CAddType::OnBnClickedRadioPrimaryTypes()
{
	AddCommonTypes();
	::ShowWindow(::GetDlgItem(m_hWnd, IDC_LIST1), SW_SHOW);
	::ShowWindow(::GetDlgItem(m_hWnd, IDC_EDIT1), SW_HIDE);
	m_lbCandidateTypes.SetFocus();
	if (m_lbCandidateTypes.GetCount() > 0)
	{
		m_lbCandidateTypes.SetCurSel(0);
		m_lbCandidateTypes.SetSel(0);
	}
}

void CAddType::OnBnClickedRadioCurrentTypes()
{
	AddCurrentClipboardTypes();
	::ShowWindow(::GetDlgItem(m_hWnd, IDC_LIST1), SW_SHOW);
	::ShowWindow(::GetDlgItem(m_hWnd, IDC_EDIT1), SW_HIDE);
	m_lbCandidateTypes.SetFocus();
	if (m_lbCandidateTypes.GetCount() > 0)
	{
		m_lbCandidateTypes.SetCurSel(0);
		m_lbCandidateTypes.SetSel(0);

	}
}

void CAddType::OnBnClickedRadioCustomType()
{
	::ShowWindow(::GetDlgItem(m_hWnd, IDC_EDIT1), SW_SHOW);
	::ShowWindow(::GetDlgItem(m_hWnd, IDC_LIST1), SW_HIDE);
	::SetFocus(::GetDlgItem(m_hWnd, IDC_EDIT1));
}

void CAddType::OnBnClickedAdd()
{
	m_csSelectedTypes.RemoveAll();

	if (IsDlgButtonChecked(IDC_RADIO_PRIMARY_TYPES) == BST_CHECKED ||
		IsDlgButtonChecked(IDC_RADIO_CURRENT_TYPES) == BST_CHECKED)
	{
		int nCount = m_lbCandidateTypes.GetSelCount();
		if (nCount)
		{
			CString cs;
			CArray<int, int> items;
			items.SetSize(nCount);
			m_lbCandidateTypes.GetSelItems(nCount, items.GetData());

			for (int i = 0; i < nCount; i++)
			{
				m_lbCandidateTypes.GetText(items[i], cs);
				m_csSelectedTypes.Add(cs);
			}
		}
	}
	else if (IsDlgButtonChecked(IDC_RADIO_CUSTOM_TYPE) == BST_CHECKED)
	{
		UpdateData();
		m_csSelectedTypes.Add(m_eCustomType);
	}

	EndDialog(IDOK);
}


void CAddType::OnLbnDblclkList()
{
	OnBnClickedAdd();
}
