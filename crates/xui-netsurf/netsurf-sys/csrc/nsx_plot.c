/*
 * nsx drawing: NetSurf's plotter, bitmap and layout (text measuring) tables,
 * each forwarding to the host as plain values.
 */
#include <stdbool.h>
#include <stdlib.h>

#include <libwapcaplet/libwapcaplet.h>

#include "utils/errors.h"
#include "netsurf/css.h"
#include "netsurf/plotters.h"
#include "netsurf/types.h"

#include "nsx_internal.h"

/* ---- styles ---------------------------------------------------------- */

uint32_t nsx_colour(colour c)
{
	if (c == NS_TRANSPARENT) {
		return 0;
	}
	return ns_color_to_nscss(c);
}

static void style_from(const plot_style_t *ps, nsx_style *out)
{
	out->fill_kind = (int)ps->fill_type;
	out->fill = nsx_colour(ps->fill_colour);
	out->stroke_kind = (int)ps->stroke_type;
	out->stroke = nsx_colour(ps->stroke_colour);
	out->stroke_width = plot_style_fixed_to_float(ps->stroke_width);
}

void nsx_font_from_style(const plot_font_style_t *fstyle, nsx_font *out)
{
	out->family = NULL;
	out->family_len = 0;
	if (fstyle->families != NULL && fstyle->families[0] != NULL) {
		out->family = lwc_string_data(fstyle->families[0]);
		out->family_len = lwc_string_length(fstyle->families[0]);
	}
	out->generic = (int)fstyle->family;
	/* Points to CSS pixels at the 96 dpi nsx_init sets. */
	out->size_px = plot_style_fixed_to_float(fstyle->size) * 96.0f / 72.0f;
	out->weight = fstyle->weight;
	out->italic = (fstyle->flags & (FONTF_ITALIC | FONTF_OBLIQUE)) != 0;
}

/* ---- plotters -------------------------------------------------------- */

static const nsx_sink *sink_of(const struct redraw_context *ctx)
{
	return ctx->priv;
}

static nserror p_clip(const struct redraw_context *ctx, const struct rect *r)
{
	const nsx_sink *s = sink_of(ctx);
	s->clip(s->rec, r->x0, r->y0, r->x1, r->y1);
	return NSERROR_OK;
}

static nserror p_arc(const struct redraw_context *ctx, const plot_style_t *ps,
		int x, int y, int radius, int angle1, int angle2)
{
	/* Only used for SVG and native widgets this build does not draw. */
	(void)ctx; (void)ps; (void)x; (void)y;
	(void)radius; (void)angle1; (void)angle2;
	return NSERROR_OK;
}

static nserror p_disc(const struct redraw_context *ctx, const plot_style_t *ps,
		int x, int y, int radius)
{
	const nsx_sink *s = sink_of(ctx);
	nsx_style st;
	style_from(ps, &st);
	s->disc(s->rec, x, y, radius, &st);
	return NSERROR_OK;
}

static nserror p_line(const struct redraw_context *ctx, const plot_style_t *ps,
		const struct rect *l)
{
	const nsx_sink *s = sink_of(ctx);
	nsx_style st;
	style_from(ps, &st);
	s->line(s->rec, l->x0, l->y0, l->x1, l->y1, &st);
	return NSERROR_OK;
}

static nserror p_rectangle(const struct redraw_context *ctx,
		const plot_style_t *ps, const struct rect *r)
{
	const nsx_sink *s = sink_of(ctx);
	nsx_style st;
	style_from(ps, &st);
	s->rect(s->rec, r->x0, r->y0, r->x1, r->y1, &st);
	return NSERROR_OK;
}

static nserror p_polygon(const struct redraw_context *ctx,
		const plot_style_t *ps, const int *p, unsigned int n)
{
	const nsx_sink *s = sink_of(ctx);
	nsx_style st;
	style_from(ps, &st);
	s->polygon(s->rec, p, n, &st);
	return NSERROR_OK;
}

static nserror p_path(const struct redraw_context *ctx, const plot_style_t *ps,
		const float *p, unsigned int n, const float transform[6])
{
	/* Paths only come from SVG, which this build leaves out. */
	(void)ctx; (void)ps; (void)p; (void)n; (void)transform;
	return NSERROR_OK;
}

struct bitmap {
	uint8_t *pixels;
	int width;
	int height;
	bool opaque;
	uint32_t generation;
};

static nserror p_bitmap(const struct redraw_context *ctx, struct bitmap *b,
		int x, int y, int width, int height, colour bg,
		bitmap_flags_t flags)
{
	const nsx_sink *s = sink_of(ctx);
	(void)bg;
	if (b == NULL || width <= 0 || height <= 0) {
		return NSERROR_OK;
	}
	s->bitmap(s->rec, b, b->generation, b->pixels, b->width, b->height,
			(size_t)b->width * 4, x, y, width, height,
			(flags & BITMAPF_REPEAT_X) != 0,
			(flags & BITMAPF_REPEAT_Y) != 0);
	return NSERROR_OK;
}

