# 集中モード（Focus mode）

## 目的

読んでいるブロックだけを残し、ほかの本文を薄くする。あわせてパネル・レール・ヘッダー・目次ルーラー・マージントレースを退けて、画面を文書だけにする。
長い文書を一段落ずつ追うときに、視線が隣のブロックへ流れないようにするのが狙い。

## 既存の仕組み（流用するもの・影響するもの）

- **「現在のブロック」の既存の定義**
  - `frontend/src/content-cursor.ts`: キーボードのカーソル。`BLOCK_SELECTOR`（:17-32）のネストした `p`/`li` の親を含む要素列を持つ。
    - 既定は未選択（`currentIndex = -1`、:36-37, :65）で、`ContentNext` などのキー操作をしたときだけ決まる。
    - `getCurrentElement()`（:398）で参照できる。`syncToViewport`（:245）は、スクロールで外れたカーソルを可視域へ寄せる。
  - `frontend/src/reading-position.ts:224-345`: スクロールごとに 1 回（rAF でまとめる）呼ばれる `update()`。上端から `HEADING_MARGIN` にある見出しを `data-current` にし（:302-316）、スクロールインジケータも描く（:238）。本文の高さの変化は `ResizeObserver` で拾う（:207-217）。
  - `frontend/src/scroll-anchor.ts:69`: トップレベルブロック `:scope > [data-source-range]` のキャッシュ。無効化するのは描画のバッチ（`render-coordinator.ts:318`）。
- **chrome の表示状態**
  - `app.rs:278-281`: `left_pinned = sidebar.pinned && chrome().panel`。
  - `app.rs:338-398`: レール、ピン留めしたパネル、覗き見のオーバーレイ。
  - `app.rs:362-366`: `.main-area` 内の `Header` と `Content`。
  - `content.rs:60-62, 75-78, 121-123`: トレースとガター。
  - `sidebar.rs:164-230`: `panel_is_showing` / `show_panel` / `toggle_sidebar`。
  - `ui-design.md` の「What comes back is state, never settings」。幅で畳んでも `pinned` には触れない方針。
- **ヘッダー**: `header.rs:52-172`。検索欄 `HeaderFind` はヘッダーの中にある（:80-82）。
- **per-window のトグルの前例**: `content_full_width`（`app_state.rs:65`, `:232-238`）。PersistedState にも保存されている（`persistence.rs:61`）。
- **キーバインドとメニュー**
  - `Action` 列挙（`crates/arto-keybindings/src/action.rs:9-150`）、文字列表（:567 付近の `action_strings!`）、`ACTION_GROUPS`（:154）、`command_label`（:323）、`COMMAND_ACTIONS`（:398、パレット）、`MENU_ACTIONS`（:445）、個数テスト `all_actions_count`（:636、現在 91）。
  - プリセット `presets/default.json`・`vim.json`・`emacs.json`。
  - 適用: `dispatcher.rs:43` の `dispatch_action`。
  - メニュー: `menu.rs:47-98`（`MenuId` の相互変換）、`:183-208`（`menu_action_for_id`）、`:292-305`（View メニュー）、`:429-470`（state 付きハンドラ）。macOS 以外のメニューは `app_menu.rs:115-133`（View サブメニュー）。
- **Escape**: `keybinding_engine.rs:191-197` → `AppState::dismiss_overlays`（`app_state.rs:329-334`）とカーソルの解除。
- **印刷**: `print.css:17-32` が chrome を `display:none` にし、カーソル強調を消している。
- **全画面**: コード中に fullscreen の扱いはない（grep で 0 件）。macOS 標準の Ctrl+Cmd+F に任せている状態。
- **レンズ**: `lenses.ts:45-59` のクラス（マーカー、翻訳ブロック、ポップオーバー）。ページレンズの HTML には `data-source-range` がない（`arto-markdown/src/lib.rs:280-294`）。

## 仕様（UI・操作・キーバインド・設定）

- **トグル**: アクション `window.toggle_focus_mode`（`Action::WindowToggleFocusMode`、ラベル "Focus Mode"）。`focus.*` は既にパネルのフォーカス移動で使われているので、その名前空間は避ける。
  - 既定キーは default と emacs が `Cmd+Shift+f`、vim が `z f`（いずれも未使用）。
  - View メニュー（macOS）とヘッダーのアプリメニューの View にも項目を出し、パレットからも実行できるようにする。
- **ON のあいだ**
  - レール・パネル（ピン留め中でも）・ヘッダー・目次ルーラー・マージントレースを隠す。スクロールインジケータは残す。
  - 本文の「単位」のうち現在の 1 つ以外を `opacity: var(--focus-dim)`（既定 0.3）にする。
  - 切り替えは `opacity 180ms ease`。`prefers-reduced-motion` のときはトランジションなし。
