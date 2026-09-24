# Lenses

A lens is something you look at a document through: a command-line agent, a
language-model server or a command of your own is handed the document's
source, and its answer is
shown with the document — a translation in the document's places, a summary
on request, a note beside each paragraph. The file itself is never changed.

Arto reaches out to nothing on its own. A lens runs only when you open it,
and only what you configured it with is handed the text it looks at: a
program on your machine, or the server you named. Whether the text leaves the
machine is up to what you chose — an Ollama server on your own machine keeps
it there.

## Three ways to show an answer

| `display` | The agent runs | The answer is shown |
| --- | --- | --- |
| `page` | once, on the whole document | in the document's places, block by block from the top as it is written — a translation |
| `popover` | once, on the whole document or on one block | from the header for the document, beside the block for a block — a summary |
| `annotate` | once per block, with the blocks around it as context | beside each block, on hover over a mark in its margin — a gloss, an explanation |

A `page` answer is paired with the document block by block, in order: the
answer's first block takes the place of the document's first, and so on, so
the page reads as the answer from the top down while the rest is still the
document. An answered block under the pointer shows a mark in its right margin, since
the left one holds the marks of the lenses over the page. Point at the mark, or hold Option (Alt) over the block, to see the block it
stands for. Resting on the block alone leaves the answer uncovered, since
the pointer rests on the page all through reading. The
header counts the blocks answered. An answer that keeps the document's structure — as a
translation does — pairs cleanly; one that merges or splits blocks still
shows, but Option pairs it with the wrong originals.

Any number of page lenses can be open, but one takes the page at a time,
since two would take the same places: the lens glyph in the header chooses which,
or **Original**, the document as written, and switching is immediate since
their answers are kept. The others only add to the page, so any number of
them can show beside it and beside each other — summarize a translated page
and the translation stays. A lens opened again replaces its own earlier
answer. A block marked by several lenses carries one mark, and hovering it
shows each lens's answer under the lens's label.

A page lens with `unit: "block"` hands the document over one top-level block
at a time instead — code, diagrams and formulas stay as written — and shows
each answer once every block above it has one, so the page still turns over
from the top down. Every block keeps its place whatever the model does, which
suits a model that drops or merges blocks when given a whole document; the
price is that no run sees the rest of the document.

An answer is Markdown written by a model, which the document it read can
talk into writing anything, so it is shown with less trust than the document
itself: raw HTML in it is shown as written, whatever `markdown.rawHtml`
allows, and nothing in it is fetched from another host — an image from
elsewhere shows as its alt text. A document's `<kbd>` therefore shows as the
tag in its translation.

A lens's answers are kept on disk, with the app's data rather than its
caches, and a document opens with the lenses it had open when it was last
read — after a restart too — showing what they answered without asking
again. Where the document has changed since, the old answer is shown marked
outdated: a note's mark goes hollow, a translated block carries a rule in its
margin, and the lens menu offers to regenerate just those. A lens stopped
before it finished leaves the rest not answered, and the same menu offers to
continue it. Nothing is asked again until one of those is chosen; a lens
over a document it never answered about asks at once. A document that
changes on disk while it is open is treated the same way. To ask a lens
again about everything, whatever it answered before, use the regenerate
button at the end of its row in the lens menu, or **Regenerate all** in the
right-click menu.
Hiding a lens takes its answer off the page but keeps the lens in the header,
to be shown again at once, and the document opens with it hidden. To be rid
of a lens and its answers for a document, choose **Forget …'s answers** from
the right-click menu — twice, since it cannot be undone.

## Configuring a lens

Lenses are edited in **Preferences → Lenses**, which lists them in the order
the menus offer them — drag one to move it — says beside a lens what keeps it
out of the menus, and opens each to show only the settings its display and
agent read. A lens is quickest started from a recipe — translate the page, a
translation beside each block, summarize, explain the terms, critique,
explain a block — which has its prompt and display written already and asks
only the language to answer in, who asks and on which model. The model field
suggests what the agent offers: `codex`'s catalog, the models an Ollama or
OpenAI-compatible server lists, and the aliases `claude` takes, since
`claude` has no list of its own. They are
kept in `lenses` in `config.json`, a list, which can be edited by hand as
well: Arto reads the file again when it is saved, so the menus offer an
edited lens without a restart. Name an `agent` Arto knows and give it a
`prompt`:

