# 自分用ハイライト（User highlights）

## 目的

読みながら選択したテキストに色付きの印（マーカー）を引き、文書ごとに保存して、次に開いたときに復元する。
印は右ガターの tick と Contents 一覧に現れ、一覧から辿ったり消したりできる。
ファイルは一切書き換えない（Arto は読むためのツール）。文書が編集されても、可能な限り同じ箇所に印が残るようにする。

## 既存の仕組み（流用するもの・影響するもの）

- Pinned search（語の常設ハイライト）: `crates/arto/src/pinned_search.rs:61` `HighlightColor`（green/blue/pink/orange/purple。yellow はアクティブ検索専用）。
  `:117` `PinnedSearch`。保存先は `:174` `pinned-searches.json`（**全文書共通**）。変更通知は `PINNED_SEARCHES_CHANGED`。
  → 色の列挙・CSS 変数（`frontend/style/variables.css:180` `--mark-green` ほか）・変更通知と JS への反映パターン
  （`components/app/listeners.rs:73-100` `setup_pinned_highlights`）をそのまま流用する。
- DOM への着色: `frontend/src/find-in-page.ts:79` のテキストノード走査と span 包み（`pre`・mermaid・未描画数式は除外, `:99-110`）。
  `:271-296` の pinned 適用、`:446` `getContext`（前後文脈）。
  スタイルは `frontend/style/components/content/highlights.css:42-69`（`.pinned-highlight[data-color]`, `-flash`）。
- ガター・一覧: `components/content/gutter.rs:139` `Names` の中で `:175` `PinnedMarks` を見出しの上に並べる。
  一覧の行は `components/pinned_marks.rs:35` `PinnedMark`（色・表示切替・削除のポップオーバー）。
  tick への着色は `frontend/src/reading-position.ts:111` `hitsByHeading` が `.pinned-highlight` を見出し単位で集計する（`:124-138`）。
- 右クリック: `frontend/src/context-menu-handler.ts:23` `ContextMenuData`（`has_selection`, `selected_text`, `source_line(_end)`）。
  `:136` `getSourceLineRange`（選択 → ソース行）、`:393` `getTextSelection` / `:413` `restoreSelection`（メニュー中も選択を保持）。
  Rust 側は `components/content/context_menu/data.rs:32` と `components/content/context_menu.rs:145`（Copy）、`:260`（Find in Page）。
- アンカーの参考: レンズの答えは `lenses/store.rs` でブロック位置（`block:<n>`）とリクエストキーで保存し、編集後は stale として残す。
  ブロックとソース行の対応は `data-source-range`（`crates/arto-markdown/src/lib.rs:33-60`、`frontend/src/source-range.ts:36`）。
- 保存先の前例: `bookmarks.rs:76` `data_file()`（`data_local_dir()/arto/`）、`lenses/store.rs` の文書パス SHA-256 ファイル名・`write_atomically`。
- 描画後の再適用: `components/content/file_viewer.rs:291` `reapply_search`（MutationObserver → `Arto.search.reapply`）。

## 仕様（UI・操作・キーバインド・設定）

- 付ける操作:
  - 右クリックメニュー（選択があるとき）に「Highlight」サブメニューを追加する。5 色と「Remove Highlight」（選択が既存ハイライトに重なるとき）を並べる。
  - キー `highlight.add` で、選択を「最後に使った色」でハイライトする。既定案は default が `Cmd+Shift+H`、vim が `m`（ビジュアル選択なし前提なので選択があるときのみ）。
- 見た目: 背景色のマーカー（`.user-highlight[data-color]`）。pinned search とは下線／背景を変えて区別する（pinned は背景、user は太めの下半分マーカー）。
  重なった場合は user を上に描く。
- ガター: ハイライトを含む見出しの tick に色を付ける（`data-hit` と同じ仕組み）。Contents 一覧に「Highlights」節を Pinned の下・見出しの上に置く。
  各行は色の点・引用の冒頭 40 字・所在見出しで構成する。クリックで移動＋フラッシュ、ポップオーバーで色変更・削除。
