#include "stdafx.h"
#include "UiDraw.h"
#include "Misc.h"
#include <algorithm>
#include <cmath>
#include <map>
#include <memory>

#ifdef min
#undef min
#endif
#ifdef max
#undef max
#endif

namespace
{
	struct FontKey
	{
		CString family;
		int height;
		int weight;
		bool italic;

		bool operator<(const FontKey& other) const
		{
			if (family != other.family)
				return family < other.family;
			if (height != other.height)
				return height < other.height;
			if (weight != other.weight)
				return weight < other.weight;
			return italic < other.italic;
		}
	};

	struct PenKey
	{
		COLORREF color;
		int width;

		bool operator<(const PenKey& other) const
		{
			if (color != other.color)
				return color < other.color;
			return width < other.width;
		}
	};

	struct RoundRectKey
	{
		int width;
		int height;
		int radius;

		bool operator<(const RoundRectKey& other) const
		{
			if (width != other.width)
				return width < other.width;
			if (height != other.height)
				return height < other.height;
			return radius < other.radius;
		}
	};

	struct GlyphKey
	{
		UiDraw::Glyph glyph;
		int width;
		int height;
		COLORREF color;
		int strokeTenths;
		int dpi;

		bool operator<(const GlyphKey& other) const
		{
			if (glyph != other.glyph)
				return glyph < other.glyph;
			if (width != other.width)
				return width < other.width;
			if (height != other.height)
				return height < other.height;
			if (color != other.color)
				return color < other.color;
			if (strokeTenths != other.strokeTenths)
				return strokeTenths < other.strokeTenths;
			return dpi < other.dpi;
		}
	};

	struct TintKey
	{
		const Gdiplus::Bitmap* source;
		int width;
		int height;
		COLORREF color;
		int alpha;

		bool operator<(const TintKey& other) const
		{
			if (source != other.source)
				return source < other.source;
			if (width != other.width)
				return width < other.width;
			if (height != other.height)
				return height < other.height;
			if (color != other.color)
				return color < other.color;
			return alpha < other.alpha;
		}
	};

	struct AlphaBrushKey
	{
		COLORREF color;
		int alpha;

		bool operator<(const AlphaBrushKey& other) const
		{
			if (color != other.color)
				return color < other.color;
			return alpha < other.alpha;
		}
	};

	std::map<FontKey, HFONT> g_fonts;
	std::map<COLORREF, HBRUSH> g_brushes;
	std::map<PenKey, HPEN> g_pens;
	std::map<COLORREF, std::unique_ptr<Gdiplus::SolidBrush>> g_gdiplusBrushes;
	std::map<AlphaBrushKey, std::unique_ptr<Gdiplus::SolidBrush>> g_gdiplusAlphaBrushes;
	std::map<PenKey, std::unique_ptr<Gdiplus::Pen>> g_gdiplusPens;
	std::map<RoundRectKey, std::unique_ptr<Gdiplus::GraphicsPath>> g_roundRects;
	std::map<GlyphKey, std::unique_ptr<Gdiplus::Bitmap>> g_glyphs;
	std::map<TintKey, std::unique_ptr<Gdiplus::Bitmap>> g_tinted;

	Gdiplus::Color ToColor(COLORREF color, BYTE alpha = 255)
	{
		return Gdiplus::Color(alpha, GetRValue(color), GetGValue(color), GetBValue(color));
	}

	Gdiplus::SolidBrush* GetGdiplusBrush(COLORREF color)
	{
		auto it = g_gdiplusBrushes.find(color);
		if (it != g_gdiplusBrushes.end())
			return it->second.get();

		std::unique_ptr<Gdiplus::SolidBrush> brush(new Gdiplus::SolidBrush(ToColor(color)));
		Gdiplus::SolidBrush* result = brush.get();
		g_gdiplusBrushes[color] = std::move(brush);
		return result;
	}

