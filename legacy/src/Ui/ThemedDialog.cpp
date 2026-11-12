#include "stdafx.h"
#include "ThemedDialog.h"

IMPLEMENT_DYNAMIC(CThemedDialog, CDialogEx)
IMPLEMENT_DYNAMIC(CThemedPropertyPage, CPropertyPage)
IMPLEMENT_DYNAMIC(CThemedPropertySheet, CPropertySheet)

BEGIN_MESSAGE_MAP(CThemedDialog, CDialogEx)
END_MESSAGE_MAP()

BEGIN_MESSAGE_MAP(CThemedPropertyPage, CPropertyPage)
END_MESSAGE_MAP()

BEGIN_MESSAGE_MAP(CThemedPropertySheet, CPropertySheet)
END_MESSAGE_MAP()

CThemedDialog::CThemedDialog(UINT nIDTemplate, CWnd* pParent)
	: CDialogEx(nIDTemplate, pParent)
{
}

CThemedDialog::~CThemedDialog()
{
}

BOOL CThemedDialog::OnInitDialog()
{
	BOOL result = CDialogEx::OnInitDialog();
	RefreshThemeNow();
	return result;
}

void CThemedDialog::RefreshThemeNow()
{
	if (GetSafeHwnd() != NULL)
	{
		CThemedControls::RefreshTheme(this);
	}
}

CThemedPropertyPage::CThemedPropertyPage(UINT nIDTemplate)
	: CPropertyPage(nIDTemplate)
{
}

CThemedPropertyPage::~CThemedPropertyPage()
{
}

BOOL CThemedPropertyPage::OnInitDialog()
{
	BOOL result = CPropertyPage::OnInitDialog();
	RefreshThemeNow();
	return result;
}

void CThemedPropertyPage::RefreshThemeNow()
{
	if (GetSafeHwnd() != NULL)
	{
		CThemedControls::RefreshTheme(this);
	}
}

CThemedPropertySheet::CThemedPropertySheet(LPCTSTR pszCaption, CWnd* pParentWnd, UINT iSelectPage)
	: CPropertySheet(pszCaption, pParentWnd, iSelectPage)
{
}

CThemedPropertySheet::~CThemedPropertySheet()
{
}

BOOL CThemedPropertySheet::OnInitDialog()
{
	BOOL result = CPropertySheet::OnInitDialog();
	RefreshThemeNow();
	return result;
}

void CThemedPropertySheet::RefreshThemeNow()
{
	if (GetSafeHwnd() != NULL)
	{
		CThemedControls::PreparePropertySheet(this);
	}
}
