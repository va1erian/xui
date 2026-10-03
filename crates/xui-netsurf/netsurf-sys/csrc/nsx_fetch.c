/*
 * nsx host fetcher: NetSurf's fetcher for http: and https: URLs, handing each
 * request to the host's fetch_start and turning the host's answers (the
 * nsx_fetch_* calls) into fetch messages, as NetSurf's curl fetcher does with
 * libcurl's callbacks.
 *
 * The host answers on the engine thread, outside any NetSurf call, so a
 * message is never sent re-entrantly. A fetch ends here: after its last
 * message (finished, error, redirect, not modified) it is removed from
 * NetSurf's queues and freed. An aborted fetch is only flagged, because
 * NetSurf aborts from inside the callbacks; the next poll frees it.
 */
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "utils/corestrings.h"
#include "utils/messages.h"
#include "utils/nsoption.h"
#include "utils/nsurl.h"
#include "utils/useragent.h"
#include "content/fetch.h"
#include "content/fetchers.h"
#include "content/urldb.h"

#include "nsx_internal.h"

/* What NetSurf will take: the formats it renders, then anything. */
#define NSX_ACCEPT "Accept: text/html,application/xhtml+xml," \
	"application/xml;q=0.9,image/png,image/jpeg,image/gif,image/bmp," \
	"*/*;q=0.8"

struct nsx_fetch {
	struct nsx_fetch *next;
	struct fetch *parent;
	nsurl *url;
	uint64_t id;
	int method;
	bool only_2xx;
	/* Handed to the host, so an abort must be passed on. */
	bool started;
	bool aborted;
	/* Inside a NetSurf callback: poll must not free it. */
	bool locked;
	/* The status and headers were acted on (the first data or the end). */
	bool headers_done;
	long http_code;
	char *location;
	char **headers;
	size_t header_count;
	uint8_t *body;
	size_t body_len;
};

static struct nsx_fetch *fetches;
static uint64_t next_id;

static struct nsx_fetch *find(uint64_t id)
{
	for (struct nsx_fetch *f = fetches; f != NULL; f = f->next) {
		if (f->id == id) {
			return f->aborted ? NULL : f;
		}
	}
	return NULL;
}

/* Appends a printf-formatted request header; false when out of memory. */
static bool add_header(struct nsx_fetch *f, const char *fmt, ...)
{
	va_list ap;
	int len;
	char *line;
	char **grown;

	va_start(ap, fmt);
	len = vsnprintf(NULL, 0, fmt, ap);
	va_end(ap);
	if (len < 0) {
		return false;
	}
	line = malloc((size_t)len + 1);
	if (line == NULL) {
		return false;
	}
	va_start(ap, fmt);
	vsnprintf(line, (size_t)len + 1, fmt, ap);
	va_end(ap);
	grown = realloc(f->headers, (f->header_count + 1) * sizeof(char *));
	if (grown == NULL) {
		free(line);
		return false;
	}
	f->headers = grown;
	f->headers[f->header_count++] = line;
	return true;
}

static void fetch_destroy(struct nsx_fetch *f)
{
	for (size_t i = 0; i < f->header_count; i++) {
		free(f->headers[i]);
	}
	free(f->headers);
	free(f->body);
	free(f->location);
	if (f->url != NULL) {
		nsurl_unref(f->url);
	}
	free(f);
}

/* The body and its Content-Type for a form; true when there is none. */
static bool setup_post(struct nsx_fetch *f, const char *urlenc,
		const struct fetch_multipart_data *multipart)
{
	if (urlenc != NULL) {
		f->method = NSX_METHOD_POST;
		f->body_len = strlen(urlenc);
		f->body = malloc(f->body_len + 1);
		if (f->body == NULL) {
			return false;
		}
		memcpy(f->body, urlenc, f->body_len + 1);
		return add_header(f, "Content-Type: "
				"application/x-www-form-urlencoded");
	}
	if (multipart != NULL) {
		char *content_type;
		f->method = NSX_METHOD_POST;
		if (!nsx_post_multipart(multipart, &f->body, &f->body_len,
				&content_type)) {
			return false;
		}
		bool ok = add_header(f, "%s", content_type);
		free(content_type);
		return ok;
	}
	return true;
}