static nserror p_text(const struct redraw_context *ctx,
		const plot_font_style_t *fstyle, int x, int y,
		const char *text, size_t length)
{
	const nsx_sink *s = sink_of(ctx);
	nsx_font f;
	nsx_font_from_style(fstyle, &f);
	s->text(s->rec, &f, x, y, text, length, nsx_colour(fstyle->foreground));
	return NSERROR_OK;
}

const struct plotter_table nsx_plotters = {
	.clip = p_clip,
	.arc = p_arc,
	.disc = p_disc,
	.line = p_line,
	.rectangle = p_rectangle,
	.polygon = p_polygon,
	.path = p_path,
	.bitmap = p_bitmap,
	.text = p_text,
	.option_knockout = false,
};

/* ---- bitmaps --------------------------------------------------------- */

void nsx_bitmap_init(void)
{
	bitmap_fmt_t fmt = { .layout = BITMAP_LAYOUT_R8G8B8A8, .pma = false };
	bitmap_set_format(&fmt);
}

static void *b_create(int width, int height, enum gui_bitmap_flags flags)
{
	struct bitmap *b = calloc(1, sizeof(*b));
	if (b == NULL) {
		return NULL;
	}
	b->pixels = calloc((size_t)width * (size_t)height, 4);
	if (b->pixels == NULL) {
		free(b);
		return NULL;
	}
	b->width = width;
	b->height = height;
	b->opaque = (flags & BITMAP_OPAQUE) != 0;
	return b;
}

static void b_destroy(void *bitmap)
{
	struct bitmap *b = bitmap;
	free(b->pixels);
	free(b);
}

static void b_set_opaque(void *bitmap, bool opaque)
{
	((struct bitmap *)bitmap)->opaque = opaque;
}

static bool b_get_opaque(void *bitmap)
{
	return ((struct bitmap *)bitmap)->opaque;
}

static unsigned char *b_get_buffer(void *bitmap)
{
	return ((struct bitmap *)bitmap)->pixels;
}

static size_t b_get_rowstride(void *bitmap)
{
	return (size_t)((struct bitmap *)bitmap)->width * 4;
}

static int b_get_width(void *bitmap)
{
	return ((struct bitmap *)bitmap)->width;
}

static int b_get_height(void *bitmap)
{
	return ((struct bitmap *)bitmap)->height;
}

static void b_modified(void *bitmap)
{
	((struct bitmap *)bitmap)->generation++;
}

static nserror b_render(struct bitmap *bitmap, struct hlcache_handle *content)
{
	/* Thumbnails are not offered. */
	(void)bitmap; (void)content;
	return NSERROR_NOT_IMPLEMENTED;
}

static struct gui_bitmap_table bitmap_table = {
	.create = b_create,
	.destroy = b_destroy,
	.set_opaque = b_set_opaque,
	.get_opaque = b_get_opaque,
	.get_buffer = b_get_buffer,
	.get_rowstride = b_get_rowstride,
	.get_width = b_get_width,
	.get_height = b_get_height,
	.modified = b_modified,
	.render = b_render,
};

struct gui_bitmap_table *nsx_bitmap_table = &bitmap_table;

/* ---- text measuring -------------------------------------------------- */

static nserror l_width(const plot_font_style_t *fstyle, const char *s,
		size_t len, int *width)
{
	nsx_font f;
	nsx_font_from_style(fstyle, &f);
	*width = nsx_host_v->text_width(nsx_host_v->ctx, &f, s, len);
	return NSERROR_OK;
}

static nserror l_position(const plot_font_style_t *fstyle, const char *s,
		size_t len, int x, size_t *offset, int *actual_x)
{
	nsx_font f;
	nsx_font_from_style(fstyle, &f);
	nsx_host_v->text_position(nsx_host_v->ctx, &f, s, len, x, offset,
			actual_x);
	return NSERROR_OK;
}

static nserror l_split(const plot_font_style_t *fstyle, const char *s,
		size_t len, int x, size_t *offset, int *actual_x)
{
	nsx_font f;
	nsx_font_from_style(fstyle, &f);
	nsx_host_v->text_split(nsx_host_v->ctx, &f, s, len, x, offset,
			actual_x);
	return NSERROR_OK;
}

static struct gui_layout_table layout_table = {
	.width = l_width,
	.position = l_position,
	.split = l_split,
};

struct gui_layout_table *nsx_layout_table = &layout_table;
