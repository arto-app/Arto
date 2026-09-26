# 本文幅・行間・書体の調整（Reading measure / line height / typeface）

## 目的

現状、本文の見た目で読者が触れるのは「ペインいっぱいに広げる」トグルとズームだけで、行長（measure）・行間・書体は Primer の Markdown スタイルシートに固定されている。日本語文書（全角主体）と英語文書（半角主体）では適切な行長も行間も違うため、`config.json` と Preferences から「読みやすさ」の 3 要素（行長・行間・書体）と基準文字サイズを調整できるようにする。アプリ・`arto page`・Quick Look で同じ見た目になることも目的に含める。

## 既存の仕組み（流用するもの・影響するもの）

- 本文の最大幅は CSS 固定: `frontend/style/components/content/markdown-viewer.css:11-13`（`.markdown-body { max-width: 960px }`）、full-width 時の解除は同 `:7-9`（`&.full-width .markdown-body { max-width: none }`）。
- 書体・文字サイズ・行間は Primer `@primer/css/dist/markdown.css`（`frontend/style/main.css:7` で import）の `.markdown-body { font-family: -apple-system,…; font-size: 16px; line-height: 1.5 }` が決めている（node_modules 未展開のため値は @primer/css 22 の記憶ベース。実装時に要確認）。見出し・`code`・`pre` は em / % 指定なので基準サイズに追従する。
- アプリ全体の `body` 書体は `frontend/style/main.css:56-59`（`"Segoe UI", Tahoma, …`）。本文には効いていない（Primer が上書き）。
- 等幅スタックは `frontend/style/variables.css:56-58`（`--font-mono`）、UI 文字サイズは `:21-26`（`--font-size-base: 14px` など）。Mermaid は `markdown-viewer.css:110-116` で `--font-size-base` を使うので本文サイズと切り離されている（変更しない）。
- full-width トグル: `crates/arto/src/state/app_state.rs:64-65`（`content_full_width: Signal<bool>`）、`:230-238`（`toggle_content_full_width`）、適用は `crates/arto/src/components/content/file_viewer.rs:79-81`、ボタンは `crates/arto/src/components/header.rs:151-156`、保存は `crates/arto/src/state/persistence.rs:61`、新規ウィンドウ解決は `crates/arto/src/window/settings.rs:338-347`、トレース表示判定 `crates/arto/src/state/app_state/layout.rs:52`。
- ズーム: `crates/arto/src/components/content.rs:44-49,83-86`（`.content > div` に `zoom:` を付与）。ズーム後の余白再計測は `content.rs:135-140`。`frontend/src/reading-position.ts:215-257` は `.markdown-body` を ResizeObserver で監視しているため、行長変更による幅変化も自動で再計測される。
- 設定セクションの型: `crates/arto-config/src/lib.rs:53-78`（`Config`、`#[serde(rename_all = "camelCase", default)]`）、1 セクション 1 ファイルの例 `crates/arto-config/src/zoom_config.rs`。スキーマ再生成は `ARTO_UPDATE_SCHEMA=1 cargo test -p arto-config`（`.claude/rules/config-module.md`）。
- 設定変更の伝播: Preferences は即時自動保存（`crates/arto/src/components/content/preferences_view/main_view.rs:84,98-111,121-147`、200ms settle 後 `CONFIG_CHANGED_BROADCAST`）、各ウィンドウは `crates/arto/src/components/app/listeners.rs:14-22` で `config_revision` を進める（`app_state.rs:78`、利用例 `layout.rs:16`）。
- Preferences の「Reading」タブ: `crates/arto/src/components/content/preferences_view/tabs/reading_tab.rs`（`SliderInput` / `OptionCards` を使用、`form_controls.rs` に `ChoiceRow`, `ToggleRow` もある）。
- `arto page` / Quick Look: `crates/arto-page/src/lib.rs:323-363`（`PageOptions` と `from_config`）、`:478-560`（`build_document`、`<div class="markdown-viewer"><article class="markdown-body">`）、Quick Look は `crates/arto-page/src/ffi.rs:38`、CLI は `crates/arto-page/src/cli.rs:71-72` で `Config::load_preferences()` を読む。
- 印刷: `frontend/style/print.css:56-67`（ズームを 1 に戻し `max-width: none`）。

## 仕様（UI・操作・キーバインド・設定）

`config.json` に新セクション `typography`（Rust 名 `TypographyConfig`）を追加。既定値は現在の見た目と完全一致させる。

