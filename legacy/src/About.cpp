// About.cpp : implementation file
// test test test

//Main branch test

#include "stdafx.h"
#include "cp_main.h"
#include "About.h"
#include "misc.h"
#include "DPI.h"

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// CAbout property page

namespace
{
	void SetCurrentPageTabTitle(CWnd* pPage, const CString& title)
	{
		if (title.IsEmpty())
			return;

		CPropertySheet* pSheet = DYNAMIC_DOWNCAST(CPropertySheet, pPage->GetParent());
		CTabCtrl* pTab = pSheet != NULL ? pSheet->GetTabControl() : NULL;
		if (pSheet == NULL || pTab == NULL || pTab->GetSafeHwnd() == NULL)
			return;

		for (int index = 0; index < pSheet->GetPageCount(); ++index)
		{
			if (pSheet->GetPage(index) != pPage)
				continue;

			TCITEM item = { 0 };
			item.mask = TCIF_TEXT;
			item.pszText = const_cast<LPTSTR>(title.GetString());
			pTab->SetItem(index, &item);
			break;
		}
	}
}

IMPLEMENT_DYNCREATE(CAbout, CThemedPropertyPage)

CAbout::CAbout() : CThemedPropertyPage(CAbout::IDD)
{
	m_csTitle = theApp.m_Language.GetString("AboutTitle", "About");
	m_psp.pszTitle = m_csTitle;
	m_psp.dwFlags |= PSP_USETITLE;

	//{{AFX_DATA_INIT(CAbout)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT
}

CAbout::~CAbout()
{
}

void CAbout::DoDataExchange(CDataExchange* pDX)
{
	CThemedPropertyPage::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(CAbout)
	DDX_Control(pDX, IDC_STATIC_LINK, m_Link);
	DDX_Control(pDX, IDC_HYPER_LINK, m_HyperLink);
	DDX_Control(pDX, IDC_LIST, m_List);
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(CAbout, CThemedPropertyPage)
	//{{AFX_MSG_MAP(CAbout)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// CAbout message handlers

BOOL CAbout::OnInitDialog() 
{
	CThemedPropertyPage::OnInitDialog();

	m_List.AddString(_T("Ditto"));

	auto runningVersion = GetRunningVersion();
	CString cs = GetVersionString(runningVersion);

	CString csText;
	csText = "    Version " + cs;
#if _WIN64
	csText += " 64bit";
#else
	csText += " 32bit";
#endif
	m_List.AddString(csText);

	const char *SqliteVersion = sqlite3_libversion();
	csText = "    Sqlite Version ";
	csText += SqliteVersion;
	m_List.AddString(csText);

	cs = CGetSetOptions::GetExeFileName();;
	csText = "    Exe Path " + cs;
	m_List.AddString(csText);

	cs = CGetSetOptions::GetDBPath();
	csText = "    DB Path " + cs;
	m_List.AddString(csText);

	m_List.AddString(_T(""));

	m_List.AddString(_T("Credits"));
	
	cs = "    Authors - Scott Brogden, sabrogden@users.sourceforge.net";
	m_List.AddString(cs);
	cs = "            - Kevin Edwards, ingenuus@users.sourceforge.net";
	m_List.AddString(cs);

	m_List.AddString(_T(""));

	m_List.AddString(_T("Addins"));

	//Show what addins are loaded
	CStringArray arr;
	theApp.m_Addins.AboutScreenText(arr);
	INT_PTR count = arr.GetCount();
	for(int i = 0; i < count; i++)
	{
		m_List.AddString(_T("    ") + arr[i]);
	}

	CDPI dpi(m_hWnd);
	const int padding = dpi.Scale(7);
	const int gap = dpi.Scale(5);
	const int linkHeight = dpi.Scale(18);

	CRect rect;
	GetClientRect(rect);
	rect.DeflateRect(padding, padding);

	CRect homeRect(rect.left, rect.bottom - linkHeight, rect.right, rect.bottom);
	CRect emailRect(rect.left, homeRect.top - linkHeight - gap,
		rect.right, homeRect.top - gap);
	CRect listRect(rect.left, rect.top, rect.right, emailRect.top - gap);

	m_List.MoveWindow(listRect);
	m_HyperLink.MoveWindow(emailRect);
	m_Link.MoveWindow(homeRect);

	m_HyperLink.SetURL(_T("mailto:sabrogden@gmail.com"));

	m_Link.SetURL(_T("https://ditto-cp.sourceforge.io/"));

	CThemedControls::FinishLayout(this);
	SetCurrentPageTabTitle(this, m_csTitle);
		
	return TRUE;  // return TRUE unless you set the focus to a control
	              // EXCEPTION: OCX Property Pages should return FALSE
}