```json
{
  "lenses": [
    {
      "id": "translate-ja",
      "label": "Translate to Japanese",
      "display": "page",
      "agent": "claude",
      "model": "sonnet",
      "prompt": "Translate the text into Japanese. Keep its structure exactly and translate the prose only. Write the translated document and nothing else."
    },
    {
      "id": "translate-ja-local",
      "label": "Translate to Japanese (local)",
      "display": "page",
      "agent": "ollama",
      "model": "qwen3:4b-instruct",
      "prompt": "Translate the text into Japanese. Keep its structure exactly and translate the prose only. Write the translated document and nothing else."
    },
    {
      "id": "summarize",
      "label": "Summarize",
      "display": "popover",
      "agent": "openai",
      "endpoint": "https://api.openai.com/v1",
      "model": "gpt-5-mini",
      "prompt": "Summarize the text in a few sentences."
    }
  ]
}
```

| Field | Meaning | Default |
| --- | --- | --- |
| `id` | Names the lens; must be unique | — |
| `label` | What the menus and the header call it | — |
| `display` | `page`, `popover` or `annotate` | — |
| `agent` | `claude`, `codex`, `ollama` or `openai` | — |
| `model` | The model the agent uses; required for `ollama` and `openai` | the agent's own |
| `prompt` | What the agent is asked to do with the text; without one the agent is handed the text alone | — |
| `program` | `claude`, `codex`: where the program is, when it is not found on its own | found on `PATH` and in the usual install directories |
| `endpoint` | `ollama`: the server's address; `openai`: the base URL, the part before `/chat/completions` | `ollama`: `http://127.0.0.1:11434`; `openai`: `https://api.openai.com/v1` |
| `system` | `ollama`, `openai`: the system prompt sent with every request | — |
| `contextLength` | `ollama`: the context the model is loaded with, in tokens | sized to each request |
| `apiKeyCommand` | `ollama`, `openai`: a program and its arguments that print the API key, in place of the stored one | — |
| `command` | Instead of an `agent`: a program and its arguments, run as given | — |
| `context` | `annotate`: blocks on each side handed over as context, at most 20 | `2` |
| `concurrency` | `annotate`: runs in flight at once, 1 to 16 | `4` |
| `timeoutSeconds` | How long one run may take before it counts as failed | `300` |
| `unit` | `page`: `document` hands the whole document over in one run; `block` hands each top-level block over in a run of its own | `document` |
| `shortcut` | Keys that open, show or hide the lens over the document while it is being read, written as in the keybindings: `Cmd+Shift+t`, or `g t` for one chord after another | — |

A lens has either an `agent` or a `command`. One with neither or both, a
server agent with no `model`, a
server-only field on a command-line agent, an empty `id`, a `shortcut`
that cannot be read, a value outside
those bounds, or an `id` another lens also uses is left out, and the others
stay available. An unknown `display` or `agent` makes `config.json`
unreadable, like any other malformed value.

## Agents

An agent is asked the `prompt`, then a blank line, then the text and nothing
else — the one shape every model reads as the text, so a model trained for a
single task, such as a translation model, gets exactly what it expects. The
block of an `annotate` lens is set apart in `<text>` tags instead, with its
neighbours in `<before>` and `<after>`, since they have to be told apart.
An `annotate` answer of `<nothing/>` alone, or an empty one, leaves its block
unmarked, so a lens can be asked to note only what stands out — "if there is
nothing to note, answer with exactly `<nothing/>`" — and mark just those
blocks. Ask for the mark rather than for no answer: a model told to write
nothing tends to write a sentence saying there is nothing, and that sentence
would mark the block like any other note.

| `agent` | Asked through | The answer arrives |
| --- | --- | --- |
| `claude` | `claude -p` with no tools, no MCP servers, no settings or hooks, no slash commands, and a short system prompt of Arto's own, so `CLAUDE.md` files near the document do not turn it into an agent | as it is written |
| `codex` | `codex app-server` with its tools turned off — no shell, no web search — plugins, hooks and every MCP server its configuration names disabled, a read-only sandbox, a short system prompt of Arto's own, and no session kept | as it is written |
| `ollama` | Ollama's own chat API, with the context sized to the request | as it is written |
| `openai` | An OpenAI-compatible chat completions API: LM Studio, llama.cpp's server, vLLM, OpenAI, OpenRouter and the like | as it is written |