- 復元できなかった印（orphan）は一覧の末尾に淡色で残す。選べば「本文から見つかりません」と示し、削除だけできる。
- 範囲: 段落・見出し・リスト・表セル・引用・インライン code を対象にする。コードブロック（`pre`）・mermaid・数式は対象外（find-in-page と同じ除外）。
- 設定: 追加しない（色の既定は最後に使った色を State で持つ）。将来の `highlights.enabled` は不要と判断する。

## 設計（データ構造、保存先を Config / PersistedState / State / 別ファイルのどれにするかと理由、Rust と frontend の分担）

- アンカー（W3C Web Annotation の TextQuoteSelector と TextPositionSelector に、ソース行ヒントを足す）:
  ```rust
  struct Highlight { id: HighlightId /* "hl_<uuid>" */, color: HighlightColor, created_at: DateTime<Utc>,
      anchor: TextAnchor, note: Option<String> /* 将来用・UI なし */ }
  struct TextAnchor { exact: String, prefix: String /* 直前 32 字 */, suffix: String /* 直後 32 字 */,
      start: u32 /* 描画テキスト全体での文字オフセット */, line: u32 /* 開始ブロックの data-source-range 開始行 */ }
  ```
  - 描画テキストで持つ（ソースの列ではなく）。理由: 選択は描画後の DOM で行われ、強調記法やリンク記法を挟むと、ソース列への逆写像が不確実になる。
  - 復元手順:
    1. `line` のブロック内で `prefix+exact+suffix` を完全一致で探す。
    2. 文書全体で `exact` を探し、prefix/suffix の一致長と `start` からの距離で採点する。
    3. 閾値未満なら orphan にする。
    - 見つかった場合は `start` と `line` を更新して保存し直す（位置が編集に追随する）。
- 保存先: **別ファイル**。`data_local_dir()/arto/highlights/<sha256(path)>.json`（1 文書 1 ファイル、`{ version, path, highlights: [] }`）。
  - Config: ユーザー設定ではないので不可。
  - PersistedState: 文書ごとでないので不可。
  - State: 永続しないので不可。
  - pinned-searches.json: 全文書共通であり、文書数に比例して肥大するので同居させない。
  - 文書ごとのファイルなら、開いた文書の分だけ読み書きすればよい。上限は設けないが、`utils/data_store`（01 の計画で切り出すもの）の書き込みを共用する。
- pinned search との関係: 色の enum（`HighlightColor`）と CSS 変数を共有する。`pinned_search.rs` から `highlight_color.rs` に移す（re-export で互換を保つ）。
  データと寿命は別にする。pinned は「語」に全文書で付き、user は「箇所」に 1 文書で付く。一覧も別節にする。
- Rust（新規 `crates/arto/src/highlights.rs`）: `Highlights::load(path)` / `save`、`add` / `remove` / `set_color` / `rebase(id, start, line)`、
  グローバル `HIGHLIGHTS_CHANGED: broadcast::Sender<PathBuf>`（同じ文書を開いた他ウィンドウへ）、`AppState.highlights: Signal<Vec<Highlight>>`。
  文書切替時に読み込み、変更のたびに書き込む。
- frontend（新規 `frontend/src/text-anchor.ts`, `frontend/src/user-highlights.ts`）:
  - `text-anchor.ts`: 描画テキストの索引（テキストノード列と累積オフセット。find-in-page と同じ除外）、`describe(range) → TextAnchor`、`resolve(anchor) → Range | null`（純関数寄りにしてテスト可能にする）。
  - `user-highlights.ts`: `set(highlights)` で解決して span を包み（複数ノードにまたがる場合は各ノードで分割）、`resolved` と `rebased` を Rust に返す。`scrollTo(id)` を持つ。
  - `window.Arto.highlights = { set, describeSelection, scrollTo }`（`runtime.ts:277`）。
  - `reading-position.ts:124` の集計に `.user-highlight` を加える。
