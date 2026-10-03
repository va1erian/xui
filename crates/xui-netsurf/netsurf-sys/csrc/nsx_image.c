/*
 * nsx images: content handlers for PNG and JPEG whose decoding the host does
 * (NetSurf's own handlers need libpng and libjpeg). The shape is NetSurf's
 * jpeg handler: the header gives the size when the data is complete, and the
 * image cache asks for the pixels when the image is first drawn, and may drop
 * them again under memory pressure.
 */
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

#include "utils/messages.h"
#include "utils/nsurl.h"
#include "utils/utils.h"
#include "netsurf/bitmap.h"
#include "netsurf/content.h"
#include "content/content_protected.h"
#include "content/content_factory.h"
#include "content/llcache.h"
#include "desktop/gui_internal.h"

#include "image/image_cache.h"

#include "nsx_internal.h"

static nserror nsx_image_create(const content_handler *handler,
		lwc_string *imime_type, const struct http_parameter *params,
		llcache_handle *llcache, const char *fallback_charset,
		bool quirks, struct content **c)
{
	struct content *image = calloc(1, sizeof(struct content));
	nserror error;

	if (image == NULL) {
		return NSERROR_NOMEM;
	}
	error = content__init(image, handler, imime_type, params, llcache,
			fallback_charset, quirks);
	if (error != NSERROR_OK) {
		free(image);
		return error;
	}
	*c = image;
	return NSERROR_OK;
}

/* The image cache's convert: decodes the source into a new bitmap. */
static struct bitmap *nsx_image_decode(struct content *c)
{
	const uint8_t *data;
	size_t size;
	int opaque = 0;
	struct bitmap *bitmap;
	uint8_t *pixels;

	data = content__get_source_data(c, &size);
	bitmap = guit->bitmap->create(c->width, c->height, BITMAP_NONE);
	if (bitmap == NULL) {
		return NULL;
	}
	pixels = guit->bitmap->get_buffer(bitmap);
	/* The host writes tightly packed rows: the nsx bitmap's own layout
	 * (R8G8B8A8, straight alpha, width * 4 bytes a row). */
	if (pixels == NULL ||
	    guit->bitmap->get_rowstride(bitmap) != (size_t)c->width * 4 ||
	    !nsx_host_v->image_decode(nsx_host_v->ctx, data, size, pixels,
			c->width, c->height, &opaque)) {
		guit->bitmap->destroy(bitmap);
		return NULL;
	}
	guit->bitmap->set_opaque(bitmap, opaque != 0);
	guit->bitmap->modified(bitmap);
	return bitmap;
}

static bool nsx_image_convert(struct content *c)
{
	const uint8_t *data;
	size_t size;
	int width = 0, height = 0;
	char *title;
	const char *kind;

	data = content__get_source_data(c, &size);
	if (!nsx_host_v->image_size(nsx_host_v->ctx, data, size, &width,
			&height) || width <= 0 || height <= 0) {
		content_broadcast_error(c, NSERROR_UNKNOWN, NULL);
		return false;
	}
	c->width = width;
	c->height = height;
	c->size = (size_t)width * (size_t)height * 4;
	image_cache_add(c, NULL, nsx_image_decode);

	kind = (size >= 4 && memcmp(data, "\x89PNG", 4) == 0) ?
			"PNGTitle" : "JPEGTitle";
	title = messages_get_buff(kind,
			nsurl_access_leaf(llcache_handle_get_url(c->llcache)),
			c->width, c->height);
	if (title != NULL) {
		content__set_title(c, title);
		free(title);
	}
	content_set_ready(c);
	content_set_done(c);
	content_set_status(c, "");
	return true;
}

static nserror nsx_image_clone(const struct content *old,
		struct content **newc)
{
	struct content *c = calloc(1, sizeof(struct content));
	nserror error;

	if (c == NULL) {
		return NSERROR_NOMEM;
	}
	error = content__clone(old, c);
	if (error != NSERROR_OK) {
		content_destroy(c);
		return error;
	}
	if ((old->status == CONTENT_STATUS_READY ||
	     old->status == CONTENT_STATUS_DONE) && !nsx_image_convert(c)) {
		content_destroy(c);
		return NSERROR_CLONE_FAILED;
	}
	*newc = c;
	return NSERROR_OK;
}

static const content_handler nsx_image_handler = {
	.create = nsx_image_create,
	.data_complete = nsx_image_convert,
	.destroy = image_cache_destroy,
	.redraw = image_cache_redraw,
	.clone = nsx_image_clone,
	.get_internal = image_cache_get_internal,
	.type = image_cache_content_type,
	.is_opaque = image_cache_is_opaque,
	.no_share = false,
};

/* The host sniffs the format from the data, so one handler serves both. */
static const char *nsx_image_types[] = {
	"image/png",
	"image/x-png",
	"image/jpeg",
	"image/jpg",
	"image/pjpeg",
};

CONTENT_FACTORY_REGISTER_TYPES(nsx_image, nsx_image_types, nsx_image_handler);
