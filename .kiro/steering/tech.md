# 技術スタック

## アーキテクチャ

fraktor-rs は Rust workspace の multi-crate runtime です。基本方針は port-and-adapter で、`*-core` crate が移植可能な contract / state machine / policy を定義し、`*-adaptor-std` や `*-adaptor-embassy` が host runtime への binding を提供します。

`no_std` core では `alloc` を明示的に使い、`std`・Tokio・network I/O・host clock・host lock は adaptor 側へ逃がします。依存方向は core -> port 定義、adaptor -> port 実装で保ちます。

## 主要技術

- **言語**: Rust, edition 2024
- **ツールチェーン**: `rust-toolchain.toml` で固定された nightly toolchain
- **実行ターゲット**: `no_std` + `alloc` core、std/Tokio adaptor、Embassy adaptor
- **仕様管理**: OpenSpec artifact と gap-analysis document を、ふるまいに影響する作業の起点にする

## 主要ライブラリ

- **Tokio / tokio-util**: std adaptor の executor、time、TCP transport、I/O worker に限定して使う。
- **Embassy**: embedded async adaptor の runtime integration に使う。
- **serde / postcard / bincode / prost**: portable serialization と wire format の表現に使う。`no_std` crate では feature と allocation 境界を確認する。
- **portable-atomic / spin / critical-section**: `no_std` に適した共有・同期基盤を作るために使う。
- **tracing**: std 側の instrumentation に使う。core で使う場合は feature と `no_std` 境界を確認する。

## 開発標準

### 型安全性

- `*-core` crate は `#![cfg_attr(not(test), no_std)]` と `#![deny(cfg_std_forbid)]` を維持する。
- 共有所有は `ArcShared`、同期は project-defined `SpinSync*` / `Shared*` / `Default*` 系 abstraction を優先し、直接の `Arc` / `Rc` / `std::sync::Mutex` / `spin::*` 依存は避ける。
- read-then-act は TOCTOU を避け、`with_write` などの closure-based API で原子的に扱う。

### コード品質

- workspace lint で `unused_must_use`、`clippy::let_underscore_must_use`、`clippy::let_underscore_future` を deny する。
- custom dylint は module layout、test placement、FQCN import、rustdoc language、`no_std` boundary、ambiguous suffix、port/adaptor boundary を守るための一級チェックとして扱う。
- `#[allow]` で lint を回避する場合は人間の明示判断を先に取る。
- rustdoc は英語、通常コメントと Markdown は日本語を基本にする。

### テスト

- 実装中は対象 crate / 対象 feature の test、clippy、dylint、no-std check を優先して回す。
- 最終確認は `./scripts/ci-check.sh ai all` を基準にする。ただし所要時間が長いため、範囲の小さい変更では targeted check の結果も明示する。
- 実行可能な利用例は `showcases/std` に集約し、module crate 内に examples を増やさない。

## 開発環境

### 必須ツール

- `rustup`
- `rustfmt` と `clippy` を含む、固定済み nightly toolchain
- full local verification 用の `cargo-dylint`、`rustc-dev`、`llvm-tools-preview`
- OpenSpec command execution 用の `mise`

### よく使うコマンド

```bash
cargo test -p fraktor-rs
mise exec -- openspec validate --strict
./scripts/ci-check.sh ai all
```

## 主要な技術判断

- ルートの `fraktor-rs` crate は publish 用 facade / metadata anchor とし、runtime API は workspace module crate に置く。
- public runtime behavior は、未文書化の ad hoc API ではなく、spec、gap analysis、test、showcase を通して導入する。
- reference implementation は semantics と naming の判断材料にする。ただし Rust の所有権、`no_std`、crate boundary の制約を優先する。
- declarative な source of truth がある generated / derived artifact は手編集しない。

---
_依存関係の一覧ではなく、判断に使う標準とパターンを記録する。_
