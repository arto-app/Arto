# バックリンク

## 目的

「この文書を参照している文書はどれか」を読み手が辿れるようにする。今開いている文書を指す相対 Markdown リンク（`./x.md`、`../x.md#sec`）と wiki リンク（`[[x]]`）を、開いているフォルダ配下から探し、リンク元の文書と該当行の抜粋を一覧にして、クリックでその箇所へ移動できるようにする。

## 既存の仕組み（流用するもの・影響するもの）

- **ファイル列挙**: `crates/arto/src/files.rs` がバックグラウンドで `.gitignore` を尊重して列挙し、3 種の上限（`MAX_FILES = 20_000` `:44`、`MAX_VISITED` `:53`、`TIME_BUDGET = 10s` `:61`）、30 秒の鮮度 `FRESH_FOR` `:75`、最大 4 フォルダ保持 `:67` を持つ。`listing()` `:176`（非ブロッキング）、`ensure()` `:191`（スレッドで走査）、完了通知 `FILES_CHANGED` `:168`、リロード時の全破棄 `forget()` `:245`。`Listing.truncated` `:89` で部分列挙を示せる。
- **パレットの使い方**: ウィンドウのフォルダ = `primary_root()`、開いたときに `ensure`、`FILES_CHANGED` で再描画 — `crates/arto/src/components/palette.rs:258-277,336-350`。`primary_root` は `crates/arto/src/state/app_state/sidebar.rs:142-147`。
- **ルート集合**: places（ブックマーク、全ウィンドウ共通）と temps（ウィンドウ固有）— `crates/arto/src/roots.rs:1-16`、`all()` `:92`、文書を含むルートを返す `covering()` `:118`、`canonical_key()` `:234`。
- **パネルの面（face）**: `Face { Places, Recent, Starred }` と巡回順 `ORDER` `crates/arto/src/state/app_state/sidebar.rs:14-41`、面の切替 `match face` `crates/arto/src/components/sidebar.rs:40-45`、レールのボタン `crates/arto/src/components/sidebar/rail.rs:52-76`、キーボードカーソルが歩く行 `panel_items` `crates/arto/src/keybindings/dispatcher/panel.rs:13-60`、面へのジャンプ `face_to` `:70`、`Action::SidebarFaceRecent` 等 `crates/arto-keybindings/src/action.rs:119-123`、既定キー `Cmd+1..3` `crates/arto-keybindings/src/presets/default.json:131-141`。単一リストの面は `Group::Flat` `crates/arto/src/state/app_state/sidebar.rs:55`。行の見た目・右クリックは `recent.rs`/`quick_access.rs` と `row_actions.rs`、`DocumentName` を流用。`Link` アイコンは既存（`crates/arto/src/components/icon.rs:48`）。
- **リンクの規則（クリック時と一致させる対象）**: `a[href]` の変換規則（外部スキーム除外、フラグメント分離、`file:` URL、拡張子 `md`/`markdown`）`crates/arto-markdown/src/post_process.rs:476-515`。wiki 対象は「同じディレクトリの `Page.md`」`crates/arto-markdown/src/engine/wiki.rs:1-30`。wiki と通常リンクの判別はソース span による `crates/arto-markdown/src/engine/hooks.rs:95-116`。リンクの実際の解決は `base_dir.join(path).canonicalize()` `crates/arto/src/document_link.rs:50-60`、`split_link_fragment` `:26`。
- **パーサ境界**: パーサ型を名指ししてよいのは `crates/arto-markdown/src/engine/` だけ（`.claude/rules/markdown-pipeline.md`）。見出し収集の AST 走査の先例 `crates/arto-markdown/src/engine/outline.rs:26-50`。
- **ウォッチャ**: 単一ファイル監視と非再帰ディレクトリ監視しかない（`FileWatcherCommand` `crates/arto/src/watcher.rs:25-30`）。再帰監視によるインクリメンタル索引は今の仕組みにはない。現在文書の変更は `use_file_watcher` `crates/arto/src/components/content/file_viewer.rs:356-388`。
- **移動**: 行（`ScrollAnchor { line, fraction }` `crates/arto/src/scroll_anchor.rs:16-24`）を `pending_scroll_anchor` `crates/arto/src/state/app_state.rs:122` に入れて開けば該当行へ着地できる。

## 仕様（UI・操作・キーバインド・設定）

