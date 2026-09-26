# 脚注・リンクのホバープレビュー

## 目的

読んでいる場所を離れずに参照先を確かめられるようにする。脚注参照・ページ内 `#見出し` リンクにポインタを置くと、その本文をポップオーバーで見せる。別の Markdown 文書への相対リンク（`./other.md`、`other.md#section`、wiki リンク `[[Other]]`）では、その文書の冒頭（フラグメントがあればその見出しの節）を見せる。クリックしたときの挙動（遷移・中クリックで新ウィンドウ）は変えない。

## 既存の仕組み（流用するもの・影響するもの）

- **HTML 契約（脚注）**: 参照は `<sup><a href="#fn-<id>" id="fnref-<id>">N</a></sup>`、定義は末尾の `<section class="footnotes"><ol><li id="fn-<id>">`。戻りリンク `<a href="#fnref-…" aria-label="Back to reference …">↩</a>` が本文末尾に入る — `crates/arto-markdown/src/lib.rs:98-104`、実例は `crates/arto-markdown/tests/snapshots/samples__samples_render_as_before@02-blocks.md.snap:388,463-474`。
- **HTML 契約（リンク）**: ローカル Markdown へのリンクは `<span class="md-link" data-md-link="…">` に変換され、`onmousedown` で `window.handleMarkdownLinkClick` を呼ぶ。フラグメントのみのリンクは `<a href="#…">` のまま — `crates/arto-markdown/src/lib.rs:113-131`, `crates/arto-markdown/src/post_process.rs:476-515`。`md-link-missing` / `md-link-invalid` はプレビュー対象外にできる。
- **見出し id**: アプリは `render_to_html_with_toc` で描くので見出しに id が付く（`crates/arto/src/markdown.rs:210`）。id は描画済み HTML から読み戻す方式で、重複時の `-1` 接尾辞も含め TOC と一致 — `crates/arto-markdown/src/engine/outline.rs:1-6`。
- **wiki リンク**: `[[Page]]` は同じディレクトリの `Page.md` を指す href になり、以降は通常の `.md-link` — `crates/arto-markdown/src/engine/wiki.rs:10`。
- **リンク解決**: `split_link_fragment`（フラグメント分離と percent-decode）`crates/arto/src/document_link.rs:26`、パス解決と canonicalize `crates/arto/src/document_link.rs:50-60`。プレビューも同じ規則で解決する。
- **JS→Rust ブリッジ**: `document::eval` で `window.handleMarkdownLinkClick` を仕込み `dioxus.send` を `eval.recv` で受ける — `crates/arto/src/components/content/file_viewer.rs:391-418`。`window.Arto` の準備待ちリトライは `crates/arto/src/components/app/mouse_navigation.rs:30-55`。Rust→JS は `window.Arto?.lenses?.fn?.(json…)` 形式の eval — `crates/arto/src/lenses/controller.rs:1196-1199`。
- **既存ポップオーバー**: lens の注記ポップオーバー（遅延 `HOVER_DELAY_MS = 400` `frontend/src/lenses.ts:60`、`showPopover` の配置とズーム補正 `frontend/src/lenses.ts:684-731`、`contentZoom` `:734`、`mouseover`/スクロールで閉じる `setup` `:825-853`）。CSS は `.lens-popover` 系 `frontend/style/components/lenses.css:92-180`（`--z-context-menu`、`max-height: 50vh`）。
- **レンダラ設定**: `render_options()` が `CONFIG.markdown` を使い、画像は `Deferred` で配信登録 — `crates/arto/src/markdown.rs:199-217`。信用しない入力向けの `render_detached`（raw HTML を Escape、外部取得を除去）`crates/arto/src/markdown.rs:253-266`, `crates/arto-markdown/src/lib.rs:280-305`。
- **キーボードカーソルのリンク**: `a[href], span.md-link[data-md-link]` を拾う `frontend/src/content-cursor.ts:378-388`、`file.open_link`（Cmd+Enter）は `crates/arto/src/keybindings/dispatcher.rs:396-426`、アクション定義 `crates/arto-keybindings/src/action.rs:81-82,577-578`、既定キー `crates/arto-keybindings/src/presets/default.json:83-88`。
- **初期化**: `lenses.setup()` を呼ぶ `frontend/src/runtime.ts:258`、`window.Arto` の組み立て `frontend/src/runtime.ts:395-440`。`arto page` 出力（`frontend/src/page.ts`）も同じ runtime の一部を使うため、文書内プレビューはそちらでも動かせる。