static bool nsx_fetch_initialise(lwc_string *scheme)
{
	(void)scheme;
	return true;
}

static bool nsx_fetch_acceptable(const nsurl *url)
{
	(void)url;
	return true;
}

static void nsx_fetch_finalise(lwc_string *scheme)
{
	(void)scheme;
}

static void *nsx_fetch_setup(struct fetch *parent, nsurl *url, bool only_2xx,
		bool downgrade_tls, const char *post_urlenc,
		const struct fetch_multipart_data *post_multipart,
		const char **headers)
{
	struct nsx_fetch *f = calloc(1, sizeof(*f));
	const char *lang = nsoption_charp(accept_language);
	char *cookie;
	bool ok;

	(void)downgrade_tls;
	if (f == NULL) {
		return NULL;
	}
	f->parent = parent;
	f->url = nsurl_ref(url);
	f->only_2xx = only_2xx;
	f->method = NSX_METHOD_GET;
	ok = add_header(f, "User-Agent: %s", user_agent_string()) &&
		add_header(f, "%s", NSX_ACCEPT) &&
		add_header(f, "Accept-Language: %s, *;q=0.1",
				(lang != NULL && lang[0] != '\0') ? lang : "en");
	if (ok && nsoption_bool(do_not_track)) {
		ok = add_header(f, "DNT: 1");
	}
	cookie = urldb_get_cookie(url, true);
	if (ok && cookie != NULL) {
		ok = add_header(f, "Cookie: %s", cookie);
	}
	free(cookie);
	for (size_t i = 0; ok && headers[i] != NULL; i++) {
		ok = add_header(f, "%s", headers[i]);
	}
	if (!ok || !setup_post(f, post_urlenc, post_multipart)) {
		fetch_destroy(f);
		return NULL;
	}
	f->id = ++next_id;
	f->next = fetches;
	fetches = f;
	return f;
}

static bool nsx_fetch_start(void *handle)
{
	struct nsx_fetch *f = handle;
	nsx_request request = {
		.id = f->id,
		.url = nsurl_access(f->url),
		.method = f->method,
		.headers = (const char *const *)f->headers,
		.header_count = f->header_count,
		.body = f->body,
		.body_len = f->body_len,
	};

	f->started = true;
	nsx_host_v->fetch_start(nsx_host_v->ctx, &request);
	return true;
}

static void nsx_fetch_abort(void *handle)
{
	struct nsx_fetch *f = handle;

	if (f->aborted) {
		return;
	}
	f->aborted = true;
	if (f->started) {
		nsx_host_v->fetch_abort(nsx_host_v->ctx, f->id);
	}
}

static void nsx_fetch_free(void *handle)
{
	struct nsx_fetch *f = handle;

	for (struct nsx_fetch **link = &fetches; *link != NULL;
			link = &(*link)->next) {
		if (*link == f) {
			*link = f->next;
			break;
		}
	}
	fetch_destroy(f);
}

/* Takes the fetch out of NetSurf's queues and frees it (through free). */
static void end(struct nsx_fetch *f)
{
	struct fetch *parent = f->parent;
	fetch_remove_from_queues(parent);
	fetch_free(parent);
}

/* Frees the fetches NetSurf aborted. */
static void nsx_fetch_poll(lwc_string *scheme)
{
	struct nsx_fetch *f;

	(void)scheme;
	do {
		for (f = fetches; f != NULL; f = f->next) {
			if (f->aborted && !f->locked) {
				break;
			}
		}
		if (f != NULL) {
			end(f);
		}
	} while (f != NULL);
}

static void send_msg(struct nsx_fetch *f, fetch_msg *msg)
{
	f->locked = true;
	fetch_send_callback(msg, f->parent);
	f->locked = false;
}

/* Acts on the status once the headers are in, as curl's fetcher does: a
 * redirect, a not-modified or an unwanted error ends the fetch here. True if
 * it ended. */