- **置き場所**: パネルの 4 つ目の面 **Links**（レールの Recent の下、`IconName::Link`）。理由: 「読みながら横で一覧を見る」は Recent 面と同じ用途で、パネルのキーボードカーソル・peek・ピン留め・幅の折り畳み規則（`ui-design.md` の Giving way）を丸ごと使える。ガター（見出しの目次）は文書内の地図なので混ぜない。パレットは検索用で常時表示に向かない。
- **中身**: 見出し「N documents link here」、各行 = リンク元文書名（`DocumentName`、ルート相対パス）＋リンクのある行の抜粋（1 行、リンク部分を強調）。同一文書に複数箇所あれば文書の下に抜粋を並べる。自己参照は除外。並びは相対パス順。
- **状態表示**: 走査中は「Scanning…」、`truncated` なら「一部のみ（フォルダが大きすぎます）」、0 件は「No backlinks」、フォルダ外の文書（どのルートにも含まれない）は「フォルダを開くと表示されます」。
- **操作**: クリック = その文書を開き該当行へ（`Group::Flat` 行、Enter も同じ）。中クリック/Cmd+クリック = 新ウィンドウ。右クリックは既存の行メニュー（Reveal, Copy Path 等）。
- **キーバインド**: `sidebar.face_links`（既定 `Cmd+4`、vim/emacs は各プリセットの面キーの並びに合わせる）、`focus.links`（既存 `FocusRecent` 相当）。`Face::ORDER` に加わるので `sidebar.face_next/prev` も巡回する。
- **設定**: 初版は設定なし。上限は `files.rs` の定数を流用。

## 設計（データ構造、保存先を Config / PersistedState / State / 別ファイルのどれにするかと理由、Rust と frontend の分担）

- **保存先**: 索引はプロセス内メモリのみ（`files.rs` と同じ考え方: ディスクに索引ファイルを持たない。再起動後の初回走査は数秒以内で、古い索引との整合問題を避ける）。現在の面 `Face::Links` は State（`Sidebar.face`）。面は現在 PersistedState に保存されていないので追加しない。Config は不要（将来オフ/上限を足すなら Config）。
- **リンク抽出（arto-markdown に新 API）**: `pub fn document_links(markdown: &str, options: &RenderOptions) -> Vec<DocumentLink>`、`DocumentLink { href: String, wiki: bool, line: u32 }`。`engine/links.rs`（新規）で AST を走査し、`Node::Link` の url（wiki は `wiki::href` 済みの値）と span の行を返す。参照スタイルリンクも AST 上で解決済みなので正規表現より正確、コードブロック内の `[x](y)` を誤検出しない、`wiki_links` オプションが無効なら wiki を拾わない — つまり「クリックできるものだけがバックリンク」になる。raw HTML の `<a href>` は初版では対象外。
- **前段フィルタ**: 全ファイルをパースすると 2 万件で重いので、ファイルを読んだ段階でバイト列に対象のファイル名の stem（例 `guide`）が含まれない文書はパースしない（`str::contains` で十分。`memchr` は直接依存に無いので追加しない。大文字小文字の扱いは OS 依存で要検討）。大半のファイルはここで落ちる。
- **解決（純関数、`crates/arto/src/backlinks.rs` 新規）**: `resolves_to(source_file, href, target_canonical) -> bool` — 外部スキーム除外 → `split_link_fragment` → `file:` URL → 拡張子判定 → `source_dir.join(path)` を canonicalize して比較。`post_process.rs` / `document_link.rs` と同じ規則を別実装しないよう、`has_foreign_scheme` と `file_url_to_path` を arto-markdown から `pub` で出す（現状 private、要変更）。
- **索引戦略: オンデマンド走査 + mtime 検証キャッシュ**。再帰ウォッチャが無いこと、`files.rs` がすでに「開いたときに走査、30 秒鮮度」で設計されていることから、ウォッチャ駆動のインクリメンタル索引は採らない。
  - `LINK_CACHE: LazyLock<RwLock<HashMap<PathBuf, Entry>>>`、`Entry { mtime, len, links: Arc<[ResolvedLink]> }`（`ResolvedLink { target: PathBuf(canonical), line, excerpt }`）。対象に関係なく「そのファイルが指す先」を覚えるので、別文書に移っても再利用でき、2 回目以降は mtime/len の `stat` だけで済む。前段フィルタは初回に「全リンクを取る」ためには使えないため、キャッシュ未作成のファイルは対象 stem で絞り、絞って捨てたファイルは「未解析」のまま残す（次の対象で再判定）。
  - `backlinks::ensure(root, target)` がスレッドで `files::listing(root, false)` を使い（無ければ `files::ensure` して `FILES_CHANGED` を待つ）、並列（`rayon` は直接依存に無いので、`files.rs` の `threads()` と同じ数の std スレッドで分割、または逐次 + 時間上限）で読み→フィルタ→解析→解決し、結果 `Backlinks { target, root, sources: Vec<(PathBuf, Vec<ResolvedLink>)>, truncated, partial }` を保持、`BACKLINKS_CHANGED: broadcast::Sender<()>` で通知。
  - **上限**: 走査対象は `files.rs` の上限そのまま（最大 2 万）、1 ファイル 2 MiB 超はスキップ、全体の時間上限 10 秒（超過で `partial`）。
  - **更新契機**: 面を開いたとき、現在文書が変わったとき、`FILES_CHANGED`、リロード（`files::forget` と同時に `LINK_CACHE` も era 方式で破棄）。面が閉じている間は走査しない。
