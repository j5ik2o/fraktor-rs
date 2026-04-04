# redundant-fqcn-lint

## 概要
- `redundant_fqcn::redundant_fqcn` は、`use` 宣言以外の場所で `crate::...`、`self::...`、`super::...`、または workspace crate (`fraktor_*`) から始まる不要な完全修飾パスを検出します。
- import は `crate::` 始まりで書きつつ、本文側では短い名前へ寄せる、というこのリポジトリの読みやすさの規約を機械的に守るための lint です。
- diagnostic 自体を AI 向け修正指示として設計し、`use` 追加と本文置換をそのまま自動化できるようにします。

## チェック内容
- `use crate::...` / `use self::...` / `use super::...` / `use fraktor_*::...` は対象外です。
- 関数呼び出し、構築式、`match` パターン、関数シグネチャ、型注釈、フィールド型など、`use` 以外に現れる完全修飾パスのうち、型名や enum 名を含むパスを警告します。
- `pub(in crate::...)` と `QSelf` を使った完全修飾は対象外にして、必要な完全修飾の誤検知を抑えます。
- すでに同名の別 import があり、短い名前にすると衝突する場合は許可します。例: `use domain::UserAccount;` がある状態で `crate::infra::UserAccount(ua)` を呼ぶケース。

## 違反例
```rust
fn build() -> crate::sample::domain::Widget {
  crate::sample::domain::Widget::new()
}

fn is_idle(mode: crate::sample::domain::Mode) -> bool {
  matches!(mode, crate::sample::domain::Mode::Idle)
}

fn log() {
  fraktor_actor_rs::core::kernel::event::logging::LogEvent::new(
    fraktor_actor_rs::core::kernel::event::logging::LogLevel::Info,
    "test".into(),
    core::time::Duration::from_secs(0),
    None,
    None,
  );
}

fn accepts(pid: fraktor_actor_rs::core::kernel::actor::Pid) {
  let _ = pid;
}
```

## 修正ガイド
1. ファイル冒頭の `use` ブロックへ `use crate::sample::domain::{Mode, Widget};` のような import を追加する。
2. 本文中の `crate::...` プレフィックスを取り除き、`Widget::new()` や `Mode::Idle` のような短い表記へ置き換える。
3. 同じファイル内の同種の FQCN をまとめて統一する。
4. `use` 宣言と対象箇所以外のコードは変更しない。

## 例外指定
- 一時的に無効化する場合は `#![allow(redundant_fqcn::redundant_fqcn)]` を使う。
- 本当に完全修飾が必要な箇所は、まず QSelf や型推論の事情で必要かを確認し、必要な場合のみ局所的に `#[allow(...)]` を使う。