	Gdiplus::Pen* GetGdiplusPen(COLORREF color, int width)
	{
		PenKey key = { color, width };
		auto it = g_gdiplusPens.find(key);
		if (it != g_gdiplusPens.end())
			return it->second.get();

		std::unique_ptr<Gdiplus::Pen> pen(new Gdiplus::Pen(ToColor(color), static_cast<Gdiplus::REAL>(width)));
		pen->SetAlignment(Gdiplus::PenAlignmentInset);
		pen->SetLineJoin(Gdiplus::LineJoinRound);
		Gdiplus::Pen* result = pen.get();
		g_gdiplusPens[key] = std::move(pen);
		return result;
	}

	Gdiplus::GraphicsPath* GetRoundRectPath(int width, int height, int radius)
	{
		width = std::max(1, width);
		height = std::max(1, height);
		radius = std::max(0, std::min(radius, std::min(width, height) / 2));
		RoundRectKey key = { width, height, radius };
		auto it = g_roundRects.find(key);
		if (it != g_roundRects.end())
			return it->second.get();

		std::unique_ptr<Gdiplus::GraphicsPath> path(new Gdiplus::GraphicsPath());
		if (radius == 0)
		{
			path->AddRectangle(Gdiplus::Rect(0, 0, width, height));
		}
		else
		{
			Gdiplus::REAL diameter = static_cast<Gdiplus::REAL>(radius * 2);
			path->AddArc(0.0f, 0.0f, diameter, diameter, 180.0f, 90.0f);
			path->AddArc(static_cast<Gdiplus::REAL>(width) - diameter, 0.0f,
				diameter, diameter, 270.0f, 90.0f);
			path->AddArc(static_cast<Gdiplus::REAL>(width) - diameter,
				static_cast<Gdiplus::REAL>(height) - diameter, diameter, diameter, 0.0f, 90.0f);
			path->AddArc(0.0f, static_cast<Gdiplus::REAL>(height) - diameter,
				diameter, diameter, 90.0f, 90.0f);
			path->CloseFigure();
		}

		Gdiplus::GraphicsPath* result = path.get();
		g_roundRects[key] = std::move(path);
		return result;
	}

	void AddLine(Gdiplus::GraphicsPath& path, float x1, float y1, float x2, float y2)
	{
		path.AddLine(x1, y1, x2, y2);
	}