- **走査範囲**: 現在文書を含むルート（`Roots::covering`）。無ければ `primary_root`。全ルート横断はしない（未決事項）。
- **frontend**: 追加なし（Dioxus コンポーネントのみ）。CSS は `frontend/style/components/sidebar/` に `links-face` 用を追加（抜粋行の 2 段表示）。

## 実装手順（1 ステップ = 1 コミット目安）

1. arto-markdown: `engine/links.rs` と `document_links()` を追加、`has_foreign_scheme`/`file_url_to_path` を公開。`tests/pipeline.rs` に振る舞いテスト。
2. `crates/arto/src/backlinks.rs`: 解決の純関数、前段フィルタ、抜粋生成（行テキストの切り詰め）とユニットテスト。
3. `backlinks.rs`: `LINK_CACHE`・`ensure`・`BACKLINKS_CHANGED`・上限・era 破棄（`files::forget` 経路に接続）。
4. `Face::Links` を追加（`ORDER`、`sidebar.rs` の match、`rail.rs` のボタン、`panel_items`）と空の面コンポーネント。
5. `components/sidebar/links.rs`: 一覧描画・状態表示・クリックで開いて行へ移動・右クリックメニュー、CSS。
6. キーバインド: `SidebarFaceLinks`/`FocusLinks` を `arto-keybindings` に追加、3 プリセット・dispatcher・メニュー生成・`schemas/` 更新。

## テスト方針

- arto-markdown（公開 API 経由）: `indoc` の文書で、インライン/参照スタイル/wiki（`wiki_links` オン・オフ）/フラグメント付き/コード内は拾わない/画像は拾わない、行番号がフロントマター込みで正しい。
- `backlinks.rs`（`tempfile`）: `../`・サブディレクトリ・`%20` を含むパス・`file:` URL・拡張子なし wiki・シンボリックリンク経由の canonical 一致・自己参照除外・存在しない先の無視。mtime 変更でキャッシュが無効になる。上限（小さい `Budget` を注入）で `partial` になる。
- `Face::ORDER` の巡回テスト（`sidebar.rs:341` の既存テストを更新）。
- `just fmt check test`。

## 規模感（S/M/L）とリスク

**M〜L**（抽出 API と索引で M、面とキーバインド・プリセット・スキーマ一式を含めて L 寄り）。リスク: 巨大フォルダ・ネットワークドライブでの初回走査時間（前段フィルタとキャッシュで緩和）、canonicalize の大量呼び出しコスト（ディレクトリ単位でメモ化）、wiki の解決規則の期待差（Obsidian はフォルダ全体から名前で探すが Arto は同ディレクトリ）、パネルの面が 4 つになることによるレールの密度と既存キー（`Cmd+4` の空き）の確認。

## 未決事項（ユーザー判断が必要な点）

1. 置き場所: パネルの新しい面（推奨）か、文書末尾の「Linked from」セクション、またはガターへの併記か。
2. wiki リンクの解決: クリック時と同じ「同ディレクトリのみ」（推奨、一貫性）か、Obsidian 風に「フォルダ内の同名ファイル」も数えるか（後者ならクリック側の解決も変える必要がある）。
3. 走査範囲: 文書を含むルートのみ（推奨）か、全ルート（places + temps）横断か。
4. raw HTML の `<a href="x.md">` と、存在しない文書へのリンク（未作成ページ）を扱うか。
5. 既定キー `Cmd+4` と面の名前（Links / Backlinks）。
