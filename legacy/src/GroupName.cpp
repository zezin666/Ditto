// GroupName.cpp : implementation file
//

#include "stdafx.h"
#include "cp_main.h"
#include "GroupName.h"

#ifdef _DEBUG
#define new DEBUG_NEW
#undef THIS_FILE
static char THIS_FILE[] = __FILE__;
#endif

/////////////////////////////////////////////////////////////////////////////
// CGroupName dialog


CGroupName::CGroupName(CWnd* pParent /*=NULL*/)
	: CThemedDialog(CGroupName::IDD, pParent)
{
	//{{AFX_DATA_INIT(CGroupName)
	m_csName = _T("");
	//}}AFX_DATA_INIT
}


void CGroupName::DoDataExchange(CDataExchange* pDX)
{
	CThemedDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(CGroupName)
	DDX_Text(pDX, IDC_NAME, m_csName);
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(CGroupName, CThemedDialog)
	//{{AFX_MSG_MAP(CGroupName)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// CGroupName message handlers

void CGroupName::OnOK() 
{
	UpdateData(TRUE);
	
	CThemedDialog::OnOK();
}

BOOL CGroupName::OnInitDialog() 
{
	CThemedDialog::OnInitDialog();

	CWnd *pWnd = GetDlgItem(IDC_NAME);
	if(pWnd)
		pWnd->SetFocus();

	CThemedControls::FinishLayout(this);
		
	return FALSE;
}