	std::unique_ptr<Gdiplus::Bitmap> BuildGlyph(UiDraw::Glyph glyph, int width, int height,
		COLORREF color, int dpi, int strokeTenths)
	{
		std::unique_ptr<Gdiplus::Bitmap> bitmap(
			new Gdiplus::Bitmap(width, height, PixelFormat32bppPARGB));
		Gdiplus::Graphics graphics(bitmap.get());
		graphics.SetSmoothingMode(Gdiplus::SmoothingModeAntiAlias);
		graphics.SetPixelOffsetMode(Gdiplus::PixelOffsetModeHighQuality);
		graphics.Clear(Gdiplus::Color(0, 0, 0, 0));

		float s = static_cast<float>(std::min(width, height));
		float stroke = std::max(1.0f,
			(static_cast<float>(strokeTenths) / 10.0f) *
			(static_cast<float>(dpi) / 96.0f));
		Gdiplus::Pen pen(ToColor(color), stroke);
		pen.SetStartCap(Gdiplus::LineCapRound);
		pen.SetEndCap(Gdiplus::LineCapRound);
		pen.SetLineJoin(Gdiplus::LineJoinRound);
		Gdiplus::SolidBrush brush(ToColor(color));

		Gdiplus::GraphicsPath path;
		switch (glyph)
		{
		case UiDraw::Glyph::Close:
			AddLine(path, s * 0.28f, s * 0.28f, s * 0.72f, s * 0.72f);
			AddLine(path, s * 0.72f, s * 0.28f, s * 0.28f, s * 0.72f);
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::ChevronUp:
			AddLine(path, s * 0.24f, s * 0.62f, s * 0.50f, s * 0.36f);
			AddLine(path, s * 0.50f, s * 0.36f, s * 0.76f, s * 0.62f);
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::ChevronDown:
			AddLine(path, s * 0.24f, s * 0.38f, s * 0.50f, s * 0.64f);
			AddLine(path, s * 0.50f, s * 0.64f, s * 0.76f, s * 0.38f);
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::ChevronLeft:
			AddLine(path, s * 0.62f, s * 0.24f, s * 0.36f, s * 0.50f);
			AddLine(path, s * 0.36f, s * 0.50f, s * 0.62f, s * 0.76f);
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::ChevronRight:
			AddLine(path, s * 0.38f, s * 0.24f, s * 0.64f, s * 0.50f);
			AddLine(path, s * 0.64f, s * 0.50f, s * 0.38f, s * 0.76f);
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::Search:
			graphics.DrawEllipse(&pen, s * 0.20f, s * 0.18f, s * 0.42f, s * 0.42f);
			AddLine(path, s * 0.58f, s * 0.56f, s * 0.80f, s * 0.78f);
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::Folder:
			path.AddLine(s * 0.16f, s * 0.32f, s * 0.40f, s * 0.32f);
			path.AddLine(s * 0.40f, s * 0.32f, s * 0.47f, s * 0.40f);
			path.AddLine(s * 0.47f, s * 0.40f, s * 0.82f, s * 0.40f);
			path.AddLine(s * 0.82f, s * 0.40f, s * 0.82f, s * 0.74f);
			path.AddLine(s * 0.82f, s * 0.74f, s * 0.16f, s * 0.74f);
			path.CloseFigure();
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::Cog:
			graphics.DrawEllipse(&pen, s * 0.34f, s * 0.34f, s * 0.32f, s * 0.32f);
			for (int i = 0; i < 8; ++i)
			{
				float angle = static_cast<float>(i) * 3.14159265f / 4.0f;
				float x1 = s * 0.50f + cosf(angle) * s * 0.26f;
				float y1 = s * 0.50f + sinf(angle) * s * 0.26f;
				float x2 = s * 0.50f + cosf(angle) * s * 0.38f;
				float y2 = s * 0.50f + sinf(angle) * s * 0.38f;
				AddLine(path, x1, y1, x2, y2);
			}
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::Check:
			AddLine(path, s * 0.20f, s * 0.52f, s * 0.42f, s * 0.72f);
			AddLine(path, s * 0.42f, s * 0.72f, s * 0.80f, s * 0.28f);
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::Pin:
			path.AddLine(s * 0.36f, s * 0.18f, s * 0.64f, s * 0.18f);
			path.AddLine(s * 0.64f, s * 0.18f, s * 0.58f, s * 0.36f);
			path.AddLine(s * 0.58f, s * 0.36f, s * 0.72f, s * 0.52f);
			path.AddLine(s * 0.72f, s * 0.52f, s * 0.28f, s * 0.52f);
			path.AddLine(s * 0.28f, s * 0.52f, s * 0.42f, s * 0.36f);
			path.AddLine(s * 0.42f, s * 0.36f, s * 0.36f, s * 0.18f);
			AddLine(path, s * 0.50f, s * 0.52f, s * 0.50f, s * 0.82f);
			graphics.DrawPath(&pen, &path);
			break;
		case UiDraw::Glyph::Dots:
		case UiDraw::Glyph::SystemMenu:
			graphics.FillEllipse(&brush, s * 0.18f, s * 0.46f, s * 0.12f, s * 0.12f);
			graphics.FillEllipse(&brush, s * 0.44f, s * 0.46f, s * 0.12f, s * 0.12f);
			graphics.FillEllipse(&brush, s * 0.70f, s * 0.46f, s * 0.12f, s * 0.12f);
			break;
		case UiDraw::Glyph::Menu:
			AddLine(path, s * 0.20f, s * 0.28f, s * 0.80f, s * 0.28f);
			AddLine(path, s * 0.20f, s * 0.50f, s * 0.80f, s * 0.50f);
			AddLine(path, s * 0.20f, s * 0.72f, s * 0.80f, s * 0.72f);
			graphics.DrawPath(&pen, &path);
			break;
		}

		return bitmap;
	}

