# Proposal: actor-test-pyramid

## Background

`modules/actor` は Pekko 公開 API / 主要セマンティクスの対応率が高くなり、`scripts/coverage.sh` でも actor 系の実カバレッジを観測できる状態になった。一方で、現状のテストは leaf 型・局所状態機械の単体テストが厚く、Pekko `actor` / `actor-typed` のユーザー可視契約を横断的に固定するテスト層はまだ体系化されていない。

このままカバレッジ数値だけを追うと、実装済みの細部は増えるが、Pekko 由来の契約漏れ、層間の接続漏れ、typed と classic の意味論差分を検出しにくい。特に actor は mailbox、dispatcher、lifecycle、supervision、death watch、event stream、scheduler、typed behavior が相互に噛み合って初めて意味を持つため、テストピラミッドとして層ごとの責務を定義する必要がある。

`references/pekko` submodule 更新後、Pekko classic / typed の参照実装とテスト資産を確認できる状態になった。特に classic は `actor-tests/src/test/scala/org/apache/pekko/actor/`、typed は `actor-typed-tests/src/test/scala/org/apache/pekko/actor/typed/` と `actor-testkit-typed/src/test/scala/org/apache/pekko/actor/testkit/typed/` に契約テストがまとまっている。実装フェーズではこれらを、既存 `docs/gap-analysis/actor-gap-analysis-evidence.md` と openspec の行単位参照と突き合わせながら、Rust の観測可能 contract へ翻訳する。

## Goal

`modules/actor-core` と `modules/actor-adaptor-std` に対し、Pekko 互換性を守るためのテストピラミッドを導入する。

- leaf 型・純粋関数の unit test は既存の `foo.rs` + `foo/tests.rs` 配置を維持する。
- Pekko の contract を Rust の公開 API / state machine に翻訳した contract test 層を追加する。
- actor system を実際に動かす integration test 層と、ユーザー操作に近い E2E scenario 層を分け、少数の高価値ケースに絞って追加する。
- gap-analysis や過去差分の再発防止は層ではなく横断タグとして扱い、Unit / Contract / Integration / E2E のいずれかへ配置する。
- coverage report を「どこが低いか」だけでなく「どの contract 層が薄いか」を見つける入口として使う。
- Wave 1 の coverage 目標を、現行 baseline (Function 83.79% / Line 83.36% / Region 82.74%) から Function 85% / Line 85% / Region 84% へ引き上げる。
- test-support / fixture / helper の配置と命名を決め、今後の actor 実装漏れを見つけやすくする。

## Non-Goals

- Pekko の Scala テストスイートを丸ごと移植しない。
- JVM / ScalaTest / TestKit 固有のアサーション、thread scheduling 前提、reflection 前提を Rust にそのまま持ち込まない。
- coverage 100% を目的化しない。未到達行の全埋めより、Pekko 契約の薄い層を優先する。
- remote / cluster 連携に依存する AC-M4b は本 change では実装しない。必要なら remote / cluster 側の change と連携する。
- `actor-core` に `std` 依存の test helper を入れない。時間・tokio・thread を使う検証は `actor-adaptor-std` または integration test に寄せる。

## Approach

テストピラミッドを実行粒度で 4 層に分ける。

1. **Unit 層**: 既存の型単位テストを維持し、境界値・不変条件・Pekko 由来の純粋契約を追加する。
2. **Contract 層**: Pekko `actor` / `actor-typed` の公開契約を、Rust の API から直接検証する。例: dispatcher id、mailbox overflow、supervision directive、typed `Behavior` 遷移、receive timeout marker、FSM timer。
3. **Integration 層**: `ActorSystem` / `TypedActorSystem` / std adaptor を起動し、複数 module の接続を検証する。
4. **E2E 層**: public API だけを使い、spawn → send / ask → watch → stop → terminate → observable event のようなユーザー操作に近い scenario を検証する。

Conformance / Regression は独立したピラミッド層ではなく、gap-analysis や過去 PR で塞いだ Pekko 差分を追跡する横断タグとして扱う。ID 付きテストは Unit / Contract / Integration / E2E のいずれかへ配置し、新しい Pekko 参照調査で見つかった契約はまず対応する層へ pin する。

実装は最初にテスト目録と fixture 方針を作り、次に coverage が低い領域を起点に Contract / Integration 層を追加する。各テストは `Pekko reference`、`fraktor module`、`contract id` のいずれかをコメントまたはテスト名で辿れるようにする。
