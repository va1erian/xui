/*
 * nsx engine lifetime: registering the frontend tables, the timer queue
 * NetSurf schedules work on, and the fetch table (file types and the built-in
 * resources the host embeds).
 */
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/time.h>

#include "utils/errors.h"
#include "utils/messages.h"
#include "utils/nsoption.h"
#include "netsurf/browser.h"
#include "netsurf/fetch.h"
#include "netsurf/misc.h"
#include "netsurf/netsurf.h"

#include "nsx.h"
#include "nsx_internal.h"

const nsx_host *nsx_host_v;

/* ---- the timer queue -------------------------------------------------- */

struct nsx_timer {
	struct nsx_timer *next;
	struct timeval when;
	void (*callback)(void *p);
	void *p;
};

static struct nsx_timer *timers;

static bool timer_remove(void (*callback)(void *p), void *p)
{
	struct nsx_timer **link = &timers;
	bool removed = false;

	while (*link != NULL) {
		struct nsx_timer *t = *link;
		if (t->callback == callback && t->p == p) {
			*link = t->next;
			free(t);
			removed = true;
		} else {
			link = &t->next;
		}
	}
	return removed;
}

/* NetSurf's schedule(): a negative time cancels; a callback and context pair
 * is scheduled at most once. */
static nserror nsx_schedule(int ms, void (*callback)(void *p), void *p)
{
	struct nsx_timer *t;
	struct timeval delay;
	bool removed = timer_remove(callback, p);

	if (ms < 0) {
		return removed ? NSERROR_OK : NSERROR_NOT_FOUND;
	}
	t = calloc(1, sizeof(*t));
	if (t == NULL) {
		return NSERROR_NOMEM;
	}
	gettimeofday(&t->when, NULL);
	delay.tv_sec = ms / 1000;
	delay.tv_usec = (ms % 1000) * 1000;
	timeradd(&t->when, &delay, &t->when);
	t->callback = callback;
	t->p = p;
	t->next = timers;
	timers = t;
	return NSERROR_OK;
}

int nsx_poll(void)
{
	struct timeval now, next, left;

	for (;;) {
		struct nsx_timer **link = &timers;
		struct nsx_timer *due = NULL;

		gettimeofday(&now, NULL);
		while (*link != NULL) {
			if (!timercmp(&(*link)->when, &now, >)) {
				due = *link;
				*link = due->next;
				break;
			}
			link = &(*link)->next;
		}
		if (due == NULL) {
			break;
		}
		/* The callback may schedule or cancel others, so the walk
		 * starts over after each one. */
		due->callback(due->p);
		free(due);
	}
	if (timers == NULL) {
		return -1;
	}
	next = timers->when;
	for (struct nsx_timer *t = timers->next; t != NULL; t = t->next) {
		if (timercmp(&t->when, &next, <)) {
			next = t->when;
		}
	}
	gettimeofday(&now, NULL);
	if (!timercmp(&next, &now, >)) {
		return 0;
	}
	timersub(&next, &now, &left);
	return (int)(left.tv_sec * 1000 + left.tv_usec / 1000);
}

/* ---- misc and fetch tables ------------------------------------------- */

static nserror nsx_launch_url(struct nsurl *url)
{
	(void)url;
	return NSERROR_NOT_IMPLEMENTED;
}

static struct gui_misc_table misc_table = {
	.schedule = nsx_schedule,
	.launch_url = nsx_launch_url,
};

/* MIME types of local files, by extension. */
static const char *nsx_filetype(const char *path)
{
	static const struct {
		const char *ext;
		const char *mime;
	} types[] = {
		{ "html", "text/html" }, { "htm", "text/html" },
		{ "css", "text/css" }, { "txt", "text/plain" },
		{ "gif", "image/gif" }, { "bmp", "image/bmp" },
		{ "ico", "image/x-icon" }, { "png", "image/png" },
		{ "jpg", "image/jpeg" }, { "jpeg", "image/jpeg" },
		{ "svg", "image/svg+xml" },
	};
	const char *dot = strrchr(path, '.');

	if (dot != NULL) {
		for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
			if (strcasecmp(dot + 1, types[i].ext) == 0) {
				return types[i].mime;
			}
		}
	}
	return "text/plain";
}

static nserror nsx_resource_data(const char *path, const uint8_t **data,
		size_t *len)
{
	if (nsx_host_v->resource(nsx_host_v->ctx, path, data, len)) {
		return NSERROR_OK;
	}
	return NSERROR_NOT_FOUND;
}

static struct nsurl *nsx_resource_url(const char *path)
{
	(void)path;
	return NULL;
}

static struct gui_fetch_table fetch_table = {
	.filetype = nsx_filetype,
	.get_resource_url = nsx_resource_url,
	.get_resource_data = nsx_resource_data,
};

/* ---- lifetime -------------------------------------------------------- */

static nserror nsx_option_defaults(struct nsoption_s *defaults)
{
	(void)defaults;
	return NSERROR_OK;
}

int nsx_init(const nsx_host *host, const uint8_t *messages, size_t len)
{
	/* NetSurf keeps a pointer to this table, so it must outlive the call. */
	static struct netsurf_table table;

	table.misc = &misc_table;
	table.window = nsx_window_table;
	table.fetch = &fetch_table;
	table.bitmap = nsx_bitmap_table;
	table.layout = nsx_layout_table;
	nsx_host_v = host;
	if (netsurf_register(&table) != NSERROR_OK) {
		return -1;
	}
	if (nsoption_init(nsx_option_defaults, &nsoptions,
			&nsoptions_default) != NSERROR_OK) {
		return -1;
	}
	/* No scripting, no disc cache, no hover-prefetch: a rendering
	 * comparison wants the page as written. */
	nsoption_set_bool(enable_javascript, false);
	nsoption_set_uint(disc_cache_size, 0);
	/* 12pt, the 16px every other browser defaults to (NetSurf's own
	 * default is 12.8pt). */
	nsoption_set_int(font_size, 120);
	if (messages != NULL) {
		messages_add_from_inline(messages, len);
	}
	browser_set_dpi(96);
	nsx_bitmap_init();
	return netsurf_init(NULL) == NSERROR_OK ? 0 : -1;
}

void nsx_fini(void)
{
	netsurf_exit();
	nsoption_finalise(nsoptions, nsoptions_default);
	while (timers != NULL) {
		struct nsx_timer *t = timers;
		timers = t->next;
		free(t);
	}
}
