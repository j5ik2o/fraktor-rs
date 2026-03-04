# Layer 3: 設計比較

`{task}` で指定されたモジュールについて、Layer 2 の比較結果（`01-interface-comparison.md`）を入力とし、
**設計・アーキテクチャレベルの等価性** を評価する。

## やらないこと (Do Not)
- ビルドコマンド（`cargo check` / `cargo build` / `cargo test`）を実行しないこと
- このムーブメントはソースコードの静的解析のみを行う

## 入力

- `00-api-inventory.md`（Layer 1 出力）
- `01-interface-comparison.md`（Layer 2 出力）

## 手順

### 1. トレイト階層・型階層の比較

- Pekko側の `trait` 継承階層を抽出（`extends` / `with` 関係）
- fraktor-rs側の `trait` 境界・`impl` 関係を抽出
- 階層構造の等価性を評価：
  - Scala の継承 → Rust のトレイト合成（composition over inheritance）は設計上正当
  - 不足しているトレイト境界があれば指摘

### 2. 抽象化パターンの比較

| Pekko パターン | Rust での期待される対応 |
|----------------|----------------------|
| sealed trait + case class | `enum` バリアント |
| type class (implicit) | ジェネリクス + トレイト境界 |
| Actor DSL (Behaviors) | `Behavior<M>` パターン |
| Props / factory | Builder / `Props` |
| ActorRef[T] | `TypedActorRef<M>` |
| Extension | trait + 型パラメータ |

- パターンの選択が妥当かどうかを評価
- 過剰な抽象化（Pekkoにない層が追加されている）を検出

### 3. エラーモデルの比較

- Pekko側のエラー型・例外階層を抽出
- fraktor-rs側のエラー型・`Result` 使用パターンを確認
- エラーの回復可能性モデルが等価かを評価

### 4. ライフサイクル・リソース管理の比較

- Pekko側の `preStart` / `postStop` / `preRestart` 等のフックを列挙
- fraktor-rs側の対応するシグナルハンドリングを確認
- 監視戦略（Supervision Strategy）の対応を確認

### 5. 設計ギャップの評価

各ギャップに以下の観点で難易度を付与：

| 難易度 | 基準 |
|--------|------|
| trivial | 型エイリアスの追加、定数の追加など |
| easy | 既存パターンに沿った単一型の追加 |
| medium | 新しいトレイト定義や複数型の連携が必要 |
| hard | アーキテクチャ変更を伴う、または no_std 制約との調整が必要 |
| n/a | JVM固有・Akka互換・deprecated・YAGNI |

## 出力ルール

- **設計判断の根拠を明記**: 「fraktor-rsの方が過剰」「Pekkoの機能が不足」等の判断には必ずコード参照を付与
- **YAGNI 評価を含める**: Pekko にあるが fraktor-rs で不要な機能（使用頻度が低い・JVM固有・歴史的理由）を識別
- **推奨事項には優先度をつける**: Phase 1（trivial）〜 Phase 4（hard）+ 対象外（n/a）

## 判定

- 設計比較を出力完了: `設計比較完了`
- Layer 2 レポートが不足・不整合: `入力不備`
