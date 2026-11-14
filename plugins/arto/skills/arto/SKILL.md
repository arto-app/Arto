---
name: arto
description: Show Markdown to the user in Arto and work with what they are reading there, through the arto MCP server. Use when the user asks to open, show or preview a Markdown file or directory in Arto, when a document you wrote is ready for them to read, when they refer to "this part" or "what I selected" while Arto is open, or when you need to check how a diagram, formula or table came out.
---

# Arto

Arto is a desktop app for reading Markdown. The `arto` MCP server this plugin
registers runs `arto mcp`, which talks to the Arto the user has open. Its tools
are the only way to drive it: do not run `arto` or `open -a Arto` from the
shell, which hangs when Arto is not running yet.

## Tools

| Tool | Use it to |
| --- | --- |
| `open` | Put files, or one directory for the file explorer, in front of the user |
| `state` | Learn which windows are open, which file each shows, its headings and where the reader is |
| `goto` | Move the reader to a heading, a place `state` reported, the top, the bottom or a search hit |
| `selection` | Read the text the user has selected |
| `capture` | See how Arto drew a block, a heading's section or the visible page, as an image |

## Showing a document

1. Pick the file: the one the user named, else the Markdown file you just
   wrote or edited. Content that exists only in the conversation goes into a
   `.md` file under the project first, where `open` is allowed to reach it.
2. Call `open` with its path. It answers once the document is drawn, and
   starts Arto when it is not running. Pass `newWindow` only when the user
   asks for a separate window, and `activate` only when they want Arto in
   front of what they are working in.
3. When one section is what matters, call `state` for the window's headings
   and `goto` the one you mean.
4. Report in one line what you opened. Do not read the document back; the
   user is reading it in Arto.

This plugin starts the server without `--allow`, so `open` reaches only the
project directory and refuses anything outside it. When it refuses, do not
copy the file somewhere it can reach. Tell the user the directory it needs,
and that registering the server themselves allows it, for example
`claude mcp add --scope user arto -- arto mcp --allow <dir>`.

## Following the reader

"This", "here" or "the part I am looking at" while Arto is open means the
reader's place: call `state` and use the focused window's file and `scroll`.
"What I selected" means `selection`. Read the file itself for the content;
the tools say where the reader is, not what the file contains.

## Checking what Arto drew

After writing a Mermaid diagram, math, a wide table or anything whose
rendering you are unsure of, `open` the file and `capture` the block by its
source line (`{"block": line}`) or heading id. Look at the image, fix the
source if it came out wrong, and capture again. Capturing never moves the
reader.
