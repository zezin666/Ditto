#pragma once

#include "ThemedControls.h"

// The common classes keep MFC's existing construction/DDX contracts while
// installing the shared painter and layout pass after MFC creates the dialog.
class CThemedDialog : public CDialogEx
{
	DECLARE_DYNAMIC(CThemedDialog)

public:
	CThemedDialog(UINT nIDTemplate, CWnd* pParent = NULL);
	virtual ~CThemedDialog();

	void RefreshThemeNow();

protected:
	virtual BOOL OnInitDialog();

	DECLARE_MESSAGE_MAP()
};

class CThemedPropertyPage : public CPropertyPage
{
	DECLARE_DYNAMIC(CThemedPropertyPage)

public:
	CThemedPropertyPage(UINT nIDTemplate);
	virtual ~CThemedPropertyPage();

	void RefreshThemeNow();

protected:
	virtual BOOL OnInitDialog();

	DECLARE_MESSAGE_MAP()
};

class CThemedPropertySheet : public CPropertySheet
{
	DECLARE_DYNAMIC(CThemedPropertySheet)

public:
	CThemedPropertySheet(LPCTSTR pszCaption, CWnd* pParentWnd = NULL, UINT iSelectPage = 0);
	virtual ~CThemedPropertySheet();

	void RefreshThemeNow();

protected:
	virtual BOOL OnInitDialog();

	DECLARE_MESSAGE_MAP()
};
