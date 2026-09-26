# 前回から変わった箇所の表示（Changes since last read）

## 目的

エージェントや共同編集者が書き換えた Markdown を開き直したとき（またはライブリロードで書き換わったとき）、
「前回読んだときから何が増えた・変わったか」を余白と右ガター（見出しルーラー）に示し、変更箇所だけを順に辿れるようにする。
Arto の「読むためのツール」という立ち位置から、差分は読み手の読書履歴（visits）を基準にとる。git の差分とは別物にする。

## 既存の仕組み（流用するもの・影響するもの）

- 読書履歴: `crates/arto/src/visits.rs:46` `Visit { path, at, anchor }`。`record_visit`（`visits.rs:374`）は開いたときに呼ばれ、
  `keep_position`（`visits.rs:356`）は離れるときに呼ばれる（`state/app_state/document/file_ops.rs:38`, `:59` `keep_reading_position`）。
  「読んだ」の定義はこの二つのタイミングに合わせる。visits.json は開くたびに丸ごと書き直される（`visits.rs:23-35` の注記）。
  そのため本文スナップショットをここに入れるのは不可。
- 保存先の前例: `crates/arto/src/lenses/store.rs:29` `MAX_BYTES`、`:39` `STORE`（`data_local_dir()/arto/lenses`）、
  `record_path`（パスの SHA-256 をファイル名にする, `store.rs:250` 付近）、`evict`（mtime の古い順に消して上限内に収める, `:260` 付近）、
  `write_atomically`（`:297` 付近）。キャッシュではなくデータ扱いで、`cache.rs:87` の WebView キャッシュ消去の対象外。
- 描画元のソース: `crates/arto/src/lenses.rs:55` `RenderedSource { path, source: Arc<str>, generation }`。
  `components/content/file_viewer.rs:119-121` で描画のたびに `state.rendered_source` にセットされる。「画面に出ていた版」はこれで取れる。
- リロード: `file_viewer.rs:356-388` `use_file_watcher` → `state.reload_document()` → `reload_trigger`（`state/app_state.rs:133`）→ `use_file_loader`（`file_viewer.rs:93`）。
  描画後は `reapply_search()`（`file_viewer.rs:291`）と同じ MutationObserver 方式で JS に渡せる。
- ブロックとソース行の対応: `data-source-range="L:C-L:C"`（`crates/arto-markdown/src/lib.rs:33-60`）。
  パーサは `frontend/src/source-range.ts:22` `parseSourceRange`。レンズのブロック列挙は `frontend/src/lenses.ts:61` `KINDS` と `:171` `collect`。
  入れ子のリスト項目の扱い（`:199-205`）もここに合わせる。
- 余白マーカーの前例: `.lens-marker`（`frontend/style/components/lenses.css:41`、`left:-18px` の点）。変更マーカーはこれと形を変える（縦線）。
- ガター: `components/content/gutter.rs:85` `Ruler`（見出しごとの tick）、`:139` `Names`。
  tick への着色は `frontend/src/reading-position.ts:111` `hitsByHeading` → `:340-350` で `data-hit` と `--hit-colour` を付ける。
  CSS は `frontend/style/components/contents-gutter.css:212`。変更も「見出し単位で印を付ける」同じ流れに乗せる。
- キーバインド: `crates/arto-keybindings/src/action.rs:592-596`（`contents.*` の命名）、presets `default.json:147`（`Cmd+j`）、`vim.json:305`（`] ]`）。

## 仕様（UI・操作・キーバインド・設定）

- 基準版（baseline）: 文書ごとに「最後に読み終えた時点のソース全文」を 1 版だけ持つ。
- 表示: 文書を開いたとき、または watcher で再描画したときに、基準版と現在のソースを行単位で比較する。
  - 追加・変更されたブロックには、本文左余白に細い縦線（`.change-marker`、追加は `--data-green-color-emphasis`、変更は `--accent-color` 系）を出す。
  - 削除位置には、ブロック間に短いヘアラインを出す。
  - ガター: 変更を含む見出しの tick に `data-changed` を付け、tick の左に小さな点を出す（`data-hit` の着色とは独立）。
    Contents 一覧の見出し行にも同じ点を出す。
  - Contents 一覧の先頭（Pinned の上）に「Changes · N」行を出す。クリックで最初の変更へ移動する。
    行内に「Mark as read」ボタンを置き、押すと基準版を今の版に進めて印を消す。
