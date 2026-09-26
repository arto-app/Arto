# 長い表の見出し行を固定（Sticky table headers）

## 目的

行数の多い表を読み進めると見出し行が画面外に消え、各列が何だったかを見失う。表が画面内にある間は見出し行を本文スクロール領域の上端に固定表示し、横に長い（横スクロールする）表でも列と見出しがずれないようにする。アプリと `arto page` の両方で同じ挙動にする。

## 既存の仕組み（流用するもの・影響するもの）

- 表の HTML: GFM 表は `<table data-source-range> <thead><tr><th>…` を出す（`crates/arto-markdown/tests/snapshots/samples__samples_render_as_before@02-blocks.md.snap` の `296:1-299:59`）。raw HTML の表（`samples/02-blocks.md:413-416`）は `thead` が無いことがある。frontmatter は `table.frontmatter-table`（`frontend/style/components/content/frontmatter.css:32-43`、`display: table !important`）。
- Primer の `.markdown-body table { display: block; width: max-content; max-width: 100%; overflow: auto }`（`frontend/style/main.css:7` で import、値は @primer/css 22 の記憶ベースで要確認）。**表自身が両軸のスクロールコンテナ**になるため、`th { position: sticky }` は縦に一切スクロールしない表の中で効かない。`overflow-x: auto` だけにしても `overflow-y` は `auto` に計算されるので同じ。横長表の例 `samples/02-blocks.md:316-320`。
- スクロールコンテナ: アプリは `.content`（`frontend/style/components/content.css:8-14`、`overflow-y: auto`）、`arto page` は文書自体（`crates/arto-page/src/lib.rs:91`, `STANDALONE_OVERRIDE_CSS`）。両者の差は `frontend/src/scroll-destination.ts:33-38`（`scrollContainer()`）が吸収している。
- ヘッダー帯と検索欄: ヘッダーは `.content` の外の独立行（`frontend/style/components/header.css:8-19`）、検索欄もその行の中（`crates/arto/src/components/find.rs:1-11`）。**本文に重なるオーバーレイは無い**ので固定位置は `top: 0` でよい。
- ズーム: `.content > div` に `zoom`（`crates/arto/src/components/content.rs:44-49,83-86`）。印刷時は `frontend/style/print.css:56-58` で 1 に戻す。
- 印刷: `print.css:139-150` で表を `display: table` に戻し、`tr { break-inside: avoid }`。
- 表を扱う frontend: `frontend/src/table-utils.ts`（CSV/TSV/Markdown 抽出）、`frontend/src/context-menu-handler.ts:350-360`（`target.closest("table")`）、`frontend/src/content-cursor.ts:17-32,418-424`（`table` をブロックとして扱う、`toElement(el, "nearest")` は `:102`）。
- DOM を走査する既存処理: 検索 `frontend/src/find-in-page.ts:91,276,301`（`.markdown-body` 内のテキストに `<mark>` を挿入）、スクロールアンカー `frontend/src/scroll-anchor.ts:70`（`.markdown-body > [data-source-range]` の直下ブロック列）。→ **表を別要素で包む（DOM 構造を変える）と scroll-anchor が壊れる**。
- 描画後フック: `frontend/src/runtime.ts:246-266`（`renderCoordinator.onRenderComplete` を自己再登録するパターン）、`frontend/src/render-coordinator.ts`。
- 余白計測の ResizeObserver パターン: `frontend/src/reading-position.ts:215-222`。

## 仕様（UI・操作・キーバインド・設定）

