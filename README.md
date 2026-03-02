# vacuum

ビルド成果物や依存関係キャッシュを再帰的に削除する CLI ツール。

## インストール

```sh
cargo install --path .
```

## 使い方

```
vacuum [OPTIONS] [PATH]
```

`PATH` を省略すると、カレントディレクトリをスキャンする。

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
```

## アダプター

各アダプターが対応する言語やエコシステムのゴミファイルを検出する。

| フラグ        | 削除対象                                       | デフォルト      |
| ------------- | ---------------------------------------------- | --------------- |
| `--node`      | `node_modules/`                                | on              |
| `--cargo`     | `target/`                                      | on              |
| `--python`    | `__pycache__/`, `.venv/`, `dist/`, `build/` 等 | on              |
| `--go`        | `vendor/`                                      | on              |
| `--gradle`    | `.gradle/`, `build/`                           | on              |
| `--maven`     | `target/`                                      | on              |
| `--gitignore` | `.gitignore` にマッチするファイルすべて        | **off**（危険） |

コンテキストファイル（`package.json`, `Cargo.toml` 等）が存在するディレクトリのみを対象とする。誤検知を防ぐためのガードとして機能する。

### アダプターの有効・無効化

フラグにはオプショナルなブール値を渡せる。

```sh
# node アダプターを無効化
vacuum --node=false

# gitignore アダプターを有効化（危険）
vacuum --gitignore
```

## シェル補完

```sh
# fish
vacuum --generate-completions fish > ~/.config/fish/completions/vacuum.fish

# zsh
vacuum --generate-completions zsh > ~/.local/share/zsh/site-functions/_vacuum

# bash
vacuum --generate-completions bash > ~/.local/share/bash-completion/completions/vacuum
```

## 新しいアダプターの追加

1. `src/adapters/` に新しいファイルを作成する
2. `Adapter` トレイトを実装する
3. `src/adapters/mod.rs` に追加する
4. `src/cli.rs` にフラグを追加する
5. `src/scanner.rs` の `build_adapters` に組み込む

```rust
// src/adapters/my_adapter.rs
use crate::adapter::{Adapter, CleanTarget, compute_dir_size};

pub struct MyAdapter;

impl Adapter for MyAdapter {
    fn name(&self) -> &'static str { "my-adapter" }
    fn description(&self) -> &str { "..." }
    fn is_safe(&self) -> bool { true }

    fn scan(&self, root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
        // walkdir で対象ディレクトリを検索し、CleanTarget を返す
        todo!()
    }
}
```