```json
"typography": {
  "measure": 60,          // 行長、em 単位（既定 60em = 16px 時 960px）。範囲 30–100
  "lineHeight": 1.5,      // 単位なし。範囲 1.2–2.2、0.05 刻み
  "fontFamily": "system", // "system" | "sans" | "serif" | "mono" | "custom"
  "customFontFamily": "", // fontFamily = custom のときの CSS font-family 値
  "fontSize": 16          // 本文基準 px。範囲 12–24
}
```

- 行長は `ch` ではなく `em` で持つ。`ch` は「0」の幅なので全角文字は約 2ch になり、日本語で行長が倍ぶれする。`em` なら「全角 N 文字 ≒ N em、半角は約 2N 字」と説明でき、和欧どちらでも予測しやすい。UI には目安として「全角 約 N 字 / 半角 約 2N 字」を併記。
- 書体プリセットは CJK フォールバック込みの固定スタック（例 serif: `"Iowan Old Style", "Hiragino Mincho ProN", "Yu Mincho", "Noto Serif CJK JP", serif`、sans: Primer 既定 + `"Hiragino Sans", "Noto Sans CJK JP"`、mono: `--fontStack-monospace` + `"Osaka-Mono"` 等）。`system` は Primer のスタックをそのまま使う（＝現状）。
- Preferences › Reading タブに「Typography」節を追加:
  - Line length: `SliderInput`（em）＋プリセット 3 つ（Narrow 40 / Standard 50 / Wide 60）を `ChoiceRow` で。
  - Line height: `SliderInput`。
  - Typeface: `OptionCards`（System / Sans / Serif / Mono / Custom）、Custom 選択時のみテキスト入力。
  - Base size: `SliderInput`（px）。説明文で「ズームはページ全体（画像含む）、これは文字だけ」と区別。
  - 各項目に既存の `shipped` 既定値リセット（`ResetLine`）を付ける。
- ライブプレビュー: 自動保存→ブロードキャストで開いている全ウィンドウに 200ms 程度で反映されるので、実文書そのものがプレビューになる。加えてタブ内に和欧混植の短いサンプル段落（同じ CSS 変数を適用）を置き、文書を開いていなくても確認できるようにする。
- full-width トグルとの関係: トグルは「このウィンドウでは行長設定を無視してペイン幅いっぱい」という per-window の上書きとして残す（`max-width: none` が `--reading-measure` に勝つ）。ツールチップを "Ignore line length" 相当に変えるかは未決。
- キーバインド: 新設しない（行長の一時変更は full-width トグル、サイズの一時変更はズームで足りる）。
- `arto page` / Quick Look / 印刷: 書体・行間・基準サイズは全て反映。行長は `arto page`・Quick Look では反映（上限なので狭いペインでは実質無効）、印刷では従来通り `max-width: none`（用紙幅優先）。

## 設計（データ構造、保存先を Config / PersistedState / State / 別ファイルのどれにするかと理由、Rust と frontend の分担）