- 対象: `.markdown-body` 内の `table` のうち `thead` を持つもの。`.frontmatter-table` / `.yaml-nested-table` は対象外。
- 挙動: 表の上端がスクロール領域上端を越え、かつ表の下端がまだ見出し行の高さ以上残っている間だけ、見出し行を上端に固定表示する。表を過ぎれば自然に消える（次の表に引き継ぐ）。
- 横スクロールする表: 固定された見出しも表の横スクロールに追従し、列位置が一致する。
- 閾値: 表の高さがスクロール領域の高さを超える表だけを対象にする（短い表で見出しが一瞬ずれて見える違和感を避ける）。ウィンドウリサイズ・ズーム・再描画で再判定。
- 見た目: 固定中の見出しは不透明背景（`--bgColor-default`）＋下辺に 1px の境界線（`border-collapse` のため `th` の border は固定されないので `box-shadow: inset 0 -1px …` で描く）。
- 操作: 固定見出しのリンクはクリック可能。右クリックは元の表に対する操作（CSV コピー等）として扱う。
- 設定・キーバインド: 当面なし（常時有効）。要望次第で `markdown` 以外の新セクションに on/off を追加（未決）。
- `arto page`: 同じ挙動（文書スクロールに対して固定）。スクリプトが動かない環境（CSP 違反・Quick Look で JS 無効など）では従来通り固定しない。
- 印刷: 固定表示はしない。紙面での見出し行繰り返しは `thead { display: table-header-group }` に任せる（WebKit の対応状況は要確認）。

## 設計（データ構造、保存先を Config / PersistedState / State / 別ファイルのどれにするかと理由、Rust と frontend の分担）

- 保存先: **なし**（設定を持たない純表示機能）。on/off を設けるなら Config（全ウィンドウ共通・`arto page` も読む）。
- 技法: **ハイブリッド**。
  1. **横に収まる表 → 純 CSS sticky**。JS が表を分類し、`scrollWidth <= clientWidth` かつ高さ閾値超の表に `data-sticky-head` を付ける。CSS は `.markdown-body table[data-sticky-head] { overflow: visible }` と `… thead th { position: sticky; top: 0; z-index: 1; background: var(--bgColor-default); box-shadow: inset 0 -1px var(--borderColor-default) }`。表がスクロールコンテナでなくなるので sticky は `.content`（アプリ）／ビューポート（page）に対して効き、ズームも CSS が自然に扱う。スクロール同期コード不要。
  2. **横にはみ出す表 → JS の浮動見出し（オーバーレイ）**。表はスクロールコンテナのままにし、`thead` の複製を表外のオーバーレイに描く。
- オーバーレイ要素（新規）: `.markdown-viewer` の先頭子として `div.table-head-overlay`（`aria-hidden="true"`, `inert` ではなくクリックは受ける）を置く。`position: sticky; top: 0; height: 0; z-index` にすると、`.markdown-viewer` が文書全体の高さを持つためアプリでも page でも上端に張り付き、しかも zoom ラッパーの内側なので寸法換算が不要。内側の `div`（`position: absolute; overflow: hidden`）の `left`/`width` を対象表の `offsetLeft`/`clientWidth`（同じズーム空間のレイアウト座標）に合わせ、中に `<table><thead>` の複製を置いて各 `th` 幅を元表の実測値で固定。表の `scroll` イベントで `inner.scrollLeft = table.scrollLeft`。
  - `.markdown-body` の外に置くので、検索・スクロールアンカー・コンテンツカーソル・レンズの走査対象にならない。複製時に `<mark>`（検索ハイライト）と `data-source-range` は除去。
  - 表示判定は `scrollContainer()` の `scroll`（passive＋rAF）で、対象表の `getBoundingClientRect()` とコンテナ上端から計算。判定ロジックは純関数 `headOverlayState(tableRect, headHeight, viewportTop): "hidden" | "shown"` として切り出す。
  - 右クリック: `context-menu-handler.ts:354` の `closest("table")` の前に「オーバーレイ内なら元の表に差し替える」処理を追加（オーバーレイが保持する元表参照を使用）。