	Gdiplus::Bitmap* GetGlyphBitmap(UiDraw::Glyph glyph, int width, int height,
		COLORREF color, int dpi, int strokeTenths)
	{
		width = std::max(1, width);
		height = std::max(1, height);
		GlyphKey key = { glyph, width, height, color, strokeTenths, dpi };
		auto it = g_glyphs.find(key);
		if (it != g_glyphs.end())
			return it->second.get();

		std::unique_ptr<Gdiplus::Bitmap> bitmap =
			BuildGlyph(glyph, width, height, color, dpi, strokeTenths);
		Gdiplus::Bitmap* result = bitmap.get();
		g_glyphs[key] = std::move(bitmap);
		return result;
	}

	Gdiplus::Bitmap* GetTintedBitmap(Gdiplus::Bitmap* source, int width, int height,
		COLORREF color, int alpha)
	{
		if (source == NULL)
			return NULL;

		TintKey key = { source, width, height, color, alpha };
		auto it = g_tinted.find(key);
		if (it != g_tinted.end())
			return it->second.get();

		std::unique_ptr<Gdiplus::Bitmap> bitmap(
			new Gdiplus::Bitmap(width, height, PixelFormat32bppPARGB));
		Gdiplus::Graphics graphics(bitmap.get());
		graphics.SetSmoothingMode(Gdiplus::SmoothingModeHighQuality);
		graphics.SetInterpolationMode(Gdiplus::InterpolationModeHighQualityBicubic);
		graphics.Clear(Gdiplus::Color(0, 0, 0, 0));

		Gdiplus::ColorMatrix matrix = {
			0.0f, 0.0f, 0.0f, 0.0f, 0.0f,
			0.0f, 0.0f, 0.0f, 0.0f, 0.0f,
			0.0f, 0.0f, 0.0f, 0.0f, 0.0f,
			0.0f, 0.0f, 0.0f, alpha / 255.0f, 0.0f,
			GetRValue(color) / 255.0f,
			GetGValue(color) / 255.0f,
			GetBValue(color) / 255.0f,
			0.0f,
			1.0f
		};
		Gdiplus::ImageAttributes attributes;
		attributes.SetColorMatrix(&matrix);
		graphics.DrawImage(source,
			Gdiplus::Rect(0, 0, width, height),
			0, 0, source->GetWidth(), source->GetHeight(),
			Gdiplus::UnitPixel, &attributes);

		Gdiplus::Bitmap* result = bitmap.get();
		g_tinted[key] = std::move(bitmap);
		return result;
	}
}

int UiDraw::Scale(int logicalPixels, int dpi)
{
	return MulDiv(logicalPixels, std::max(1, dpi), 96);
}

int UiDraw::IconSizeForDpi(int dpi)
{
	if (dpi <= 120)
		return 16;
	if (dpi <= 168)
		return 20;
	return 24;
}

HFONT UiDraw::Font(const CString& family, int logicalPixels, int weight, int dpi, bool italic)
{
	FontKey key = { family, -Scale(logicalPixels, dpi), weight, italic };
	auto it = g_fonts.find(key);
	if (it != g_fonts.end())
		return it->second;

	LOGFONT lf = { 0 };
	lf.lfHeight = key.height;
	lf.lfWeight = weight;
	lf.lfItalic = italic ? TRUE : FALSE;
	lf.lfCharSet = DEFAULT_CHARSET;
	lf.lfQuality = CLEARTYPE_QUALITY;
	_tcsncpy_s(lf.lfFaceName, LF_FACESIZE, family, _TRUNCATE);

	HFONT font = CreateFontIndirect(&lf);
	g_fonts[key] = font;
	return font;
}

HBRUSH UiDraw::Brush(COLORREF color)
{
	auto it = g_brushes.find(color);
	if (it != g_brushes.end())
		return it->second;

	HBRUSH brush = CreateSolidBrush(color);
	g_brushes[color] = brush;
	return brush;
}