static bool process_headers(struct nsx_fetch *f)
{
	fetch_msg msg;

	f->headers_done = true;
	if (f->http_code == 0) {
		f->http_code = 200;
		fetch_set_http_code(f->parent, 200);
	}
	if (f->http_code == 304 && f->method != NSX_METHOD_POST) {
		msg.type = FETCH_NOTMODIFIED;
	} else if (f->http_code >= 300 && f->http_code < 400 &&
			f->location != NULL) {
		msg.type = FETCH_REDIRECT;
		msg.data.redirect = f->location;
	} else if (f->only_2xx &&
			(f->http_code < 200 || f->http_code >= 300)) {
		msg.type = FETCH_ERROR;
		msg.data.error = messages_get("Not2xx");
	} else {
		return false;
	}
	/* NetSurf aborts a redirected fetch itself; make sure the host stops
	 * whatever it still sends. */
	send_msg(f, &msg);
	nsx_fetch_abort(f);
	end(f);
	return true;
}

/* ---- host calls ------------------------------------------------------ */

int nsx_fetch_register(void)
{
	const struct fetcher_operation_table ops = {
		.initialise = nsx_fetch_initialise,
		.acceptable = nsx_fetch_acceptable,
		.setup = nsx_fetch_setup,
		.start = nsx_fetch_start,
		.abort = nsx_fetch_abort,
		.free = nsx_fetch_free,
		.poll = nsx_fetch_poll,
		.finalise = nsx_fetch_finalise,
	};

	if (fetcher_add(lwc_string_ref(corestring_lwc_http), &ops) !=
			NSERROR_OK) {
		return -1;
	}
	return fetcher_add(lwc_string_ref(corestring_lwc_https), &ops) ==
			NSERROR_OK ? 0 : -1;
}

void nsx_fetch_status(uint64_t id, int code)
{
	struct nsx_fetch *f = find(id);

	if (f == NULL || f->headers_done || code < 100 || code > 999) {
		return;
	}
	f->http_code = code;
	fetch_set_http_code(f->parent, code);
}

void nsx_fetch_header(uint64_t id, const char *name, size_t name_len,
		const char *value, size_t value_len)
{
	struct nsx_fetch *f = find(id);
	fetch_msg msg;
	char *line;

	if (f == NULL || f->headers_done || name_len == 0 ||
			memchr(name, ':', name_len) != NULL ||
			name_len > SIZE_MAX / 2 || value_len > SIZE_MAX / 2) {
		return;
	}
	/* NetSurf's header parser reads a NUL-terminated "Name: value". */
	line = malloc(name_len + value_len + 3);
	if (line == NULL) {
		return;
	}
	memcpy(line, name, name_len);
	memcpy(line + name_len, ": ", 2);
	memcpy(line + name_len + 2, value, value_len);
	line[name_len + 2 + value_len] = '\0';
	if (strlen(line) != name_len + 2 + value_len) {
		/* An embedded NUL: not a header NetSurf can read. */
		free(line);
		return;
	}
	if (name_len == 8 && strncasecmp(name, "Location", 8) == 0) {
		free(f->location);
		f->location = strdup(line + 10);
	} else if (name_len == 10 && strncasecmp(name, "Set-Cookie", 10) == 0) {
		fetch_set_cookie(f->parent, line + 12);
	}
	msg.type = FETCH_HEADER;
	msg.data.header_or_data.buf = (const uint8_t *)line;
	msg.data.header_or_data.len = strlen(line);
	send_msg(f, &msg);
	free(line);
}

void nsx_fetch_data(uint64_t id, const uint8_t *data, size_t len)
{
	struct nsx_fetch *f = find(id);
	fetch_msg msg;

	if (f == NULL || (!f->headers_done && process_headers(f)) || len == 0) {
		return;
	}
	msg.type = FETCH_DATA;
	msg.data.header_or_data.buf = data;
	msg.data.header_or_data.len = len;
	send_msg(f, &msg);
}

void nsx_fetch_finish(uint64_t id)
{
	struct nsx_fetch *f = find(id);
	fetch_msg msg;

	if (f == NULL || (!f->headers_done && process_headers(f))) {
		return;
	}
	msg.type = FETCH_FINISHED;
	send_msg(f, &msg);
	end(f);
}

void nsx_fetch_fail(uint64_t id, const char *message)
{
	struct nsx_fetch *f = find(id);
	fetch_msg msg;

	if (f == NULL) {
		return;
	}
	msg.type = FETCH_ERROR;
	msg.data.error = message;
	send_msg(f, &msg);
	end(f);
}