- 基準版が進むタイミング（「読んだ」の定義。既定案）:
  1. 初めて開いたとき: 基準版がなければ、現在版を基準にする（印は出ない）。
  2. 文書を離れたとき（`keep_reading_position` と同じ呼び出し点）とウィンドウを閉じたとき: 画面に出ていた `RenderedSource` を基準にする。
  3. 明示操作 `changes.mark_read` を実行したとき。
  - 開いている間の watcher リロードでは基準版を進めない。そのため、開いている間に積み重なった変更は、離れるまで印が残り続ける。
- 移動: `changes.next` / `changes.prev`（新規 Action）。既定案は default が `Alt+]` / `Alt+[`、vim が `] c` / `[ c`（vimdiff 流儀）、emacs は未割当。
  移動先は `window.Arto.scroll` と同じ中央寄せで、到着時に短いフラッシュを出す。
- 設定（Config、新セクション `reading`）: `reading.showChanges: bool`（既定 true）、`reading.ignoreWhitespaceChanges: bool`（既定 true）。
  Preferences には 1 行ずつ追加する。

## 設計（データ構造、保存先を Config / PersistedState / State / 別ファイルのどれにするかと理由、Rust と frontend の分担）

- 保存するもの: ブロックハッシュではなくソース全文を保存する。理由は次のとおり。
  - ブロック境界はレンダラ（comrak オプションや後処理）の変更で動く。ハッシュ列だとレンダラ更新のたびに全ブロックが「変更」になる。
  - 行差分であれば、削除位置の表示や将来の「前の版をホバーで見る」にそのまま使える。
  - 比較は Rust 側の行 diff で行う。新規依存 `similar`（workspace に未導入。Myers/patience）を追加する。
- 保存先: **別ファイル**。`data_local_dir()/arto/baselines/<sha256(path)>.json.gz`。
  - Config: ユーザーが編集するものではないので不可。
  - PersistedState: 最後に閉じたウィンドウの状態であり、文書ごとではないので不可。
  - State: 再起動で消えるので不可。
  - visits.json: 丸ごと書き直す一覧に本文を混ぜると重くなるので不可。
  - 圧縮には既存依存の `flate2` を使う（`crates/arto/Cargo.toml:75`）。
- 新モジュール `crates/arto/src/baselines.rs`:
  ```rust
  struct Baseline { version: u32, path: PathBuf, read_at: DateTime<Local>, source: String }
  pub fn load(path) -> Option<Baseline>; pub fn advance(path, source: &str);
  pub fn changes(old: &str, new: &str, ignore_ws: bool) -> Vec<Change>
  #[serde(tag="kind")] enum Change { Added{start,end}, Modified{start,end}, Removed{after} } // 新側の 1-based 行
  ```
  上限は 1 文書あたり 2 MiB（超える文書は基準版を持たず機能オフ）、合計 64 MiB。
  lens store の `evict` と `write_atomically` は `crate::utils::data_store`（新規）に切り出して共用する。
- State: `AppState` に `changes: Signal<Vec<Change>>` を追加する。基準版そのものはメモリに持たず、比較の瞬間だけ読む。
- Rust の担当: 基準版の読み書き、diff、`Change` 列の算出、基準版を進めるタイミング、Action 処理。
  `use_file_loader` の描画成功直後（`file_viewer.rs:117-121`）に `spawn_blocking` で diff を取り、`state.changes` を更新して JS に渡す。
- frontend の担当（新規 `frontend/src/changes.ts`）: `setChanges(changes)` で `.markdown-body` の葉ブロックを走査する。
  葉ブロックとは、`data-source-range` を持ち、子孫に同じ行を含む範囲付き要素がないもの。
  行区間が交わるブロックに `data-change="added|modified"` とマーカーを付け、`Removed{after}` は該当ブロック直後に `.change-gap` を差し込む。
  `next()` / `prev()` は画面中央より下／上の最初の印へ移動する。脚注セクションは範囲が単調でないので（`lib.rs` の注記）、文書順ではなく DOM 順で辿る。
  `reading-position.ts` の `update()` で見出しごとに `data-changed` を付ける（`hitsByHeading` と同じ走査に相乗りする）。
  `window.Arto.changes = { set, next, prev, clear }` を `runtime.ts:277` に追加する。