HPEN UiDraw::Pen(COLORREF color, int widthPixels)
{
	PenKey key = { color, std::max(1, widthPixels) };
	auto it = g_pens.find(key);
	if (it != g_pens.end())
		return it->second;

	HPEN pen = CreatePen(PS_SOLID, key.width, color);
	g_pens[key] = pen;
	return pen;
}

Gdiplus::SolidBrush* UiDraw::AlphaBrush(COLORREF color, int alpha)
{
	alpha = std::max(0, std::min(255, alpha));
	AlphaBrushKey key = { color, alpha };
	auto it = g_gdiplusAlphaBrushes.find(key);
	if (it != g_gdiplusAlphaBrushes.end())
		return it->second.get();

	std::unique_ptr<Gdiplus::SolidBrush> brush(new Gdiplus::SolidBrush(
		ToColor(color, static_cast<BYTE>(alpha))));
	Gdiplus::SolidBrush* result = brush.get();
#ifdef _DEBUG
	if (g_gdiplusAlphaBrushes.empty())
	{
		Log(_T("T3 UiDraw created translucent brush cache"));
	}
#endif
	g_gdiplusAlphaBrushes[key] = std::move(brush);
	return result;
}

CSize UiDraw::MeasureText(CDC* dc, const CString& text, const CString& family,
	int logicalPixels, int weight, int dpi, bool italic)
{
	if (dc == NULL)
		return CSize(0, 0);

	HFONT font = Font(family, logicalPixels, weight, dpi, italic);
	HFONT oldFont = static_cast<HFONT>(dc->SelectObject(font));
	CSize size = dc->GetTextExtent(text);
	dc->SelectObject(oldFont);
	return size;
}

int UiDraw::DrawTextEllipsis(CDC* dc, const CString& text, const CRect& rect,
	const CString& family, int logicalPixels, int weight, COLORREF color,
	int dpi, UINT drawFlags, bool italic)
{
	if (dc == NULL)
		return 0;

	HFONT font = Font(family, logicalPixels, weight, dpi, italic);
	HFONT oldFont = static_cast<HFONT>(dc->SelectObject(font));
	COLORREF oldColor = dc->SetTextColor(color);
	dc->SetBkMode(TRANSPARENT);

	CRect drawRect(rect);
	int height = dc->DrawText(text, &drawRect,
		drawFlags | DT_END_ELLIPSIS | DT_NOPREFIX);

	dc->SetTextColor(oldColor);
	dc->SelectObject(oldFont);
	return height;
}

void UiDraw::RoundedRect(Gdiplus::Graphics& graphics, const CRect& rect, int radiusPixels,
	COLORREF fillColor, COLORREF borderColor, int borderWidthPixels)
{
	if (rect.Width() <= 0 || rect.Height() <= 0)
		return;

	Gdiplus::GraphicsPath* path =
		GetRoundRectPath(rect.Width(), rect.Height(), radiusPixels);
	if (path == NULL)
		return;

	Gdiplus::GraphicsState state = graphics.Save();
	graphics.SetSmoothingMode(Gdiplus::SmoothingModeAntiAlias);
	graphics.TranslateTransform(static_cast<Gdiplus::REAL>(rect.left),
		static_cast<Gdiplus::REAL>(rect.top));

	if (fillColor != CLR_INVALID)
		graphics.FillPath(GetGdiplusBrush(fillColor), path);
	if (borderColor != CLR_INVALID && borderWidthPixels > 0)
		graphics.DrawPath(GetGdiplusPen(borderColor, borderWidthPixels), path);

	graphics.Restore(state);
}

void UiDraw::RoundedRect(CDC* dc, const CRect& rect, int radiusPixels,
	COLORREF fillColor, COLORREF borderColor, int borderWidthPixels)
{
	if (dc == NULL || dc->GetSafeHdc() == NULL)
		return;

	Gdiplus::Graphics graphics(dc->GetSafeHdc());
	RoundedRect(graphics, rect, radiusPixels, fillColor, borderColor, borderWidthPixels);
}