- DOM 所有: アプリでは `FileViewer`（`crates/arto/src/components/content/file_viewer.rs:79-86`）の rsx にオーバーレイ `div` を追加（Dioxus 管理下の要素に JS が兄弟を差し込むと差分がずれる恐れがあるため Rust 側で出す）。`arto page` は `build_document`（`crates/arto-page/src/lib.rs:553`）のテンプレートに同じ `div` を追加。中身は JS のみが触る。
- frontend 新規モジュール `frontend/src/sticky-table-head.ts`: `setup()`（runtime `init` から呼ぶ）、`refresh()`（`onRenderComplete` と ResizeObserver から）、分類関数 `classifyTable(table, viewportHeight)` を export。`frontend/style/components/content/sticky-table-head.css` を新設し `content.css` から import。
- Rust 側の変更はオーバーレイ要素の追加のみ。Markdown の HTML 契約（`arto-markdown`）は変更しない。
- 印刷: `print.css` に `.table-head-overlay { display: none !important }` と `.markdown-body th { position: static !important; box-shadow: none }` を追加。

## 実装手順（1 ステップ = 1 コミット目安）

1. frontend: `sticky-table-head.ts` の分類（`data-sticky-head` 付与）と CSS sticky、`runtime.ts` への登録、印刷 CSS。横に収まる長い表で固定されることを確認。
2. Rust: `FileViewer` と `arto-page` テンプレートにオーバーレイ `div` を追加（空要素、見た目に影響なし）。`arto-page` のテンプレートテスト更新。
3. frontend: 横長表用オーバーレイ（複製・列幅同期・横スクロール同期・表示判定）。
4. frontend: 右クリック・リンククリックの元表への委譲、検索ハイライトの除去、再描画／テーマ変更時の作り直し。
5. サンプル: `samples/02-blocks.md` に「縦に長い表」「縦にも横にも長い表」を追加し、スナップショットを更新（意図した差分のみ受け入れ）。

## テスト方針

- vitest（happy-dom はレイアウトを計算しないため、幾何は引数で渡す純関数に寄せる）: `classifyTable` の判定（幅超過／高さ閾値／`thead` 無し／frontmatter 除外）、`headOverlayState` の境界（表上端・下端ちょうど、見出し高さ分の余白）、複製から `mark`・`data-source-range` が除かれること、列幅配列の適用。
- context-menu のテスト（`context-menu-handler.test.ts`）にオーバーレイ内右クリックが元表の CSV を返すケースを追加。
- Rust: `arto-page` の `test_build_document_wraps_body_and_embeds_assets` を更新。`arto-markdown` のスナップショットはサンプル追加分のみ差分。
- 手動（ユーザーに依頼）: アプリでズーム 0.5–2.0、full-width 切替、テーマ切替、印刷プレビュー、`arto page` 出力をブラウザで確認。
- `just fmt check test` を通す。

## 規模感（S/M/L）とリスク

**M**（CSS のみの段階 1 は S。オーバーレイ込みで frontend 約 300 行＋テスト）。

- WebKit で「zoom をかけた祖先の中の sticky」が正しく配置されるかは実機確認が必要（ズーム時のずれ）。
- `overflow: visible` に切り替えた表は、フォントロードや画像読み込みで後から横にはみ出す可能性 → ResizeObserver で再分類し CSS/オーバーレイを切り替える。切替時のちらつき。
- オーバーレイの列幅は元表と別レイアウトなので、`colspan`・セル内改行・画像入りセルでずれうる（実測幅を `th` ごとに固定して緩和）。
- 多数の長い表がある文書でのスクロールコスト（対象表を IntersectionObserver で絞り、scroll ハンドラは「現在表示中の表」だけを見る）。
- 印刷で `thead` 繰り返しが WebKit で効かない場合は現状と同じ（退行ではない）。

## 未決事項（ユーザー判断が必要な点）

1. 閾値: 「スクロール領域より高い表のみ」でよいか、全ての `thead` 付き表を対象にするか（行数基準にするか）。
2. 横長表への対応範囲: 段階 1（横に収まる表だけ CSS で固定）で一旦リリースし、オーバーレイは後回しでよいか。
3. 設定で無効化できるようにするか（する場合は Config の新セクション名）。
4. `thead` の無い raw HTML 表で「先頭行が全て `th`」のものも対象にするか。
5. 表の左端列（行見出し）の横方向固定も将来やるか（今回は範囲外の想定）。
