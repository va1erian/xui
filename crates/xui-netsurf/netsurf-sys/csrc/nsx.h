/*
 * nsx: a small, flat C interface over the NetSurf core for an embedding host.
 *
 * NetSurf talks to a frontend through tables of callbacks that take NetSurf's
 * own structures (plot styles, font styles, lwc strings, bitmaps). This glue
 * implements those tables and forwards each call as plain values (integers,
 * UTF-8 pointers, straight 0xAARRGGBB colours), so the host language binds a
 * dozen functions and never sees a NetSurf header.
 *
 * Threading: the NetSurf core keeps global state. Every nsx_* call, and every
 * callback below, happens on the one thread that called nsx_init.
 */
#ifndef NSX_H
#define NSX_H

#include <stddef.h>
#include <stdint.h>

/* A font as NetSurf asks for it, resolved to plain values. */
typedef struct nsx_font {
	/* The first family the CSS named (not NUL terminated), or NULL. */
	const char *family;
	size_t family_len;
	/* The generic family: 0 sans-serif, 1 serif, 2 monospace, 3 cursive,
	 * 4 fantasy. */
	int generic;
	/* The em size in CSS pixels. */
	float size_px;
	/* 100 to 900. */
	int weight;
	/* Italic or oblique. */
	int italic;
} nsx_font;

/* How a stroke or fill is drawn. */
enum nsx_plot_kind {
	NSX_PLOT_NONE = 0,
	NSX_PLOT_SOLID = 1,
	NSX_PLOT_DOT = 2,
	NSX_PLOT_DASH = 3,
};

/* A plot style; colours are straight 0xAARRGGBB. */
typedef struct nsx_style {
	int fill_kind;
	uint32_t fill;
	int stroke_kind;
	uint32_t stroke;
	float stroke_width;
} nsx_style;

/* Events a window reports (the values of NetSurf's gui_window_event). */
enum nsx_window_event {
	NSX_EVENT_UPDATE_EXTENT = 1,
	NSX_EVENT_START_THROBBER = 3,
	NSX_EVENT_STOP_THROBBER = 4,
	NSX_EVENT_NEW_CONTENT = 6,
};

/* Request methods for nsx_request. */
enum nsx_method {
	NSX_METHOD_GET = 0,
	NSX_METHOD_HEAD = 1,
	NSX_METHOD_POST = 2,
};

/* An http: or https: request NetSurf wants made. Everything it points at
 * lives only for the fetch_start call. */
typedef struct nsx_request {
	/* Names the fetch in the nsx_fetch_* calls that answer it. */
	uint64_t id;
	const char *url;
	int method;
	/* `header_count` "Name: value" strings, NUL terminated. */
	const char *const *headers;
	size_t header_count;
	/* The request body (POST), or NULL. */
	const uint8_t *body;
	size_t body_len;
} nsx_request;

/* What the host provides for the engine's whole life. `ctx` is passed back. */
typedef struct nsx_host {
	void *ctx;
	/* The advance width of `s` in CSS pixels. */
	int (*text_width)(void *ctx, const nsx_font *f, const char *s, size_t len);
	/* The byte offset of the character boundary nearest `x`, and its x. */
	void (*text_position)(void *ctx, const nsx_font *f, const char *s,
			size_t len, int x, size_t *offset, int *actual_x);
	/* Where to break `s` to fit `x`: the last space before `x` overflows,
	 * else the first space after it, else `len`; and the width up to it. */
	void (*text_split)(void *ctx, const nsx_font *f, const char *s,
			size_t len, int x, size_t *offset, int *actual_x);
	/* `win` is the handle given to nsx_window_create. */
	void (*win_invalidate)(void *ctx, void *win);
	void (*win_event)(void *ctx, void *win, int event);
	void (*win_title)(void *ctx, void *win, const char *title);
	void (*win_url)(void *ctx, void *win, const char *url);
	void (*win_size)(void *ctx, void *win, int *width, int *height);
	void (*win_pointer)(void *ctx, void *win, int shape);
	/* Built-in resources by name ("default.css"...): 1 and the bytes, which
	 * must live as long as the engine, or 0. */
	int (*resource)(void *ctx, const char *path, const uint8_t **data,
			size_t *len);
	/* Starts an http(s) request once nsx_fetch_register was called. The
	 * host answers later, on this thread, through the nsx_fetch_* calls,
	 * and never follows redirects itself. */
	void (*fetch_start)(void *ctx, const nsx_request *request);
	/* NetSurf no longer wants fetch `id`: the host stops answering it. */
	void (*fetch_abort)(void *ctx, uint64_t id);
	/* PNG and JPEG images. image_size reads the size from the header: 1
	 * and the size, or 0 for data the host cannot or will not decode. */
	int (*image_size)(void *ctx, const uint8_t *data, size_t len,
			int *width, int *height);
	/* Decodes into `pixels` (width * height * 4 bytes, rows of RGBA with
	 * straight alpha); 1 on success, setting *opaque when every pixel's
	 * alpha is 255. */
	int (*image_decode)(void *ctx, const uint8_t *data, size_t len,
			uint8_t *pixels, int width, int height, int *opaque);
	/* The status line text NetSurf wants shown for `win` (the link under
	 * the pointer, the load's progress). */
	void (*win_status)(void *ctx, void *win, const char *text);
	/* A URL NetSurf has no fetcher for (mailto:, say), for the host to
	 * hand to the system. `win` is the window it came from, or NULL. */
	void (*launch_url)(void *ctx, void *win, const char *url);
	/* A download starts in `win` (or NULL): download `id` of `url`, with
	 * NetSurf's file name (from Content-Disposition or the URL), its MIME
	 * type and its total size (0 when unknown). 1 to accept it, 0 to
	 * refuse. */
	int (*dl_start)(void *ctx, void *win, uint64_t id, const char *url,
			const char *filename, const char *mime,
			unsigned long long total);
	/* The next bytes of download `id`; 1 to go on, 0 to stop it. */
	int (*dl_data)(void *ctx, uint64_t id, const uint8_t *data,
			size_t len);
	/* Download `id` ended: `error` is NULL when it completed. */
	void (*dl_end)(void *ctx, uint64_t id, const char *error);
} nsx_host;

