/*
 * nsx form bodies: NetSurf hands a fetcher a form as a list of name/value
 * parts; a host fetcher wants the bytes, so a multipart form is encoded here
 * as multipart/form-data (RFC 7578).
 */
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "content/fetch.h"

#include "nsx_internal.h"

/* A growable byte buffer. */
struct buf {
	uint8_t *data;
	size_t len;
	size_t cap;
	bool failed;
};

static void buf_add(struct buf *b, const void *data, size_t len)
{
	if (b->failed || len == 0) {
		return;
	}
	if (len > SIZE_MAX - b->len) {
		b->failed = true;
		return;
	}
	if (b->len + len > b->cap) {
		size_t cap = b->cap ? b->cap : 256;
		while (cap < b->len + len) {
			if (cap > SIZE_MAX / 2) {
				cap = b->len + len;
				break;
			}
			cap *= 2;
		}
		uint8_t *grown = realloc(b->data, cap);
		if (grown == NULL) {
			b->failed = true;
			return;
		}
		b->data = grown;
		b->cap = cap;
	}
	memcpy(b->data + b->len, data, len);
	b->len += len;
}

static void buf_str(struct buf *b, const char *s)
{
	buf_add(b, s, strlen(s));
}

/* A quoted-string value for a Content-Disposition parameter: quotes and line
 * breaks are percent-encoded, as browsers do, so a name cannot end the
 * parameter early. */
static void buf_quoted(struct buf *b, const char *s)
{
	buf_str(b, "\"");
	for (; *s != '\0'; s++) {
		switch (*s) {
		case '"':
			buf_str(b, "%22");
			break;
		case '\r':
			buf_str(b, "%0D");
			break;
		case '\n':
			buf_str(b, "%0A");
			break;
		default:
			buf_add(b, s, 1);
		}
	}
	buf_str(b, "\"");
}

/* Appends the contents of the file at `path`; false if it cannot be read. */
static bool buf_file(struct buf *b, const char *path)
{
	char chunk[8192];
	size_t n;
	FILE *fp = fopen(path, "rb");

	if (fp == NULL) {
		return false;
	}
	while ((n = fread(chunk, 1, sizeof(chunk), fp)) > 0) {
		buf_add(b, chunk, n);
	}
	bool ok = !ferror(fp);
	fclose(fp);
	return ok;
}

static const char *leafname(const char *path)
{
	const char *slash = strrchr(path, '/');
	return slash != NULL ? slash + 1 : path;
}

bool nsx_post_multipart(const struct fetch_multipart_data *parts,
		uint8_t **body, size_t *len, char **content_type)
{
	static unsigned counter;
	char boundary[64];
	struct buf b = { 0 };

	/* The boundary only has to be absent from the parts; a value that
	 * holds a line starting with it is vanishingly unlikely. */
	snprintf(boundary, sizeof(boundary), "----nsxFormBoundary%08lx%08x",
			(unsigned long)time(NULL), ++counter);
	for (; parts != NULL; parts = parts->next) {
		buf_str(&b, "--");
		buf_str(&b, boundary);
		buf_str(&b, "\r\nContent-Disposition: form-data; name=");
		buf_quoted(&b, parts->name);
		if (parts->file) {
			buf_str(&b, "; filename=");
			buf_quoted(&b, leafname(parts->value));
			buf_str(&b, "\r\nContent-Type: application/octet-stream");
		}
		buf_str(&b, "\r\n\r\n");
		if (!parts->file) {
			buf_str(&b, parts->value);
		} else if (parts->rawfile != NULL && parts->value[0] != '\0' &&
				!buf_file(&b, parts->rawfile)) {
			free(b.data);
			return false;
		}
		buf_str(&b, "\r\n");
	}
	buf_str(&b, "--");
	buf_str(&b, boundary);
	buf_str(&b, "--\r\n");

	size_t ct_len = strlen(boundary) + 64;
	*content_type = malloc(ct_len);
	if (b.failed || *content_type == NULL) {
		free(*content_type);
		free(b.data);
		return false;
	}
	snprintf(*content_type, ct_len,
			"Content-Type: multipart/form-data; boundary=%s",
			boundary);
	*body = b.data;
	*len = b.len;
	return true;
}