## 仕様（UI・操作・キーバインド・設定）

- **対象**: (a) 脚注参照 `sup > a[href^="#fn-"]` → 対応する `li#fn-…` の中身（戻りリンク ↩ は除去）。(b) `a[href^="#"]`（脚注以外）→ その見出しの節。(c) `span.md-link`（`-missing`/`-invalid` を除く）→ 他文書の冒頭または節。外部 URL は対象外。
- **遅延**: ポインタ静止 350ms で表示（lens と同程度。lens の 400ms と揃えるかは未決）。ポップオーバー上へ移動しても閉じない（離脱後 150ms の猶予）。スクロール・Esc・クリックで閉じる。
- **中身**: 脚注は全文。見出し節は「その見出しから同レベル以下の次見出しの手前まで」、冒頭は「最初の見出しより前 + 最初の節」。どちらも上限（目安 12 ブロック / 高さは CSS の `max-height: 50vh` でスクロール）で切り、切った場合は末尾に「…続きを開く」表示。ヘッダにファイル名（と見出し名）を出し、クリックでそのリンクを開く（`handleMarkdownLinkClick` と同じ経路）。
- **読み込み中/失敗**: 他文書は非同期なので「Loading…」を先に出さず、結果が来てから出す（遅延内に間に合わなければ到着時に表示）。読めない・大きすぎる場合は「プレビューできません」。
- **キーボード**: 新アクション `file.preview_link`（既定 `Cmd+Shift+Space` 案、vim `K`、emacs は未割当）でカーソル位置のリンクをプレビュー。表示中は Esc で閉じる。ポップオーバーは `role="tooltip"` + リンク側 `aria-describedby`。
- **設定**: `config.json` に `reading.linkPreview`（`enabled: bool`, 既定 true）を足すかは未決。最小案は設定なし（常時オン）。

## 設計（データ構造、保存先を Config / PersistedState / State / 別ファイルのどれにするかと理由、Rust と frontend の分担）

- **保存先**: ランタイム状態のみ（ポップオーバーは JS 側の一時 DOM）。キャッシュは Rust プロセス内メモリ（別ファイルに書かない）。オン/オフを入れるなら Config（ユーザーが決める既定・全ウィンドウ共通、PersistedState/State は不要）。
- **frontend（新規 `frontend/src/link-preview.ts`）**: `mouseover` を document で拾い、対象判定 → タイマー → 表示。(a)(b) は DOM だけで完結（`getElementById` → `cloneNode(true)`、id 属性と `data-source-range` を除去して重複 id を防ぐ）。節の切り出しは純関数 `sectionOf(root, heading, maxBlocks)` として DOM 上で行う（兄弟要素を次の同レベル以下の見出しまで）。(c) は `window.Arto.linkPreview.onRequest(cb)` で登録されたコールバックに `{ seq, link }` を渡し、Rust から `window.Arto.linkPreview.resolve(seq, html | null)` で返ってきた HTML を `<template>` に入れて同じ `sectionOf` で切る。`seq` は最新要求以外の応答を捨てるため。
- **ポップオーバー共通化**: `lenses.ts` の配置・ズーム補正（`showPopover` の後半と `contentZoom`）を `frontend/src/popover.ts`（新規）に切り出し、lens と link-preview の両方が使う。見た目は `.lens-popover` と同系の `.link-preview`（新 CSS `frontend/style/components/content/link-preview.css`）。
- **Rust（新規 `crates/arto/src/link_preview.rs` + `components/content/file_viewer.rs` にフック `use_link_preview_handler`）**: `split_link_fragment` と `document_link` と同じ解決で canonical path を得る → `is_markdown_file` 確認 → サイズ上限（例 2 MiB 超は拒否）→ `tokio::fs::read_to_string` → `spawn_blocking` で `markdown::render_to_html_with_toc` 相当を描画。
  - **同じレンダラ設定**: `render_options()` をそのまま使い、見出し id が本体と一致するようにする。ただし `raw_html` は `min(reader, Filter)` に落とす（ホバーは「開いた」ことではないため `Allow` の文書でも `on*`/`<script>` 相当を素通しにしない）。新関数 `markdown::render_preview` として `markdown.rs` に追加。
  - **リンクの付け替え**: プレビュー HTML 内の `data-md-link` は対象文書基準の相対パスなので、そのままでは現在文書基準で誤解決される。`lol_html` で `data-md-link` を絶対パスへ書き換える（`base_dir.join(abs)` は絶対パスを返すので `open_document_link` 側は無変更で動く）。この書き換えは arto-markdown に純関数（例 `rebase_md_links(html, base_dir)`）として追加しテスト可能にする。
  - **フラグメントの扱い**: 節の切り出しは JS 側で行う（id の規則を Rust で再実装しないため。id は描画結果から決まる — `outline.rs:1-6`）。Rust は全文 HTML を返す。巨大文書の転送量はサイズ上限で抑える。
  - **キャッシュ**: `HashMap<PathBuf, (SystemTime mtime, u64 len, Arc<String>)>` の LRU（32 件程度）を `link_preview.rs` の `LazyLock<Mutex<…>>` に置く。mtime/len が一致すれば再描画しない。画像登録（`assets::images::register`）はキャッシュヒット時も有効なまま（プロセス内で保持される前提、要確認）。
