# 読了時間・残り時間（Reading time）

## 目的

ヘッダー行に「あとどれくらいで読み終わるか」を控えめに出す。開いた直後は文書全体の所要時間（`12 min read`）、読み進めたら残り時間（`8 min left`）。
日本語は文字数、英語は語数で数え、コードブロックとフロントマターは本文と同じ速さでは数えない。
「残り」はスクロールのピクセル量ではなく、残っている本文の量で決める（図やコードが長い文書で比率が大きくずれるのを防ぐ）。

## 既存の仕組み（流用するもの・影響するもの）

- **ヘッダー行**: `crates/arto/src/components/header.rs:52-163`。左は Menu グリフ・`Breadcrumb`（:87）・ファイル操作ボタン、検索中は `HeaderFind` が Breadcrumb と入れ替わる（:80-88）。右は `LensControls`・検索・全幅トグル・テーマ（:133-163）で、`reading = !document.is_empty()` のときだけ描かれる（:30, :136）。
  - 補足: `components/document_name.rs` はパネルやパレットの**行**で使う名前表示で、ヘッダーでは使っていない。ヘッダーの名前は `header/breadcrumb_menu.rs` の `Breadcrumb`。
- **UI の言語は英語**: コマンド名 `crates/arto-keybindings/src/action.rs:323-386`（"Find in Page" など）、ヘッダーの title `header.rs:102,125,143`。i18n の仕組み（fluent など）はない。
- **現在位置はすでに Rust にある**: `content.rs:144-205` の `use_scroll_anchor_tracker` が、スクロールのたびに 1 フレームへまとめて `ScrollAnchor { line, fraction }` を `dioxus.send` し、`state.current_scroll_anchor` に入れている（:173-201、`app_state.rs:127-129`）。
  - `line` は、ビュー上端にある**トップレベル**ブロック（`frontend/src/scroll-anchor.ts:69` の `:scope > [data-source-range]`）が始まるソース行、`fraction` はそのブロックの中の位置（`scroll_anchor.rs:16-24`）。
  - つまり「どのブロックのどこまで来たか」がソース行で分かるので、新しい JS→Rust チャネルは要らない。
- **描画の流れ**: `file_viewer.rs:115-121` が `render_to_html_with_toc`（`crates/arto/src/markdown.rs:39-47`）を呼び、`state.headings` と `state.rendered_source` をセットする。
  - `arto_markdown::RenderResult`（`crates/arto-markdown/src/lib.rs:247-257`）は `html / headings / images`。見出しは engine が AST から集めている（`engine.rs:112-130`、`engine/outline.rs:26`）。
- **パイプラインの制約**: `.claude/rules/markdown-pipeline.md` にあるとおり、パーサ型を名指ししてよいのは `src/engine/` だけ。フロントマターは engine より前に切り出され、行オフセットを返す（`frontmatter.rs:12`）。
- **ページレンズ**: 翻訳など文書全体を差し替えるレンズは `render_detached` で描かれ、source range を持たない（`lib.rs:280-294`）。適用中かどうかは `LensRun.applied`（`lenses.rs:143`）で分かる。
- **設定と Preferences**: `Config`（`crates/arto-config/src/lib.rs:59-76`）。Preferences には Reading タブがある（`preferences_view/tabs/reading_tab.rs:100` の "Margin Trace" 節）。
- **スクロールを描く既存コード**（`reading-position.ts:54`、`scroll-indicator.ts:54`）はピクセル基準なので、今回は流用しない。

## 仕様（UI・操作・キーバインド・設定）

- **置き場所**: `header-right` の先頭（`LensControls` の左）。ボタンではなくテキストだけ。
  - `--opacity-rest`、`.header:hover` で `--opacity-woken`（`ui-design.md` の Quiet controls）。小さめの字、`font-variant-numeric: tabular-nums`。
- **表示の段階**
  - 先頭（`anchor.is_top()`）: `12 min read`
  - 途中: `8 min left`
  - 残り 1 分未満: `<1 min left`
  - 最後まで来たら（`scrolled_to_end`）: 表示しない
- **表示しない場合**
  - 文書全体が `reading.minMinutes`（既定 3 分）未満
  - Markdown 以外（`rendered_source == None`）
  - ページレンズを適用中
  - Welcome ページ
  - `reading.showTime = false`