- 保存先は **Config のみ**。全ウィンドウ共通の嗜好で、`arto page` / Quick Look も読む必要があるため `arto-config` に置く（`arto-page` は `arto-config` だけに依存できる）。per-window で変える需要は full-width トグルとズームが既に担っており、PersistedState / State は増やさない（決定表 `.claude/rules/architecture-overview.md` の「全ウィンドウ共通・ユーザー編集」に該当）。
- 新規ファイル `crates/arto-config/src/typography_config.rs`（新規 API）:
  - `pub struct TypographyConfig { measure: f64, line_height: f64, font_family: FontFamilyChoice, custom_font_family: String, font_size: f64 }`（`JsonSchema`, `serde(rename_all = "camelCase", default)`）。
  - `pub enum FontFamilyChoice { System, Sans, Serif, Mono, Custom }`（`snake_case`）。
  - 範囲定数 `MIN_MEASURE` などと `normalize()`（非有限値・範囲外をクランプ、`zoom_config.rs` の `normalize_zoom` と同じ流儀）。
  - `pub fn css_declarations(&self) -> String`: `--reading-measure: 60em; --reading-line-height: 1.5; --reading-font-size: 16px; --reading-font-family: …;` を返す純関数。`system` の場合は `--reading-font-family` を出さない（CSS 側のフォールバック＝Primer 既定が効く）。
  - `custom_font_family` は `[A-Za-z0-9 _,"'\-\p{L}]` 以外（特に `; { } < > \`）を含むと無効扱いで `system` にフォールバック。`arto page` では `<style>` に埋め込むため `</style>` 注入を防ぐ必要がある。
- `Config` に `pub typography: TypographyConfig` を追加（`lib.rs:53-78`）。
- frontend CSS（`markdown-viewer.css`）: `.markdown-body { max-width: var(--reading-measure, 960px); font-size: var(--reading-font-size, 16px); line-height: var(--reading-line-height, 1.5); }` と `font-family: var(--reading-font-family, <Primer と同じスタック>)`。変数が無い環境でも現状と同一になるようフォールバック値を持たせる。`pre`/`code` は Primer の `--fontStack-monospace` のままにして本文書体に巻き込まない。CJK 向けに `line-break: strict; word-break: normal;` は常時適用を検討（未決）。
- Rust（アプリ）: `FileViewer`（`file_viewer.rs:79`）の `.markdown-viewer` に `style: "{typography_css}"` を付ける。`use_memo` で `state.config_revision` を読んでから `CONFIG.read().typography.css_declarations()` を計算し、設定変更時だけ再描画。`dangerous_inner_html` の article 自体は再描画されないので本文 DOM とスクロール位置は保たれる。
- Rust（`arto-page`）: `PageOptions` に `typography: TypographyConfig` を追加し `from_config` で詰める。`build_document` の `.markdown-viewer` に `style="…"` を出力（`STANDALONE_OVERRIDE_CSS` とは別）。CLI フラグは追加しない。
- 印刷: `print.css` は `max-width: none` を維持。他の変数は継承されるのでそのまま反映。
- Preferences のサンプル段落は `reading_tab.rs` 内で同じ `css_declarations()` を `style` に渡す。

## 実装手順（1 ステップ = 1 コミット目安）

1. `arto-config`: `typography_config.rs`（型・既定値・normalize・`css_declarations`・サニタイズ）と `Config.typography` 追加、スキーマ再生成、単体テスト。
2. frontend CSS: `markdown-viewer.css` を CSS 変数化（フォールバックで現状維持）。`samples` の見た目が変わらないことを目視確認（`arto page samples/02-blocks.md`）。
3. アプリ: `FileViewer` に style 付与（`config_revision` 購読）。
4. Preferences: Reading タブに Typography 節とサンプル段落、Custom 入力欄。必要なら `form_controls` にテキスト入力を追加。
5. `arto-page`: `PageOptions.typography` と `build_document` への埋め込み、テスト。
6. 仕上げ: full-width ボタンの文言調整（決まれば）、README の設定説明を追記。

## テスト方針

- `arto-config`: 既定値テスト（`test_config_default` に追記、現状値と一致）、serde ラウンドトリップ、未知値・範囲外のクランプ、旧 `config.json`（`typography` 無し）が既定で読めること、`css_declarations` のスナップショット、サニタイズ（`</style>`・`;`・`{` を含む値が拒否される）。`the_committed_schema_matches_the_types` を通す。
- `arto-page`: `build_document` の出力に変数が入ること、`system` では `--reading-font-family` が出ないこと、悪意ある custom が埋め込まれないこと（`indoc` で期待 HTML）。
- アプリ: `css_declarations` を純関数に寄せたので component テストは不要。手動で「Preferences 操作→開いているウィンドウに即反映」「full-width が行長に勝つ」「ズーム併用時にトレース／ガターが追従」を確認（アプリ起動はユーザーに依頼）。
- `just fmt check test` を各コミットで通す。

## 規模感（S/M/L）とリスク

**M**（Rust 3 クレート＋CSS＋Preferences UI、約 500–700 行）。

- リスク: Primer の `.markdown-body` 指定との詳細度競合（同じセレクタで後勝ちにする必要。import 順は `main.css:7` → `content.css` なので現状は後勝ち）。
- 基準サイズ変更で `min_content_width`（`reading_tab.rs` の Minimum Content Width、px）との関係が直感に反する可能性（行長は em、最小幅は px）。
- custom 書体の注入対策を誤ると `arto page` の CSP 外スタイル注入になる。
- Quick Look の WebView で `var()` フォールバックが効くことは確認が必要。

## 未決事項（ユーザー判断が必要な点）

1. 行長の単位を em に統一してよいか（`ch` プリセットも欲しいか）。既定値を 60em（=現状 960px）に据え置くか、読みやすさ寄りに 45–50em に下げるか。
2. 基準文字サイズ設定を入れるか（ズームと役割が重なる）。入れるなら `code` / 表の相対サイズも調整対象にするか。
3. 書体プリセットの具体的なスタック（特に明朝体の和文フォント順）と、`palt`・`text-spacing-trim`・`line-break: strict` など CJK 組版オプションを設定化するか固定するか。
4. full-width トグルの位置づけ（per-window 上書きのまま／「行長 = 無制限」を設定値の一つにして統合）。
5. 印刷で行長を反映するか（現状は常に用紙幅）。
