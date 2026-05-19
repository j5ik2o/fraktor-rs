# `association_driver` を `association` へ再改名する計画

## 概要
`modules/remote-adaptor-std/src/std/association_driver*` は driver 群だけでなく `AssociationShared`、`AssociationRegistry`、`SystemMessageDeliveryState`、各種 loop / helper を束ねる実装名前空間であり、`driver` は責務を狭く言い過ぎている。`adapter` も Port 実装層ではないため不適切である。  
そのため、`remote-adaptor-std` 側の package 名は `association` に統一する。

## 主要変更
- `modules/remote-adaptor-std/src/std/association_driver.rs` を `association.rs` に改名する。
- `modules/remote-adaptor-std/src/std/association_driver/` ディレクトリを `association/` に改名する。
- `std.rs` の公開面を `pub mod association;` に更新する。
- `association_driver` 参照を現行コード・現行 docs/specs から `association` に置換する。
- `association_driver` 配下の内部ファイル名はそのまま維持する。
  - `handshake_driver.rs`、`outbound_loop.rs`、`inbound_dispatch.rs` など個別責務名は現状のままでよい。
- アーカイブ済みの履歴文書は原則そのままにし、現行の `docs/`、`openspec/specs/`、進行中 `openspec/changes/` のみ追随する。

## 実装方針
- 命名判断は「中の中心概念」で行う。
  - package 全体の中心は `Association` であり、個別の driver ではない。
- 既存の `tick_driver` とは切り分ける。
  - `tick_driver` は `TickDriver` という中心概念 package。
  - 今回は `Association` という中心概念 package なので `association` が対応する。
- `association_adapter` や `association_runtime` への再変更は行わない。
  - `adapter` は意味不整合。
  - `runtime` は規約違反。

## テスト
- `./scripts/ci-check.sh ai dylint ambiguous-suffix-lint`
- `./scripts/ci-check.sh ai all`
- 追加確認:
  - `rtk rg -n "association_driver" modules docs openspec/specs openspec/changes`
  - 現行コードと現行 docs/specs に `association_driver` が残っていないこと
  - remote-adaptor-std の association 系テスト名や import が `association` 経由で通ること

## 想定される影響
- import path は `crate::std::association::*` に変わる。
- `docs/gap-analysis/remote-gap-analysis.md`、現行 `openspec/specs/remote-adaptor-std-*`、進行中 change の文書は追随が必要。
- archive 配下は履歴として凍結扱いにする前提。

## 前提
- `association_driver` への前回改名はまだ未コミットで、ここから再改名してよい。
- `association` という package 名は、`remote-adaptor-std::std::association` という文脈で十分に読めるものとして扱う。
