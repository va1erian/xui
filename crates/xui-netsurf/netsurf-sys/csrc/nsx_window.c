/*
 * nsx browser windows: NetSurf's window table (reported to the host) and the
 * calls the host makes into a browser window.
 */
#include <stdbool.h>
#include <stdlib.h>

#include "utils/errors.h"
#include "utils/nsurl.h"
#include "netsurf/browser_window.h"
#include "netsurf/keypress.h"
#include "netsurf/mouse.h"
#include "netsurf/plotters.h"
#include "netsurf/types.h"

#include "nsx_internal.h"

struct gui_window {
	struct browser_window *bw;
	void *win;
	int scroll_x;
	int scroll_y;
};

/* The host handle for the window nsx_window_create is creating: NetSurf calls
 * the table's create from inside browser_window_create. */
static void *pending_win;
/* The window that create made, handed back to nsx_window_create. */
static struct gui_window *created_gw;

static struct gui_window *gw_create(struct browser_window *bw,
		struct gui_window *existing, gui_window_create_flags flags)
{
	struct gui_window *gw = calloc(1, sizeof(*gw));

	(void)existing;
	(void)flags;
	if (gw == NULL) {
		return NULL;
	}
	gw->bw = bw;
	gw->win = pending_win;
	created_gw = gw;
	return gw;
}

static void gw_destroy(struct gui_window *gw)
{
	free(gw);
}

static nserror gw_invalidate(struct gui_window *gw, const struct rect *rect)
{
	(void)rect;
	if (gw->win != NULL) {
		nsx_host_v->win_invalidate(nsx_host_v->ctx, gw->win);
	}
	return NSERROR_OK;
}

static bool gw_get_scroll(struct gui_window *gw, int *sx, int *sy)
{
	*sx = gw->scroll_x;
	*sy = gw->scroll_y;
	return true;
}

static nserror gw_set_scroll(struct gui_window *gw, const struct rect *rect)
{
	gw->scroll_x = rect->x0;
	gw->scroll_y = rect->y0;
	return NSERROR_OK;
}

static nserror gw_get_dimensions(struct gui_window *gw, int *w, int *h)
{
	*w = 800;
	*h = 600;
	if (gw->win != NULL) {
		nsx_host_v->win_size(nsx_host_v->ctx, gw->win, w, h);
	}
	return NSERROR_OK;
}

static nserror gw_event(struct gui_window *gw, enum gui_window_event event)
{
	if (gw->win != NULL) {
		nsx_host_v->win_event(nsx_host_v->ctx, gw->win, (int)event);
	}
	return NSERROR_OK;
}

static void gw_set_title(struct gui_window *gw, const char *title)
{
	if (gw->win != NULL) {
		nsx_host_v->win_title(nsx_host_v->ctx, gw->win, title);
	}
}

static nserror gw_set_url(struct gui_window *gw, struct nsurl *url)
{
	if (gw->win != NULL) {
		nsx_host_v->win_url(nsx_host_v->ctx, gw->win, nsurl_access(url));
	}
	return NSERROR_OK;
}

static void gw_set_pointer(struct gui_window *gw, enum gui_pointer_shape shape)
{
	if (gw->win != NULL) {
		nsx_host_v->win_pointer(nsx_host_v->ctx, gw->win, (int)shape);
	}
}

static struct gui_window_table window_table = {
	.create = gw_create,
	.destroy = gw_destroy,
	.invalidate = gw_invalidate,
	.get_scroll = gw_get_scroll,
	.set_scroll = gw_set_scroll,
	.get_dimensions = gw_get_dimensions,
	.event = gw_event,
	.set_title = gw_set_title,
	.set_url = gw_set_url,
	.set_pointer = gw_set_pointer,
};

struct gui_window_table *nsx_window_table = &window_table;

/* ---- host calls ------------------------------------------------------ */

struct gui_window *nsx_window_create(void *win, const char *url)
{
	struct browser_window *bw = NULL;
	nsurl *nurl;

	if (nsurl_create(url, &nurl) != NSERROR_OK) {
		return NULL;
	}
	pending_win = win;
	created_gw = NULL;
	nserror err = browser_window_create(BW_CREATE_HISTORY, nurl, NULL,
			NULL, &bw);
	pending_win = NULL;
	nsurl_unref(nurl);
	if (err != NSERROR_OK || bw == NULL) {
		return NULL;
	}
	return created_gw;
}

int nsx_window_navigate(struct gui_window *gw, const char *url)
{
	nsurl *nurl;
	nserror err;

	if (nsurl_create(url, &nurl) != NSERROR_OK) {
		return -1;
	}
	err = browser_window_navigate(gw->bw, nurl, NULL, BW_NAVIGATE_HISTORY,
			NULL, NULL, NULL);
	nsurl_unref(nurl);
	return err == NSERROR_OK ? 0 : -1;
}

void nsx_window_destroy(struct gui_window *gw)
{
	/* The host stops listening first: NetSurf may still report while the
	 * window is torn down. */
	gw->win = NULL;
	browser_window_destroy(gw->bw);
}

void nsx_window_reformat(struct gui_window *gw)
{
	browser_window_schedule_reformat(gw->bw);
}

int nsx_window_extent(struct gui_window *gw, int *width, int *height)
{
	*width = 0;
	*height = 0;
	if (!browser_window_has_content(gw->bw)) {
		return 0;
	}
	return browser_window_get_extents(gw->bw, false, width, height) ==
			NSERROR_OK;
}

int nsx_window_ready(struct gui_window *gw)
{
	return browser_window_redraw_ready(gw->bw);
}

int nsx_window_redraw(struct gui_window *gw, int x0, int y0, int x1, int y1,
		const nsx_sink *sink)
{
	struct rect clip = { .x0 = x0, .y0 = y0, .x1 = x1, .y1 = y1 };
	struct redraw_context ctx = {
		.interactive = true,
		.background_images = true,
		.plot = &nsx_plotters,
		.priv = (void *)sink,
	};

	return browser_window_redraw(gw->bw, 0, 0, &clip, &ctx) ? 0 : -1;
}

void nsx_window_mouse(struct gui_window *gw, int action, int x, int y)
{
	switch (action) {
	case NSX_MOUSE_PRESS:
		browser_window_mouse_click(gw->bw, BROWSER_MOUSE_PRESS_1, x, y);
		break;
	case NSX_MOUSE_CLICK:
		browser_window_mouse_click(gw->bw, BROWSER_MOUSE_CLICK_1, x, y);
		break;
	case NSX_MOUSE_RELEASE:
		browser_window_mouse_track(gw->bw, BROWSER_MOUSE_HOVER, x, y);
		break;
	default:
		browser_window_mouse_track(gw->bw, BROWSER_MOUSE_HOVER, x, y);
		break;
	}
}

int nsx_window_key(struct gui_window *gw, uint32_t key)
{
	return browser_window_key_press(gw->bw, key) ? 1 : 0;
}
