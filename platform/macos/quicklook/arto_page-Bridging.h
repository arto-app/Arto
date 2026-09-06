#ifndef ARTO_PAGE_BRIDGING_H
#define ARTO_PAGE_BRIDGING_H

/* C ABI exported by the Rust `arto_page` static library (see crates/arto-page, feature `ffi`). */

/* Render the Markdown file at `path_utf8` to a self-contained HTML document.
 * Returns a Rust-allocated C string, or NULL on failure. Release it ONLY with
 * arto_page_free_string — never with libc free(3). */
char *arto_page_render_markdown_file(const char *path_utf8);

/* Free a string returned by arto_page_render_markdown_file. */
void arto_page_free_string(char *ptr);

#endif /* ARTO_PAGE_BRIDGING_H */