void UiDraw::FocusRing(Gdiplus::Graphics& graphics, const CRect& rect, int radiusPixels,
	COLORREF color, int widthPixels)
{
	RoundedRect(graphics, rect, radiusPixels, CLR_INVALID, color,
		std::max(1, widthPixels));
}

void UiDraw::FocusRing(CDC* dc, const CRect& rect, int radiusPixels,
	COLORREF color, int widthPixels)
{
	if (dc == NULL || dc->GetSafeHdc() == NULL)
		return;

	Gdiplus::Graphics graphics(dc->GetSafeHdc());
	FocusRing(graphics, rect, radiusPixels, color, widthPixels);
}

void UiDraw::DrawFallbackBorder(CDC* dc, const CRect& rect, COLORREF color)
{
	if (dc == NULL)
		return;

	RECT fallbackRect = rect;
	::FrameRect(dc->GetSafeHdc(), &fallbackRect, Brush(color));
}

void UiDraw::DrawGlyph(Gdiplus::Graphics& graphics, const CRect& rect, Glyph glyph,
	COLORREF color, int dpi, int strokeTenths)
{
	if (rect.Width() <= 0 || rect.Height() <= 0)
		return;

	Gdiplus::Bitmap* bitmap =
		GetGlyphBitmap(glyph, rect.Width(), rect.Height(), color, dpi, strokeTenths);
	if (bitmap == NULL)
		return;

	graphics.SetSmoothingMode(Gdiplus::SmoothingModeAntiAlias);
	graphics.DrawImage(bitmap, rect.left, rect.top, rect.Width(), rect.Height());
}

void UiDraw::DrawGlyph(CDC* dc, const CRect& rect, Glyph glyph,
	COLORREF color, int dpi, int strokeTenths)
{
	if (dc == NULL || dc->GetSafeHdc() == NULL)
		return;

	Gdiplus::Graphics graphics(dc->GetSafeHdc());
	DrawGlyph(graphics, rect, glyph, color, dpi, strokeTenths);
}

void UiDraw::DrawIcon(CDC* dc, Gdiplus::Bitmap* bitmap, const CRect& rect,
	bool tinted, COLORREF tint, int alpha)
{
	if (dc == NULL || bitmap == NULL || rect.Width() <= 0 || rect.Height() <= 0)
		return;

	Gdiplus::Graphics graphics(dc->GetSafeHdc());
	graphics.SetInterpolationMode(Gdiplus::InterpolationModeHighQualityBicubic);

	UINT sourceWidth = bitmap->GetWidth();
	UINT sourceHeight = bitmap->GetHeight();
	if (sourceWidth == 0 || sourceHeight == 0)
		return;

	double scale = std::min(
		static_cast<double>(rect.Width()) / sourceWidth,
		static_cast<double>(rect.Height()) / sourceHeight);
	int width = std::max(1, static_cast<int>(sourceWidth * scale + 0.5));
	int height = std::max(1, static_cast<int>(sourceHeight * scale + 0.5));
	int x = rect.left + (rect.Width() - width) / 2;
	int y = rect.top + (rect.Height() - height) / 2;

	Gdiplus::Bitmap* drawBitmap = tinted ?
		GetTintedBitmap(bitmap, width, height, tint, alpha) : bitmap;
	if (drawBitmap == NULL)
		return;

	graphics.DrawImage(drawBitmap, x, y, width, height);
}

void UiDraw::PurgeCaches()
{
	for (auto& entry : g_fonts)
		DeleteObject(entry.second);
	g_fonts.clear();

	for (auto& entry : g_brushes)
		DeleteObject(entry.second);
	g_brushes.clear();

	for (auto& entry : g_pens)
		DeleteObject(entry.second);
	g_pens.clear();

	g_gdiplusBrushes.clear();
	if (!g_gdiplusAlphaBrushes.empty())
	{
		Log(_T("T3 UiDraw purged translucent brush cache"));
	}
	g_gdiplusAlphaBrushes.clear();
	g_gdiplusPens.clear();
	g_roundRects.clear();
	g_glyphs.clear();
	g_tinted.clear();
}