The command-line agents are run with as little of their own set-up as they
allow, because a lens asks a question about a text, and what an agent loads
beyond that only makes each run slower and its answer less predictable. They
use the sign-in you already have for them. Arto looks for the program on
`PATH` and in the usual install directories — Homebrew, `~/.local`, Nix
profiles — because an app opened from the Finder or the Dock does not get
your shell's `PATH`; set `program` when it lives elsewhere.

Ollama is asked through its own API rather than its OpenAI-compatible one
because only its own takes the context length with each request. Left to
itself, Ollama loads a model with the longest context the model takes, which
for a small model made for long contexts means tens of gigabytes of memory;
Arto asks for enough to hold the text and an answer as long as it, rounded up
so that requests of a similar size share a loaded model. Set `contextLength`
to choose it yourself.

A server that wants an API key gets the one stored for its endpoint in
**Preferences → Lenses**, which keeps it in the system's credential store —
the Keychain on macOS, the Credential Manager on Windows, the Secret Service
elsewhere — never in `config.json`, so the file can be kept in a dotfiles
repository. Every lens that asks the same endpoint uses the one key, and a
server with none stored, such as a local Ollama, is asked without one. Arto
does not read the key from the environment: an app opened from the Finder or
the Dock does not see the shell's. To keep the key in a password manager
instead, set `apiKeyCommand` to a program that prints it — 1Password's
`op read`, say — which is run for every request.

A model trained for a single task, such as a translation model, reads an
instruction as more text to work on. Leave `prompt` out for one, so it is
handed the text alone, and give it what it needs to know — the language to
translate into, say — in `system`.

## A command of your own

A lens with a `command` instead of an `agent` runs it as given, in the
directory of the document. It reads one JSON object on stdin and writes its
answer to stdout, as Markdown; a `page` lens shows the answer as it is
written, so a command that writes as it generates is read as it goes.

A `page` or `popover` run is given the whole of what it looks at:

```json
{
  "version": 1,
  "lens": "translate-ja",
  "range": null,
  "markdown": "The whole document, or the text of the block, as written.",
  "document": "/path/to/file.md"
}
```

`range` is `null` for the whole document; for a block it is where the text is
in the file, `L:C-L:C` in lines and columns, both ends inclusive.

An `annotate` run is given one block and its neighbours:

```json
{
  "version": 1,
  "lens": "explain",
  "kind": "paragraph",
  "markdown": "The block's own text, without list markers or quote prefixes.",
  "range": "12:1-14:20",
  "before": ["The block before it", "and the one before that"],
  "after": ["The block after it"],
  "document": "/path/to/file.md"
}
```

- `kind` is `paragraph`, `heading`, `list-item`, `table-cell`,
  `definition-term` or `definition`.
- `markdown` is the block's content: a heading without its `#`, an item
  without its marker or checkbox, lines of a quoted block without the `>`.
- `before` and `after` are the neighbouring blocks' content in document order,
  so the last of `before` is the block right before this one.

## Opening a lens

Right-click the page and choose **Lens on Document** or **Lens on Block**, then
the lens. Lens on Block looks at the block the right-click marked — for a
table, a list or a quote, all of it — and offers only `popover` and
`annotate` lenses, since a page is a whole document.

The header holds one lens glyph while any lens is open, since it is narrow
and the document is what is being read. While a lens is being answered an
hourglass turns over beside it; hovering or pressing the hourglass lists
what is being answered and how far each has got, with the way to stop it.
The lens glyph carries a dot while a lens has outdated answers, places left
not answered, or a failure, and hovering it says which. Pressing it opens
the menu of what the page shows — which page lens, or **Original**, and
which lenses over the page, where one being answered turns a small
hourglass — and of what can be done: regenerate or continue a lens. A summary opened over the document keeps a glyph of its own, which
opens its answer. In the right-click menu, a lens that is open is a submenu
of the same, and of forgetting its answers. Stopping and hiding are also
keybinding actions, `lens.stop` and `lens.hide`; see
[Keybindings](./keybindings.md).

## When a run fails

A run fails when the program cannot start, exits with a non-zero status,
reports an error of its own, takes longer than `timeoutSeconds`, writes more
than 16 MiB, or writes output that is not UTF-8. A failed block is marked in
the margin and hovering over it says why, including the last lines the program
wrote to stderr; a failed `page` or `popover` run says so in the header.

Stopping a lens, closing it, or leaving the document stops the program and
everything it started.