- **安全性**: 描画は reader の信頼境界内（ローカル文書）だが、ホバーのみで実行されうる属性は `Filter` 以上で除去。挿入は `innerHTML` だが `<script>` は実行されない。外部画像は現行本体と同じ扱い（本体で許しているものは許す）。

## 実装手順（1 ステップ = 1 コミット目安）

1. `frontend/src/popover.ts` を新設し、`lenses.ts` の配置・ズーム処理を移す（挙動不変、`lenses.test.ts` が通ること）。
2. `link-preview.ts`: 脚注とページ内 `#` リンクのプレビュー（DOM 完結）、`sectionOf` 純関数、CSS、`runtime.ts` で `setup()`。
3. arto-markdown: `rebase_md_links`（`data-md-link` を絶対化）を追加しユニットテスト。
4. Rust `link_preview.rs`: 解決・サイズ上限・描画（`render_preview`）・LRU キャッシュ。純ロジックにテスト。
5. `file_viewer.rs` にブリッジ（`onRequest`/`resolve`、seq 管理）を追加し、JS 側で `.md-link` 対応を有効化。
6. キーボード: `Action::FilePreviewLink`（`file.preview_link`）を `arto-keybindings` に追加し、各プリセットと dispatcher（`getCurrentElement` のリンクを対象に JS の `showFor(el)` を呼ぶ）を実装。スキーマ生成物更新。
7. （設定を入れる場合）`arto-config` に `reading.linkPreview` と Preferences の Reading タブに `ToggleRow`、`schemas/` 更新。

## テスト方針

- vitest（happy-dom）: 対象判定、`sectionOf`（同レベル/上位見出しで止まる・上限で切る・見出しなし文書）、脚注の ↩ 除去と id 除去、seq の古い応答破棄、遅延とスクロールで閉じる挙動（fake timers）。`lenses.test.ts` は手順 1 の回帰確認。
- Rust: `rebase_md_links`（相対→絶対、フラグメント維持、`-missing` 維持）、プレビュー用オプションで `raw_html` が Filter 以下になること（`markdown.rs` の既存テストと同形式）、サイズ上限・非 Markdown 拒否・キャッシュの mtime 無効化を `tempfile` で。
- スナップショット: 本体 HTML 契約は変えないので `samples` の差分は出ない想定。出たら意図外。
- `just fmt check test`。

## 規模感（S/M/L）とリスク

**M**。(a)(b) だけなら S。リスク: lens の `mouseover` ハンドラとの競合（lens マーカー上ではリンクプレビューを出さない優先順位が要る）、ズーム下の配置、巨大文書の転送量、`data-md-link` 付け替えの漏れ（プレビュー内のリンククリックが誤ったファイルを開く）、`Allow` 利用者にとってプレビューと本体の見た目差。

## 未決事項（ユーザー判断が必要な点）

1. 他文書プレビューの `raw_html` を `Filter` に落とす方針でよいか（本体と完全一致を優先するか）。
2. 遅延時間（350ms / lens と同じ 400ms）と、オン/オフ設定を Config に持つか。
3. キーボードの既定キー（`Cmd+Shift+Space` / vim `K`）。
4. プレビューの上限（ブロック数・ファイルサイズ）と、`arto page` 出力でも脚注/`#` プレビューを有効にするか。