- 付与の流れ: メニュー／キー → Rust が `Arto.highlights.describeSelection()` を eval → `TextAnchor` 受信 → 追加・保存 → `set` で再描画。
  メニュー経由では、`restoreSelection` 済みの `savedRange` を使う（`context-menu-handler.ts:383`）。
- 再描画時: `reapply_search` の後に `Arto.highlights.set(...)` を呼ぶ（`file_viewer.rs:156` の直後）。

## 実装手順（1 ステップ = 1 コミット目安）

1. `HighlightColor` を独立モジュールへ移す（振る舞いは不変）。
2. `highlights.rs` を追加する: 型・文書ごとファイルの load/save・add/remove/set_color/rebase と単体テスト。
3. `frontend/src/text-anchor.ts` を追加する: 索引・describe・resolve（採点と orphan 判定）と vitest。
4. `frontend/src/user-highlights.ts` と CSS（`highlights.css` に `.user-highlight`）を追加し、`window.Arto.highlights` を公開する。
5. `AppState.highlights` と、文書切替・再描画時の `set` 配線、`rebase` の書き戻しを追加する。
6. 右クリックの Highlight サブメニューと、Action `highlight.add` / `highlight.remove` を presets に追加する。
7. Contents 一覧の Highlights 節（`components/user_highlights.rs`、`PinnedMarks` と同じ行 UI）と tick 着色を追加する。
8. `HIGHLIGHTS_CHANGED` による他ウィンドウ同期と orphan 表示を追加する。

## テスト方針

- Rust: `tempfile` の root で保存→読込、壊れたファイルは空で読む、`rebase` が永続されること、パスの表記揺れ（`utils::paths::true_spelling`）を確認する。
- frontend（vitest + jsdom、`lenses.test.ts` と同様に HTML 断片を組む）:
  - `describe` → `resolve` の往復（単一ノード、太字をまたぐ、段落をまたぐ）。
  - 編集耐性: 前に段落を挿入する、同じ語が複数ある（prefix/suffix で正しい方を選ぶ）、語の一部を書き換える（orphan になる）。
  - 除外領域（`pre`・`.mermaid`）を選んでも付かないことを確認する。
- 手動確認はユーザーに依頼する（アプリは起動しない）。最後に `just fmt check test`。

## 規模感（S/M/L）とリスク

- 規模: **M**（Rust 約 450 行＋TS 約 400 行＋CSS。8 コミット前後）。
- リスク:
  - DOM を span で包む方式は、検索・pinned・レンズ（`lenses.ts` がブロック要素を tag する）・KaTeX の遅延描画と干渉しうる。
    CSS Custom Highlight API（`CSS.highlights`）なら DOM 非破壊だが、WKWebView は Safari 17.2 相当が必要になる。
    `platform/macos/quicklook/Info.plist:13` は macOS 12 を最低としており、アプリ本体の最低版が不明なので既定は span 方式にする。
  - 選択の端がテキストノードの途中・`<br>`・脚注参照にかかるときのオフセット計算。
  - 描画テキストはレンダラの変更（例: スマート引用符の設定）で変わりうる。その場合は exact 一致が崩れて orphan が増える。

## 未決事項（ユーザー判断が必要な点）

1. 描画方式: span 包み（互換重視、既定案）か CSS Custom Highlight API（非破壊・実装が簡潔、macOS 14.2+ 必須）か。アプリの最低 macOS 版の確認も要る。
2. キーバインド既定（`Cmd+Shift+H`、vim の `m`）と、色を選ぶ UI（右クリックのみか、キーで色を巡回するか）。
3. 一言メモ（note）を最初から付けるか。データには枠だけ用意する案にしている。
4. 保存場所を Arto のデータ領域（既定案）にするか、文書の横（`.arto/` や sidecar ファイル）にしてリポジトリと一緒に持ち運べるようにするか。
5. pinned search と一覧を同じ節にまとめるか（既定案は別節）。ファイル移動・改名時に追随させるか（現状 visits も追随しない）。
