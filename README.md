# vacuum

ビルド成果物や依存関係キャッシュを再帰的に削除する CLI ツール

https://github.com/user-attachments/assets/1f263a29-222e-4bf8-9101-e04cb53ac56c

## インストール

```sh
cargo install --path .
```

## 使い方

```
vacuum [OPTIONS] [PATH]...
```

`PATH` を省略すると、カレントディレクトリをスキャンする。
`PATH` は複数指定できる（例: `vacuum ~/work ~/oss`）。一方がもう一方の中にある場合は、外側だけをスキャンする。
一覧に表示するパスは、指定したすべての `PATH` に共通する祖先からの相対パスになる。

safe モードでは、Enter を押すと確認ダイアログが出る。`y` で削除し、`n` か `Esc` で選択画面に戻る。

### モード

| モード               | 動作                                                       |
| -------------------- | ---------------------------------------------------------- |
| `safe`（デフォルト） | 候補を一覧表示し、削除するものをインタラクティブに選択する |
| `auto`               | 確認なしで全候補を自動削除する                             |

```sh
# インタラクティブに選択して削除
vacuum ~/projects

# 全候補を自動削除
vacuum ~/projects --mode auto

# 削除せず候補を確認する
vacuum ~/projects --dry-run

# サイズ計算をスキップして高速スキャン
vacuum ~/projects --mode auto --no-size

# 完全に削除せず、OS のゴミ箱に移動する（あとで復元できる）
vacuum ~/projects --trash
```

### 除外

`--exclude <GLOB>` に一致するパスは、走査も削除もしない。複数回指定できる。
スキャンルートの `.vacuumignore` も読み込む。どちらも gitignore と同じ書式で、スキャンルートからの相対パスで書く。

```sh
# 作業中のプロジェクトと、巨大な .venv を残す
vacuum ~/projects --exclude work/current --exclude '*.venv'
```

```gitignore
# ~/projects/.vacuumignore
work/current/
*.venv
```

### 最近触っていないプロジェクトだけを掃除する

`--older-than <DURATION>` を付けると、一定期間更新のないプロジェクトの候補だけを対象にする。
単位は `h`（時間）、`d`（日）、`w`（週）。

```sh
# 30 日以上触っていないプロジェクトの依存だけを消す
vacuum ~/projects --older-than 30d
```

プロジェクトの最終更新は、候補の親ディレクトリ配下にあるファイルの更新日時の最新値とする。
候補ディレクトリ自体はビルドのたびに更新されるため、判定に含めない。

### 大きな候補だけを対象にする

`--min-size <SIZE>` を付けると、指定サイズ未満の候補を除外する。少ない操作で大きな容量を空けたいときに使う。
サイズの計算が必要なので、`--no-size` とは同時に指定できない。

```sh
vacuum ~/projects --min-size 100MB
```

### 走査範囲を制限する

- `--max-depth <N>`: スキャンルートから N 階層までしか走査しない。ルート直下が 1 階層目。
- `--one-file-system`: 別のファイルシステム（マウントポイント）に入らない。ネットワークドライブや外部ディスクを誤って走査するのを防ぐ。

### 終了コード

削除に 1 件でも失敗すると、終了コード 1 で終わる。スクリプトや CI から失敗を検知できる。

## アダプター

各アダプターが対応する言語やエコシステムのゴミファイルを検出する。

| フラグ        | 削除対象                                       | デフォルト      |
| ------------- | ---------------------------------------------- | --------------- |
| `--node`      | `node_modules/`                                | on              |
| `--js-cache`  | `.next/`, `.nuxt/`, `.svelte-kit/`, `.turbo/`, `.parcel-cache/`, `.angular/` | on |
| `--cargo`     | `target/`                                      | on              |
| `--python`    | `__pycache__/`, `.venv/`, `dist/`, `build/` 等 | on              |
| `--python-cache` | `.pytest_cache/`, `.mypy_cache/`, `.ruff_cache/`, `.tox/`, `.nox/`, `*.egg-info/` | on |
| `--go`        | `vendor/`                                      | on              |
| `--gradle`    | `.gradle/`, `build/`                           | on              |
| `--maven`     | `target/`                                      | on              |
| `--ocaml`     | `_build/`, `_opam/`                            | on              |
| `--dotnet` | `bin/`, `obj/`（`*.csproj` などと同じ階層） | on |
| `--swift` | `.build/` | on |
| `--haskell` | `.stack-work/`, `dist-newstyle/` | on |
| `--elixir` | `_build/`, `deps/` | on |
| `--zig` | `.zig-cache/`, `zig-cache/`, `zig-out/` | on |
| `--dart` | `.dart_tool/`, `build/` | on |
| `--php` | `vendor/`（`composer.json` と同じ階層） | on |
| `--ruby` | `vendor/bundle/` | on |
| `--scala` | `target/`, `project/target/` | on |
| `--terraform` | `.terraform/`（中に `terraform.tfstate` があれば残す） | on |
| `--elm` | `elm-stuff/` | on |
| `--gitignore` | `.gitignore` にマッチするファイルすべて        | **off**（危険） |