- **現在の単位の決め方**（上から順に優先）
  1. content-cursor が選択中なら、その要素を含む単位。
  2. それ以外は読線（`.content` の上端から高さの 35%）にかかっている単位。
  - 最後の 1 画面では、`scrollTop` が最大値に近づくにつれて読線を 35% から 100% へ線形に下げ、末尾のブロックにも届くようにする。
  - 先頭（`scrollTop = 0`）では最初の単位。
- **単位の粒度**: `.markdown-body` の直下の子要素。ただしリスト（`ul`/`ol`）は直下の `li` ごとに分ける。表・引用・アラート・コード・図は丸ごと 1 単位。`data-source-range` には頼らない（ページレンズでも動かすため）。
- **OFF にする操作**
  - 同じキー、またはメニュー。
  - Escape: 開いているもの（パレット・目次・検索・カーソル）がなければ集中モードを抜ける。何か開いていれば、従来どおりそれを閉じるだけ。
  - Cmd+B（`toggle_sidebar`）: 集中モードを抜けてからパネルを出す。
- **ほかの機能との関係**
  - **検索**: Cmd+F で開いているあいだは、ヘッダーを一時的に出して薄くするのを止める（ハイライトを薄めないため）。閉じれば元に戻る。
  - **目次**: `contents.toggle`（Cmd+J）は、オーバーレイとして従来どおり開ける。
  - **パレット・コンテキストメニュー・レンズのポップオーバー**: 本文の外に描かれるので影響なし。レンズのマーカーは単位と一緒に薄くなる。
  - **印刷**: `@media print` で薄くするのを無効にする。chrome はもともと隠れている。
- **設定**: 当面は追加しない。薄さは CSS トークン `--focus-dim` だけにする。

## 設計（データ構造、保存先を Config / PersistedState / State / 別ファイルのどれにするかと理由、Rust と frontend の分担）

- **保存先は State**: `AppState.focus_mode: Signal<bool>`。
  - ウィンドウごとの一時的な読み方で、`palette_open` と同じ扱い（`app_state.rs:100-104`）。
  - 起動をまたいで残すかは未決（残すなら `content_full_width` と同じく PersistedState に足すだけで済む）。
  - Config には入れない（ユーザーが既定を選ぶ種類のものではないため）。
- **Rust の責務（chrome）**
  - `AppState::toggle_focus_mode()` と `exit_focus_mode()` を追加する（`sidebar.rs` と同じく `impl AppState` に置く）。
  - `toggle_sidebar` の冒頭で、集中モード中なら抜ける。
  - `app.rs`: `let focused = focus_mode && !search_open` とし、`rail_visible()` と `left_pinned` に `&& !focused` を掛ける。`sidebar.pinned` 自体は書き換えないので、OFF で元どおりに戻る。
  - `.main-area` に `class: if focused { "focus-mode" }` を付ける。
  - `Header` は描いたまま CSS で畳む。`HeaderFind` はヘッダーの中にあり、検索時にはすぐ出す必要があるので、アンマウントはしない。
  - `content.rs`: トレースとガターの条件に `!focused` を足す。`contents_open` のときのオーバーレイは残す。
  - Escape: `keybinding_engine.rs:191` で、`dismiss_overlays` の前に「開いているものがあったか」を返す `AppState::has_overlays()`（新規）を見て、なければ `exit_focus_mode()` を呼ぶ。
  - `use_effect` で `focused` の変化を JS に伝える: `window.Arto.focus.set(bool)`（新 API）。`window.Arto` の初期化待ちは `search_handler.rs:41-50` と同じ流儀にする。
- **frontend の責務（本文の薄さ）**: `frontend/src/focus-mode.ts`（新規）。
  - `setFocusMode(on)`: `.markdown-body` に `data-focus-mode` を付け外しし、`refreshReadingPosition()` を呼ぶ。
  - `collectUnits(body)`: 直下の子と、リストの直下の `li` を集めてキャッシュする。`scroll-anchor.ts` と同じく、`render-coordinator.ts:318` で無効化する。
  - `pickUnit(tops: number[], bottoms: number[], line: number, prev: number): number`: 純関数。読線を含む単位を二分探索で探す。前の単位が読線から ±8px 以内に残っていれば維持して、ちらつきを抑える。
  - `readingLine(scrollTop, maxScroll, viewportH): number`: 純関数（末尾の補間）。
  - `drawFocus(content)`: `reading-position.ts` の `update()` から、`drawScrollIndicator` と並べて呼ぶ（スクロールごとの測定を 1 回にまとめる方針に沿う）。現在の単位に `data-focus-current` を付け、ほかからは外す。
  - カーソルの優先: `contentCursor.getCurrentElement()` を見る。カーソルが動いてもスクロールしないことがあるので、`moveTo`（content-cursor.ts:106）の最後で `refreshReadingPosition()` を呼ぶ。
  - `runtime.ts` の `window.Arto` に `focus: { set }` を追加し、型宣言も足す。
