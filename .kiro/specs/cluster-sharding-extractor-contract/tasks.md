# 実装計画

- [ ] 1. 基盤: envelope と extractor 契約を定義する
- [x] 1.1 (P) メッセージ envelope 表現を実装する
  - entity id と内部メッセージの組を保持し、構築時に与えた値を参照できる envelope を定義する（内部メッセージの型を型パラメータとして保持する）
  - 構築値（entity id / 内部メッセージ）の参照一致と取り出しを検証する sibling テストが green になる
  - _Requirements:_ 1.1, 1.2, 1.3
  - _Boundary:_ ShardingEnvelope<M>
  - _Depends:_ none
- [x] 1.2 (P) 宛先導出の extractor 契約と構築時検証エラーを定義する
  - entity id 導出（導出不能は識別可能な結果で返す）・shard id 導出・内部メッセージ取り出しの3操作を持つ差し替え契約を定義する
  - 標準実装が共有する構築時検証エラー（shard 数 0 の拒否）を独立ファイルのエラー型として定義する
  - 利用者定義のテストローカル実装が契約経由で3操作を提供でき、導出不能が識別可能な結果として観測できる sibling テストが green になる
  - _Requirements:_ 2.1, 2.2, 2.3, 2.4, 2.5
  - _Boundary:_ ShardingMessageExtractor<E, M> / ShardingExtractorConfigError
  - _Depends:_ none

- [ ] 2. コア: 標準 extractor 実装群
- [x] 2.1 (P) HashCode 標準実装を実装する
  - envelope から entity id を取り出し、固定仕様のハッシュ（FNV-1a 32bit、定数を rustdoc に明記）と shard 数で決定的に shard id を導出する
  - shard 数 0 の構築拒否、同一入力 → 同一 shard id の決定性、既知ベクタによるハッシュ仕様の固定を検証する sibling テストが green になる
  - _Requirements:_ 3.1, 3.5, 3.6
  - _Boundary:_ HashCodeMessageExtractor<M>
  - _Depends:_ 1.1, 1.2
- [x] 2.2 (P) Kafka 互換 Murmur2 標準実装を実装する
  - Kafka DefaultPartitioner と同一の定数・手順で shard id を導出する（murmur2 関数は同ファイル内 private）
  - Kafka リファレンス出力が既知の entity id を与えたとき shard id が一致することを、出典コメント付きの参照ベクタ sibling テストで検証し green になる
  - 同一入力 → 同一 shard id の決定性と shard 数 0 の構築拒否も検証する
  - _Requirements:_ 3.3, 3.4, 3.5, 3.6
  - _Boundary:_ Murmur2MessageExtractor<M>
  - _Depends:_ 1.1, 1.2
- [x] 2.3 envelope なし HashCode 標準実装を実装する
  - 利用者定義の entity id 導出規則を適用し、HashCode 標準実装と同一の shard 規則（共有ハッシュ）で shard id を導出する
  - 利用者定義導出の適用、導出不能の伝搬、HashCode 実装と同一 entity id → 同一 shard id になること、および同一入力 → 同一 shard id の決定性を検証する sibling テストが green になる
  - _Requirements:_ 3.2, 3.5, 3.6
  - _Boundary:_ HashCodeNoEnvelopeMessageExtractor<M>
  - _Depends:_ 2.1

- [x] 3. 統合: 配送経路への接続点を実装する
  - kind と extractor を保持し、メッセージから宛先識別を導出して既存の grain 参照へ委譲する接続点と、専用の失敗種別（導出不能 / 識別不正 / 呼び出し失敗）を実装する
  - 実装順の示唆: 失敗種別（エラー型）を先に定義してから接続点本体に入る
  - 接続点経由で解決した宛先が、同一 kind / entity id の明示構築と同一の grain を指すことを検証する sibling テストが green になる
  - 導出不能・識別不正で送信が拒否され原因種別が観測できること、送信3操作（tell_with_sender / request / request_future）が既存経路へ委譲されることを検証する sibling テストが green になる（fixture は既存の grain 参照テストを踏襲。extractor はテストローカル実装または 2.1 の HashCode 実装を使用）
  - _Requirements:_ 4.1, 4.2, 4.3
  - _Boundary:_ ShardingRouter / ShardingDispatchError
  - _Depends:_ 1.2, 2.1

- [ ] 4. 検証: 非回帰と範囲限定を確認する
  - kernel の既存ファイル差分が配線（grain.rs）のみであることを確認する
  - 既存テストが無変更で green になり、extractor 未指定の既存送信手段が維持されていることを確認する
  - placement / shard allocation / serialization に変更がないこと、shard id を消費する配置決定コードを導入していないことを確認する
  - 対象 crate の targeted check（cargo test -p、clippy / dylint）が exit 0 で通過する
  - _Requirements:_ 4.4, 5.1, 5.2, 5.3, 5.4
  - _Boundary:_ 全体検証
  - _Depends:_ 3

## Implementation Notes

- 2.1 レビュー指摘（Suggestion）: 決定性テストは同一インスタンス2回呼びではなく、別インスタンス間（`new(n)` を2回）で書く方が強い。2.2 / 2.3 ではインスタンス横断で検証する
- `*Config*Error` 型は `fmt::Display` + `core::error::Error` 実装と Display の sibling テストが既存パターン（1.2 レビューで確立）
- `Box<dyn Fn(...)>` をフィールド/引数に直接書くと `clippy::type_complexity` で CI が fail する。private 型エイリアスで回避する（2.3 レビューで確立）