- レンズの page 表示中は元ブロックが退避されている（`lenses.ts` の `pageState`）。そのため印は `restorePage` 後に再適用する。

## 実装手順（1 ステップ = 1 コミット目安）

1. `utils/data_store.rs` を切り出す（`write_atomically`、`evict`、`sha256` ファイル名）。lens store をこれに載せ替える。挙動は変えない。
2. `baselines.rs` を追加する: `Baseline` の load/advance（gzip・上限・evict）と単体テスト。
3. `similar` を導入し、`baselines::changes()` を実装する（空白無視オプション、削除位置）。純ロジックのテストを付ける。
4. `arto-config` に `ReadingConfig { show_changes, ignore_whitespace_changes }` を追加し、schema を再生成する（`ARTO_UPDATE_SCHEMA=1`）。
5. 基準版を進めるフックを入れる: 初回・離脱（`keep_reading_position`）・ウィンドウクローズ（`App` の `use_drop`）。
6. `file_viewer.rs` で diff → `state.changes` → `window.Arto.changes.set` の配線を作る。
7. `frontend/src/changes.ts` と CSS（`style/components/content/changes.css`）を追加する: マーカー、削除ギャップ、vitest。
8. ガター: `reading-position.ts` の `data-changed` と `contents-gutter.css`、および `Names` の「Changes · N」行と Mark as read。
9. Action `changes.next` / `changes.prev` / `changes.mark_read` を presets 3 種に追加し、メニュー（Go メニュー）にも項目を足す。
10. Preferences に 2 項目を追加する。レンズ page 表示との共存を修正する。

## テスト方針

- Rust 純ロジック: `changes()` について、追加／変更／削除／空白のみ変更／CRLF 混在（`arto-markdown/src/line_endings.rs` と同じ正規化）／末尾改行差を確認する。入力は `indoc`。
- `baselines`: `tempfile` のディレクトリを root にして、保存→読込、上限超え文書の拒否、合計上限での evict（最新は残す）、壊れたファイルは None を確認する。
- 基準版を進めるタイミングは `AppState` を介さない純関数（`should_advance(prev, event)`）に切り出してテストする。
- frontend（vitest + jsdom）: `data-source-range` 付きの HTML 断片に `setChanges` をかけ、葉ブロック選択、入れ子リスト、削除ギャップ位置、`next/prev` の順序を確認する。
- `samples/` に差分確認用の対を置くかは任意（スナップショット対象外にする）。
- 最後に `just fmt check test`。

## 規模感（S/M/L）とリスク

- 規模: **M〜L**（Rust 約 600 行＋TS 約 300 行＋CSS。10 コミット前後）。
- リスク:
  - 行 diff とブロックの対応が粗い（長い段落の 1 語変更でも段落全体が印になる）。ブロック単位の印なので許容範囲と考える。
  - 巨大ファイルや頻繁に書き換わるファイル（ログ的な Markdown）での diff コスト。`spawn_blocking` と 2 MiB 上限で抑える。
  - 「読んだ」の定義次第で、印が消えるのが早すぎたり遅すぎたりする。ここは体感調整が要る。
  - レンズの page 表示・検索ハイライト・pinned の DOM 書き換えとの競合。マーカーは属性と疑似要素中心にし、DOM 挿入は削除ギャップのみにする。

## 未決事項（ユーザー判断が必要な点）

1. 基準版を進めるタイミング: 「離れたとき自動」（既定案）でよいか。「一定時間表示した」「末尾までスクロールした」を条件にするか、明示の Mark as read のみにするか。
2. 開いている間のライブリロード: 印を基準版（開いた時点）との差にするか（既定案）、直前の描画との差にするか。後者は「今まさに書き換わった所」だけを示せる。
3. 保存の上限値（1 文書 2 MiB、合計 64 MiB）と、`baselines/` を visits の `forget_visit`（履歴から消す）と連動して消すかどうか。
4. キーバインド既定（`Alt+]` / `Alt+[`、vim は `] c` / `[ c`）。
5. 削除箇所を表示するか。表示するならヘアラインのみか、ホバーで旧テキストを見せるか（後者は追加で M 規模）。
