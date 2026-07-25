#ifndef ARTO_QL_BRIDGING_H
#define ARTO_QL_BRIDGING_H

/* C ABI exported by the Rust `arto_ql` static library (see ql-generator). */

/* Render the Markdown file at `path_utf8` to a self-contained HTML document.
 * Returns a Rust-allocated C string, or NULL on failure. Release it ONLY with
 * arto_ql_free_string — never with libc free(3). */
char *arto_ql_render_markdown_file(const char *path_utf8);

/* Free a string returned by arto_ql_render_markdown_file. */
void arto_ql_free_string(char *ptr);

#endif /* ARTO_QL_BRIDGING_H */