コンテキストファイル（`package.json`, `Cargo.toml` 等）が存在するディレクトリのみを対象とする。誤検知を防ぐためのガードとして機能する。

### git 管理下の候補

git で追跡しているファイルを含む候補は、既定で除外する。
例えば、意図的にコミットした Go の `vendor/` は削除しない。
`--include-tracked` を付けると候補に含め、一覧で `[tracked]` と表示する。

判定には `git ls-files` を使う。git が使えない場合や、リポジトリの外にある候補は、追跡なしとして扱う。

### Git worktree

`gitignore` アダプターは Git worktree やサブモジュール（`.git` を持つディレクトリ）を丸ごと削除しない。
worktree の中では、その worktree 自身の `.gitignore` を適用する。
例えば `.claude/worktrees/` が無視されていても、Claude Code の worktree は残る。
worktree 内のビルド成果物だけが削除される。

### 否定パターン

`gitignore` アダプターは否定パターン（`!build/keep.txt` など）で除外解除されたパスを削除しない。
除外解除されたパスを含むディレクトリは丸ごと削除せず、中身を個別に判定する。

### アダプターの有効・無効化

フラグにはオプショナルなブール値を渡せる。
safe モードでは、フラグの値がアダプター選択画面の初期状態になる。

```sh
# node アダプターを無効化
vacuum --node=false

# gitignore アダプターを有効化（危険）
vacuum --gitignore
```

### スキャンを高速化する

- `--no-size` を付けるとサイズ計算を省略する。巨大ディレクトリが多いときに有効。
- 使わないアダプターは `--xxx=false` で無効化する。

## シェル補完

```sh
# fish
vacuum --generate-completions fish > ~/.config/fish/completions/vacuum.fish

# zsh
vacuum --generate-completions zsh > ~/.local/share/zsh/site-functions/_vacuum

# bash
vacuum --generate-completions bash > ~/.local/share/bash-completion/completions/vacuum
```

## 開発

### セットアップ

```sh
git clone https://github.com/r4ai/vacuum
cd vacuum
cargo build
```

### コマンド

```sh
# デバッグビルドして実行
cargo run -- [OPTIONS] [PATH]

# リリースビルド
cargo build --release

# テスト実行
cargo test

# 静的解析（警告・エラーを確認）
cargo clippy

# フォーマット
cargo fmt
```

### ベンチマーク

性能改善の効果は、ダミーツリーに対する実行時間で比べる。
計測には [hyperfine](https://github.com/sharkdp/hyperfine) を使う。

```sh
# ダミーツリーを生成する（第 2 引数はプロジェクト数。既定は 200）
cargo run --release --example gen_bench_tree -- /tmp/vacuum-bench 200

# 計測する（削除はしない）
cargo build --release
hyperfine --warmup 3 \
  'target/release/vacuum /tmp/vacuum-bench -m auto --dry-run --no-size' \
  'target/release/vacuum /tmp/vacuum-bench -m auto --dry-run'
```

- 生成するツリーは決定的で、同じ引数なら毎回同じになる。
- 変更前後を比べるときは、それぞれのバイナリを別名でコピーし、1 回の `hyperfine` で並べて計測する。
- 上の手順はページキャッシュが温まった状態を測る。コールドキャッシュを測るなら、Linux では `--prepare 'sync; echo 3 | sudo tee /proc/sys/vm/drop_caches'` を付ける。

### アダプターの追加

アダプターは 1 つのエントリ（ディレクトリかファイル）を判定するだけでよい。
ディレクトリの走査は `src/scanner.rs` が 1 回だけ行い、有効なアダプターに順に問い合わせる。

1. `src/adapters/` に新しいファイルを作成し、`Adapter` トレイトを実装する
2. `src/adapters/mod.rs` の `REGISTRY` に定義を 1 つ追加する
3. `README.md` のアダプター一覧表を更新する（漏れるとテストが失敗する）

CLI フラグ・スキャン設定・TUI のアダプター一覧は、`REGISTRY` から自動で生成される。

```rust
// src/adapters/my_adapter.rs
use std::path::Path;

use crate::adapter::{Adapter, Context};

pub struct MyAdapter;

impl Adapter for MyAdapter {
    fn name(&self) -> &'static str {
        "my-adapter"
    }

    // `parent` の中の `name` ディレクトリが削除対象なら、説明を返す。
    fn match_dir(&self, name: &str, parent: &Path, ctx: &mut Context) -> Option<String> {
        (name == "out" && ctx.has_file(parent, "my.toml")).then(|| "My build output (out/)".into())
    }
}
```

ファイルを対象にする場合は `match_file` も実装する。
単体テストでは `scanner::scan_with` を使う。本番と同じ走査経路で検証できる。
