/*
 * nsx downloads: NetSurf's download table, reported to the host as plain
 * values. NetSurf hands a page it cannot show (an archive, a PDF, an
 * attachment) or an explicit download to a download context; each one is
 * named to the host by an id, and its bytes go to the host's dl_data.
 */
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#include "utils/errors.h"
#include "utils/nsurl.h"
#include "desktop/download.h"
#include "desktop/gui_internal.h"
#include "netsurf/download.h"
#include "netsurf/misc.h"

#include "nsx_internal.h"

struct gui_download_window {
	struct gui_download_window *next;
	struct download_context *ctx;
	uint64_t id;
	/* Why the download ends before NetSurf says so (the host stopped it,
	 * or it was cancelled); NULL while it runs. */
	const char *stopped;
};

static struct gui_download_window *downloads;
static uint64_t next_id;

static void unlink_download(struct gui_download_window *dw)
{
	for (struct gui_download_window **link = &downloads; *link != NULL;
			link = &(*link)->next) {
		if (*link == dw) {
			*link = dw->next;
			return;
		}
	}
}

/* Reports the end, frees the context (NetSurf leaves that to the frontend)
 * and the window. */
static void finish(struct gui_download_window *dw, const char *error)
{
	unlink_download(dw);
	nsx_host_v->dl_end(nsx_host_v->ctx, dw->id, error);
	download_context_destroy(dw->ctx);
	free(dw);
}

/* A download stopped from inside its own data callback ends here, on the
 * next timer round, once NetSurf is done with its handle. */
static void reap(void *p)
{
	struct gui_download_window *dw = p;

	finish(dw, dw->stopped);
}

static struct gui_download_window *dl_create(struct download_context *ctx,
		struct gui_window *parent)
{
	struct gui_download_window *dw = calloc(1, sizeof(*dw));
	nsurl *url = download_context_get_url(ctx);

	if (dw == NULL) {
		return NULL;
	}
	dw->ctx = ctx;
	dw->id = ++next_id;
	if (!nsx_host_v->dl_start(nsx_host_v->ctx, nsx_window_host(parent),
			dw->id, url != NULL ? nsurl_access(url) : "",
			download_context_get_filename(ctx),
			download_context_get_mime_type(ctx),
			download_context_get_total_length(ctx))) {
		free(dw);
		return NULL;
	}
	dw->next = downloads;
	downloads = dw;
	return dw;
}

static nserror dl_data(struct gui_download_window *dw, const char *data,
		unsigned int size)
{
	if (dw->stopped != NULL) {
		return NSERROR_SAVE_FAILED;
	}
	if (!nsx_host_v->dl_data(nsx_host_v->ctx, dw->id,
			(const uint8_t *)data, size)) {
		/* NetSurf aborts the fetch when this returns; the context is
		 * freed after that. */
		dw->stopped = "The file could not be written";
		guit->misc->schedule(0, reap, dw);
		return NSERROR_SAVE_FAILED;
	}
	return NSERROR_OK;
}

static void dl_error(struct gui_download_window *dw, const char *error_msg)
{
	if (dw->stopped == NULL) {
		finish(dw, error_msg != NULL ? error_msg : "Download failed");
	}
}

static void dl_done(struct gui_download_window *dw)
{
	if (dw->stopped == NULL) {
		finish(dw, NULL);
	}
}

static struct gui_download_table download_table = {
	.create = dl_create,
	.data = dl_data,
	.error = dl_error,
	.done = dl_done,
};

struct gui_download_table *nsx_download_table = &download_table;

void nsx_download_cancel(uint64_t id)
{
	for (struct gui_download_window *dw = downloads; dw != NULL;
			dw = dw->next) {
		if (dw->id == id && dw->stopped == NULL) {
			/* Marked first: the abort may call dl_error, which must
			 * not finish (and free) the download under us. */
			dw->stopped = "Cancelled";
			download_context_abort(dw->ctx);
			finish(dw, dw->stopped);
			return;
		}
	}
}