- **ツールチップ（title）**: `12 min total · 3,400 words · 5,200 characters`
- **数え方**（ブロックごと）
  - 本文: CJK 文字数 ÷ `charactersPerMinute`（既定 500）+ 英語などの語数 ÷ `wordsPerMinute`（既定 230）。CJK は Han・Hiragana・Katakana・Hangul・全角英数。句読点と記号は数えない。`Rustの所有権` は 1 語 + 4 文字。
  - コードブロック: 1 行 2 秒（固定の定数）。Mermaid と数式ブロックは 1 つ 10 秒、画像は 1 枚 10 秒。
  - 数えないもの: フロントマター、リンクの URL、HTML タグ。表はセルのテキストを本文として数える。
- **キーバインド**: なし（コマンドではなく表示だけ）。
- **設定**（`config.json` の新しい `reading` 節）
  ```json
  "reading": { "showTime": true, "wordsPerMinute": 230, "charactersPerMinute": 500, "minMinutes": 3 }
  ```
  Preferences の Reading タブに "Reading Time" 節を追加する（トグル 1 つとスライダー 3 本。既存の Slider 規約に従う）。

## 設計（データ構造、保存先を Config / PersistedState / State / 別ファイルのどれにするかと理由、Rust と frontend の分担）

- **数えるのは Rust の描画時**（arto-markdown の engine）。frontend の DOM で数えない理由は次のとおり。
  1. ヘッダーは Rust の RSX なので、DOM で数えると数値を Rust へ送る新しいチャネルが要る。
  2. DOM には Mermaid の SVG 文字・KaTeX・レンズのポップオーバー・コピーボタンが混ざり、除外のセレクタが脆い。
  3. 位置（`ScrollAnchor.line`）はすでにソース行で Rust に届いている。
  4. 将来 `arto page` や Quick Look でも使える。

  DOM を読まずに済むので、フォントやズームにも左右されない。

- **新しい型**（`crates/arto-markdown/src/reading.rs`、パーサ非依存）

  ```rust
  pub struct ReadingBlock { pub line: u32, pub cjk_chars: u32, pub words: u32,
                            pub code_lines: u32, pub figures: u32 }
  pub struct ReadingProfile { pub blocks: Vec<ReadingBlock> }   // line 昇順
  pub fn count_text(text: &str) -> (u32 /*cjk*/, u32 /*words*/); // 純関数
  ```

  - AST を歩いてトップレベルブロックごとに集計する部分は `src/engine/reading.rs`（`outline.rs` と同じ流儀）に置く。
  - `line` はフロントマターのオフセットを足したファイル行で、`data-source-range` の開始行と一致させる。
  - `RenderResult` に `pub reading: ReadingProfile` を足す。HTML は変わらないので、スナップショットの差分は出ない。

- **秒への換算はアプリ側**（`crates/arto/src/reading_time.rs`、新規）

  ```rust
  pub struct Speeds { wpm: u32, cpm: u32 }
  pub fn total_seconds(p: &ReadingProfile, s: Speeds) -> f64
  pub fn remaining_seconds(p: &ReadingProfile, anchor: ScrollAnchor, s: Speeds) -> f64
  pub fn label(total: f64, remaining: f64, at_top: bool, at_end: bool, min_minutes: u32) -> Option<String>
  ```

  - 残り時間 = `anchor.line` より後のブロックの合計 + 今いるブロック × (1 − `fraction`)。
  - ブロックの特定は二分探索で行う。速度の設定を変えても再描画は要らない（プロファイルに入るのは件数だけ）。

- **末尾の判定**: ビュー上端のアンカーは、最後の 1 画面より先に進まない。そこで `content.rs:173-189` の JS が `{ line, fraction, atEnd }` を送るようにする。
  - `atEnd` は `scrollTop + clientHeight >= scrollHeight - 2`。
  - Rust 側は `content.rs` の中に `#[serde(flatten)] anchor` を持つ `ScrollReport` を置いて受ける。`ScrollAnchor` そのものは変えない（履歴に保存される型なので）。

- **保存先**
  - 速度・表示の有無・しきい値 → **Config**（ユーザーの好みで、全ウィンドウに共通）。
  - プロファイル・末尾フラグ → **State**。`AppState` に `reading_profile: Signal<Option<Arc<ReadingProfile>>>` と `scrolled_to_end: Signal<bool>` を追加し、`file_viewer.rs:118` で headings と同時にセット・クリアする。
  - PersistedState と別ファイルは使わない（文書から毎回計算できる値なので）。