/* Where one redraw's drawing goes. Coordinates are CSS pixels. */
typedef struct nsx_sink {
	void *rec;
	/* Replaces the clip (NetSurf's clips are absolute, not nested). */
	void (*clip)(void *rec, int x0, int y0, int x1, int y1);
	void (*rect)(void *rec, int x0, int y0, int x1, int y1,
			const nsx_style *s);
	void (*line)(void *rec, int x0, int y0, int x1, int y1,
			const nsx_style *s);
	void (*disc)(void *rec, int x, int y, int radius, const nsx_style *s);
	/* `p` holds `n` x,y pairs. */
	void (*polygon)(void *rec, const int *p, unsigned n,
			const nsx_style *s);
	/* `id` and `generation` identify the pixels (generation changes when
	 * NetSurf rewrites them); RGBA rows of `stride` bytes. */
	void (*bitmap)(void *rec, const void *id, uint32_t generation,
			const uint8_t *rgba, int bw, int bh, size_t stride,
			int x, int y, int w, int h, int repeat_x, int repeat_y);
	/* `y` is the baseline. */
	void (*text)(void *rec, const nsx_font *f, int x, int y,
			const char *s, size_t len, uint32_t colour);
} nsx_sink;

/* Mouse actions for nsx_window_mouse. */
enum nsx_mouse {
	NSX_MOUSE_MOVE = 0,
	NSX_MOUSE_PRESS = 1,
	NSX_MOUSE_CLICK = 2,
	NSX_MOUSE_RELEASE = 3,
};

struct gui_window;

/* Starts NetSurf. `messages` is a plain Messages file; 0 on success. */
int nsx_init(const nsx_host *host, const uint8_t *messages, size_t len);
/* Runs what is due; returns ms until the next timer, or -1 for none. */
int nsx_poll(void);
void nsx_fini(void);

/* Opens `url` in a new browser window reporting through `win`; NULL on
 * failure. */
struct gui_window *nsx_window_create(void *win, const char *url);
int nsx_window_navigate(struct gui_window *gw, const char *url);
void nsx_window_destroy(struct gui_window *gw);
/* Lays the page out again after the host's size changed. */
void nsx_window_reformat(struct gui_window *gw);
/* The laid-out document size; 0 when there is no content yet. */
int nsx_window_extent(struct gui_window *gw, int *width, int *height);
/* Whether the content can be drawn. */
int nsx_window_ready(struct gui_window *gw);
/* Draws the document area `x0,y0 - x1,y1` (document coordinates) into
 * `sink`; 0 on success. */
int nsx_window_redraw(struct gui_window *gw, int x0, int y0, int x1, int y1,
		const nsx_sink *sink);
/* A pointer action at a document point. */
void nsx_window_mouse(struct gui_window *gw, int action, int x, int y);
/* A typed character (UCS-4) or NetSurf key code; 1 if it was used. */
int nsx_window_key(struct gui_window *gw, uint32_t key);
/* Stops the window's load. */
void nsx_window_stop(struct gui_window *gw);
/* Downloads `url` (instead of showing it) for the window; 0 on success. */
int nsx_window_download(struct gui_window *gw, const char *url);

/* Stops download `id`; its dl_end reports "Cancelled". */
void nsx_download_cancel(uint64_t id);

/* Hands http: and https: URLs to the host's fetch_start from now on; 0 on
 * success. */
int nsx_fetch_register(void);
/* The host's answer to request `id`, on the nsx_init thread: the status
 * first, then the headers, the body in pieces, and finish or fail last. An
 * id NetSurf has aborted or that has ended is ignored. */
void nsx_fetch_status(uint64_t id, int code);
void nsx_fetch_header(uint64_t id, const char *name, size_t name_len,
		const char *value, size_t value_len);
void nsx_fetch_data(uint64_t id, const uint8_t *data, size_t len);
void nsx_fetch_finish(uint64_t id);
void nsx_fetch_fail(uint64_t id, const char *message);

#endif
