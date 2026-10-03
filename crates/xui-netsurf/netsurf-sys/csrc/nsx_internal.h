/* Shared between the nsx glue files; not part of the host interface. */
#ifndef NSX_INTERNAL_H
#define NSX_INTERNAL_H

#include <stdbool.h>

#include "utils/errors.h"
#include "netsurf/bitmap.h"
#include "netsurf/layout.h"
#include "netsurf/plot_style.h"
#include "netsurf/window.h"

#include "nsx.h"

/* The host given to nsx_init. */
extern const nsx_host *nsx_host_v;

extern struct gui_window_table *nsx_window_table;
extern struct gui_bitmap_table *nsx_bitmap_table;
extern struct gui_layout_table *nsx_layout_table;
extern const struct plotter_table nsx_plotters;

/* Sets the bitmap pixel format NetSurf decodes into. */
void nsx_bitmap_init(void);

/* Resolves a NetSurf font style to plain values. */
void nsx_font_from_style(const plot_font_style_t *fstyle, nsx_font *out);

/* Registers the PNG and JPEG content handlers, decoded by the host. */
nserror nsx_image_init(void);

struct fetch_multipart_data;
/* Encodes a multipart form as a multipart/form-data body (malloc'd) and its
 * "Content-Type: ..." request header line (malloc'd); false on failure. */
bool nsx_post_multipart(const struct fetch_multipart_data *parts,
		uint8_t **body, size_t *len, char **content_type);

/* A NetSurf colour (inverted alpha, BGR) as straight 0xAARRGGBB. */
uint32_t nsx_colour(colour c);

#endif