- **描画コンポーネント**: `components/reading_time.rs`（新規）の `ReadingTime {}`。
  - `current_scroll_anchor` を読むのはこのコンポーネントだけにして、`Header` 全体が毎フレーム再描画されないようにする。`use_memo` で出力する文字列を作り、分の値が変わったときだけ DOM が変わる。
  - `config_revision` を読んで、設定の変更に追従する（`layout.rs:16` と同じ流儀）。

- **frontend の変更**: `content.rs` の埋め込み JS に `atEnd` を足すだけ。CSS は `frontend/style/components/header/` に `reading-time.css` を追加する。

## 実装手順（1 ステップ = 1 コミット目安）

1. `arto-markdown`: `reading.rs`（`count_text` と型）と単体テストを追加する。
2. `arto-markdown`: `engine/reading.rs` で AST からトップレベルブロックごとに集計し、`RenderResult.reading` を追加する。`tests/pipeline.rs` にテストを足す。
3. `arto-config`: `ReadingConfig`（`reading_config.rs`）を `Config.reading` として追加し、`ARTO_UPDATE_SCHEMA=1 cargo test -p arto-config` でスキーマを再生成する。
4. `arto`: `markdown.rs:39` の戻り値をタプルから構造体（`Rendered { html, headings, reading }`）に変え、`AppState.reading_profile` と `file_viewer.rs` でのセット・クリアを実装する。
5. `arto`: `reading_time.rs`（`remaining_seconds` と `label` の純関数）とテストを追加する。
6. `arto`: `content.rs` のスクロール報告に `atEnd` を足し、`AppState.scrolled_to_end` を追加する。
7. `arto`: `ReadingTime` コンポーネント、`header.rs` への配置、CSS を追加する。
8. Preferences の Reading タブに "Reading Time" 節を追加する。
9. `just fmt check test` を通す。

## テスト方針

- **`count_text`**（indoc を使う）
  - 日本語だけ、英語だけ、混在
  - 全角英数、絵文字、句読点の除外
  - 空文字
- **pipeline テスト**
  - フロントマターを含まない。`line` がフロントマター分ずれて `data-source-range` の開始行と一致する。
  - コードブロックが `code_lines` に入る
  - リストや引用はトップレベルの 1 ブロックとして合算される
- **`remaining_seconds`**
  - TOP なら total、最後のブロックの途中なら端数
  - 存在しない `line`（アンカーが古い）でも panic しない
- **`label`**
  - しきい値未満なら `None`、先頭・途中・`<1 min`・`at_end` の各段階
- **Config**
  - roundtrip、部分 JSON で既定値、スキーマ一致テスト
- **SSR**
  - `ReadingTime` が `None` のとき何も描かない
- UI の見た目はユーザーが確認する（アプリは起動しない）。

## 規模感（S/M/L）とリスク

**規模: M**（3 クレートにまたがるが、それぞれは小さい）。

リスク:
- ox-content の AST でトップレベルブロックの開始行を、HTML の `data-source-range` とずれなく取れるか。engine の annotate が使うのと同じ span から取れば一致するはずだが、footnote 定義は描画順とソース順が違う（`outline.rs:19-25`）。
- `render_to_html_with_toc` の戻り値の型を変えると、`arto-page` など呼び出し元すべてに波及する（`RenderResult` にフィールドを足すだけなら後方互換）。
- スクロールのたびに ReadingTime のメモが再計算される（O(log n) なので問題はない見込み）。

## 未決事項（ユーザー判断が必要な点）

1. **表示言語**: UI は英語で統一されている。依頼文の「残り約 8 分」のような日本語表示にするか。する場合、i18n 基盤がないので、この 1 箇所だけ `navigator.language` を見て出し分けるか。**推奨は英語**。
2. **置き場所**: ヘッダー右端の先頭（推奨）か、Breadcrumb の右隣か。後者は検索中に消え、狭い幅では Breadcrumb の切り詰めと競合する。
3. **コードと図の重み**（2 秒/行、10 秒/個）を設定に出すか、固定の定数にするか。
4. **しきい値の既定**（3 分）と、最後まで読んだときの扱い（何も出さない / `Done`）。
5. 残り時間を「ビュー上端」から数えるか、「画面の 1/3 あたりの読線」から数えるか。後者は JS 側でアンカーを別途取る必要があり、規模が増える。