- **CSS**（`frontend/style/components/content/focus-mode.css`、新規）
  ```css
  .markdown-body[data-focus-mode] > :not(ul, ol):not([data-focus-current]),
  .markdown-body[data-focus-mode] > :is(ul, ol) > li:not([data-focus-current]) {
    opacity: var(--focus-dim); transition: opacity 180ms ease; }
  ```
  - `ul`/`ol` 本体は薄くしない（opacity は親子で掛け算になるため）。
  - `.main-area.focus-mode > .header` は高さを 0 にし、`overflow:hidden` と `opacity:0` で畳むトランジション。
  - `variables.css` に `--focus-dim: 0.3` を追加する。`print.css` で打ち消す。

## 実装手順（1 ステップ = 1 コミット目安）

1. `arto-keybindings`: 次をまとめて追加する。
   - `Action::WindowToggleFocusMode`、文字列、`ACTION_GROUPS`（Window）、`command_label`、`COMMAND_ACTIONS`、`MENU_ACTIONS`
   - 3 つのプリセットへのキー追加
   - `all_actions_count` を 92 に更新
2. `arto` state: `focus_mode`、`toggle_focus_mode` / `exit_focus_mode` / `has_overlays`、`toggle_sidebar` との連携、それらの単体テスト。
3. `arto` dispatcher と Escape の連鎖（`dispatcher.rs`、`keybinding_engine.rs`）。
4. chrome を隠す: `app.rs`、`content.rs`、`.main-area` のクラス、ヘッダーを畳む CSS。
5. frontend: `focus-mode.ts`（純関数とテスト）、`runtime.ts` の API、`reading-position.ts` からの呼び出し、`content-cursor.ts` の `moveTo` 後の再計算、CSS と印刷の打ち消し。
6. Rust→JS の同期（`use_effect`）と、検索中は一時停止する処理。
7. メニュー: `MenuId::ToggleFocusMode`（`"view.toggle_focus_mode"`）を `menu.rs` の各所に追加し、`app_menu.rs` の View にも追加する。アイコンは `add-icon` スキルで Tabler `focus-2` などを追加する。
8. `just fmt check test` を通す。

## テスト方針

- **Rust**
  - `toggle_focus_mode` の往復で `sidebar.pinned` が変わらない
  - 集中モード中の `toggle_sidebar` で、集中モードが OFF になりパネルが出る
  - `has_overlays` の真偽
  - `menu.rs` の roundtrip テスト（:490）と `menu_actions_cover_all_menu_items`（:533）に新項目を追加
  - Action の文字列 roundtrip と個数
  - プリセットの検証（`presets.rs:54`）
- **frontend**（vitest + happy-dom、既存の `reading-position.test.ts` の流儀）
  - `pickUnit`: 境界、ヒステリシス、空配列
  - `readingLine`: 先頭、途中、末尾の補間
  - `collectUnits`: リストが `li` に分かれる、空要素を除外、ページレンズの HTML（range なし）でも動く
- 見た目（トランジションやヘッダーの畳み方）はユーザーが確認する（アプリは起動しない）。

## 規模感（S/M/L）とリスク

**規模: M**。Rust の chrome 制御は小さい。frontend の単位判定と、既存機能との干渉の整理が主な作業。

リスク:
- **ヘッダーを畳むと `.content` の高さが変わる**: スクロール位置が 1 行分跳ぶ。切り替えの前後で `scroll.anchor()` / `toAnchor` を使い、位置を保つ必要がある。
- **Dioxus の再描画でも付けた属性が残るか**: `.markdown-body` の中身は `dangerous_inner_html` で差し替えられるので、属性は描画完了のコールバック（`runtime.ts:268-272`）で付け直す。
- **Mermaid の遅延描画など、単位の高さが後から変わる**: `ResizeObserver` 経由の `update()` で追従できる見込み。
- **ピン留めパネルを隠したときの幅の再計算**: `visible_chrome()` は `pinned` を見て幅を見積もるので、集中モード中にトレースの予算が合わなくなる。ただしトレース自体を隠すので実害はない。

## 未決事項（ユーザー判断が必要な点）

1. **現在ブロックの基準**: 読線（画面の 35%、推奨）か、上端か、中央か。キーボードのカーソルを優先してよいか。
2. **単位の粒度**: リストを `li` ごとに分ける（推奨）か、リスト全体を 1 単位にするか。長い段落を文単位まで細かくするか（推奨はしない）。
3. **ヘッダーの復帰**: 画面上端へのホバーでヘッダーを一時的に出すか（macOS の全画面風）、キー操作でしか戻らないか。
4. **永続化**: ウィンドウを閉じても集中モードを覚えておくか（PersistedState）。
5. **全画面との連動**: 集中モードで OS の全画面にも入るか（コード中に fullscreen の扱いは現在ない）。
6. **既定キー**: `Cmd+Shift+f` / vim の `z f` でよいか。薄さ（0.3）を Config に出すか。
7. ポインタを乗せた薄いブロックを一時的に濃くするか。
