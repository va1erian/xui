/* Stands in for NetSurf's curl fetcher header, which content/fetch.c includes
 * unconditionally: this build has no curl fetcher (WITH_CURL is not set), so
 * nothing from it is needed. Found ahead of the real header by include order. */
#ifndef NETSURF_CONTENT_FETCHERS_FETCH_CURL_H
#define NETSURF_CONTENT_FETCHERS_FETCH_CURL_H
#endif
