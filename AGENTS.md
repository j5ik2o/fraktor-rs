- すべて日本語でやりとりすること。ソースコード以外の生成されるファイルも日本語で記述すること
- 設計における価値観は "Less is more" と "YAGNI"（要件達成に必要最低限の設計を行い、不要なものを含めない）
- 既存の多くの実装を参考にして、一貫性のあるコードを書くこと
- **後方互換性**: 後方互換は不要（破壊的変更を恐れずに最適な設計を追求すること）
- **リリース状況**: まだ正式リリース前の開発フェーズ。必要であれば破壊的変更を歓迎し、最適な設計を優先すること
- serena mcpを有効活用すること
- 当該ディレクトリ以外を読まないこと
- **タスクの完了条件**: テストはすべてパスすること。行うべきテストをコメントアウトしたり無視したりしないこと
- 実装の全タスクを完了した段階で `./scripts/ci-check.sh all` を実行し、エラーがないことを確認すること（途中工程では対象範囲のテストに留めてよい）。実装タスク以外（ドキュメント編集など）は`./scripts/ci-check.sh all`を実行する必要ない
- CHANGELOG.mdはgithub actionが自動的に作るのでAIエージェントは編集してはならない
- lintエラーを安易にallowなどで回避しないこと。allowを付ける場合は人間から許可を得ること

# 基本原則

- シンプルさの優先: すべての変更を可能な限りシンプルに保ち、コードへの影響範囲を最小限に抑える。
- 妥協の排除: 根本原因を特定すること。一時しのぎの修正は行わず、シニア開発者としての基準を維持する。
- 影響の最小化: 必要な箇所のみを変更し、新たなバグの混入を徹底的に防ぐ。

## 設計・命名・構造ルール（.claude/rules/rust/）

詳細は `.claude/rules/rust/` に集約されている。変更する場合は人間から許可を取ること：

| ファイル | 内容 |
|----------|------|
| `immutability-policy.md` | 内部可変性禁止、&mut self 原則、AShared パターン |
| `cqs-principle.md` | CQS 原則、違反判定フロー |
| `type-organization.md` | 1file1type + 例外基準、公開範囲の判断フロー |
| `naming-conventions.md` | 曖昧サフィックス禁止、Shared/Handle 命名、ドキュメント言語 |
| `reference-implementation.md` | protoactor-go/pekko 参照手順、Go/Scala → Rust 変換 |

## Dylint lint（8つ、機械的強制）

mod-file, module-wiring, type-per-file, tests-location, use-placement, rustdoc, cfg-std-forbid, ambiguous-suffix

## AI-DLC and Spec-Driven Development
@.agent/CC-SDD.md を読むこと

# Rules


# 曖昧なサフィックスを避ける

型の命名において曖昧なサフィックスを検出し、明確な命名へ導く。

## 目的

- 型・モジュール名から責務・境界・契約が即座に推測できる状態を保つ
- 曖昧な語による責務の吸い込み・肥大化・境界崩壊を防ぐ
- ドメイン語彙を優先する

## 基本原則

- 命名は「何をするか」ではなく「何であるか」を表す
- 名前は責務・境界・依存方向を最小限の語で符号化する
- プロジェクト内で意味が一意に定義できない語はサフィックスとして使わない

## 禁止サフィックス

新規命名では以下を使用しない：

| サフィックス | 問題 |
|--------------|------|
| Manager | 「Xxxに関することを全部やる箱」になる |
| Util | 「設計されていない再利用コード」 |
| Facade | 責務の境界が不明確 |
| Service | 層や責務が未整理 |
| Runtime | 何が動くのか不明 |
| Engine | 実行体の責務が不明確 |

## 責務別 命名パターン

### データ保持・管理
`*Registry`, `*Catalog`, `*Index`, `*Table`, `*Store`

### 選択・分岐・方針
`*Policy`, `*Selector`, `*Router`

### 仲介・調停・制御
`*Coordinator`, `*Dispatcher`, `*Controller`

### 生成・構築
`*Factory`, `*Builder`

### 変換・適合
`*Adapter`, `*Bridge`, `*Mapper`

### 実行・評価
`*Executor`, `*Scheduler`, `*Evaluator`

## 例外ルール

- 外部API/OSS/フレームワーク由来の名称は無理に改名しない
- 既存コードで責務が明文化されている場合のみ例外的に許容

## 判定フロー

1. 禁止サフィックスを含むか確認
2. 含む場合:
   - この名前だけで責務を一文で説明できるか？
   - 依存してよい層・してはいけない層が推測できるか？
3. できない場合は具体名への置換案を提示

## 最終チェック

「この名前だけ見て、何に依存してよいか分かるか？」

分からないなら、その名前はまだ設計途中である。


# Explain Skill Selection

スキルを使用する際は、選択したスキルとその選択理由を明示する。

## 基本原則

**スキルを呼び出す前に、どのスキルをなぜ選んだかをユーザーに説明しなければならない。**

AIはスキルを暗黙的に呼び出す傾向があるが、ユーザーはどのスキルが使われたか、なぜそのスキルが適切だったかを理解する必要がある。このルールはスキル選択の透明性を強制する。

## ルール

### MUST（必須）

- スキルを呼び出す前に、選択したスキル名を明示する
- そのスキルを選んだ理由（ユーザーのリクエストとスキルの目的の対応関係）を説明する
- 複数のスキル候補がある場合は、なぜそのスキルが最適かを述べる

### MUST NOT（禁止）

- 説明なしにスキルを呼び出す
- スキル名だけ提示して理由を省略する

## 説明のフォーマット

スキル呼び出し前に以下の形式で説明する：

```
スキル: [スキル名]
目的: [このスキルを選んだ理由。ユーザーのリクエストとスキルの機能がどう対応するか]
```

## 例

```
# 良い例
スキル: parse-dont-validate
目的: バリデーション関数の改善リクエストに対し、型で不変式を保証するパターンへの変換を支援するため

# 良い例
スキル: creating-rules
目的: プロジェクト固有のルール（.claude/rules/*.md）を新規作成するリクエストのため

# 悪い例（説明なし）
（スキルをいきなり呼び出す）

# 悪い例（理由がない）
スキル: clean-architecture
（なぜこのスキルかの説明がない）
```

## 理由

- **透明性**: ユーザーがAIの判断プロセスを理解できる
- **学習効果**: ユーザーが利用可能なスキルとその用途を学べる
- **検証可能性**: スキル選択が不適切な場合にユーザーが指摘できる


# コーディング前の学習

新しいコードを書く前に既存の実装を分析する。既存のコードベースこそがプロジェクト規約のドキュメントである。

## 基本原則

**このプロジェクトで類似のコードがどのように書かれているかを理解せずにコードを書いてはならない。**

AIは一般的なベストプラクティスに従った「教科書的に正しい」コードを書く傾向があるが、プロジェクト固有のパターンを無視しがちである。このルールは必須の分析フェーズを強制する。

## 必須ワークフロー

### 1. 類似コードの特定

何かを実装する前に、以下の条件を満たす既存のコードを見つける：
- **同じレイヤー**：リポジトリを追加するなら、他のリポジトリを見つける
- **同じ種類**：サービスを追加するなら、他のサービスを見つける
- **同じドメイン**：認証周りで作業するなら、他の認証コードを見つける
- **同じパターン**：APIエンドポイントを追加するなら、他のエンドポイントを見つける

### 2. プロジェクトパターンの抽出

2〜3個の類似実装を分析する：

| 観点 | 確認事項 |
|------|----------|
| 構造 | インターフェース + クラス？クラスのみ？関数型？ |
| 命名 | プレフィックス/サフィックス規約、ケーシングスタイル |
| 依存関係 | 依存性はどのように注入されるか？ |
| エラー処理 | 例外？Result型？エラーコード？ |
| テスト | テストファイルの場所、命名、パターン |
| インポート | 絶対パス？相対パス？ |

### 3. パターンに従って実装

分析完了後にのみ、特定したパターンに正確に一致するコードを書く。

## 禁止事項

| やってはいけないこと | 代わりにやるべきこと |
|----------------------|----------------------|
| プロジェクトが直接クラスを使用しているのにインターフェースを追加 | 既存パターンに合わせる |
| プロジェクトが手動DIを使用しているのにDIフレームワークを使用 | 手動のコンストラクタインジェクションを使用 |
| プロジェクトがシンプルなthrowを使用しているのに包括的なエラー処理を追加 | 既存のエラースタイルに合わせる |
| プロジェクトにコメントがないのにJSDocを追加 | 既存のドキュメントスタイルに従う |

## チェックリスト

新しいコードを書く前に：

1. **検索**: 2〜3個の類似する既存実装を見つける
2. **読む**: その構造、パターン、規約を学ぶ
3. **抽出**: プロジェクト固有のパターンを把握
4. **一致**: 新しいコードが特定したパターンに正確に従うようにする


# レガシーコードの一時許容と排除

短期間の移行や安全なリファクタリングのために、レガシーコードを一時的に残すことは許容する。  
ただし、作業完了時点では必ず排除することを義務化する。

## 目的

レガシーコードを使って「壊れたままの大規模リライト」にするのではなく、  
短期の移行ステップとして利用し、最終的に単一の実装に収束させる。

## ルール

1. レガシー実装の導入は、同時に削除見込みのある代替実装を伴うこと。
2. 代替実装が有効になっていることを検証できる範囲では、レガシー実装は参照されないこと。
3. PRまたはタスク完了時には、同一責務のレガシー実装を残さないこと。
4. 残す必要がある場合は、`TODO` などで期限目安を明記し、次のマイルストーンで必ず削除すること。

## 運用上の注意

- レガシーコードは「短期の過渡状態」を表すものであり、完成状態での共存を許容しない。
- 例外が必要な場合は、事前に作業計画に明記し、削除条件を明文化すること。


# Less Is More

過剰設計を避け、シンプルで保守しやすいコードを書く。

## 核心原則

### YAGNI (You Aren't Gonna Need It)

**今必要ないものは作らない。**

- ❌ 「将来使うかもしれない」機能
- ❌ 「念のため」の設定オプション
- ❌ 仮定に基づく拡張ポイント
- ✅ 現在の要件のみ実装
- ✅ 必要になったら追加

### KISS (Keep It Simple, Stupid)

**複雑さは敵。シンプルさは味方。**

- ❌ 3行で書けるコードを10行にする
- ❌ 不要なデザインパターンの適用
- ❌ 過度な階層化・抽象化
- ✅ 最も単純な解決策をまず検討
- ✅ 読みやすさ > 賢さ

### 早すぎる抽象化の回避

**3回ルール: 3回繰り返すまで抽象化しない。**

- 1回目: 直接書く
- 2回目: 直接書く（メモする）
- 3回目: パターンを確認してから抽象化を検討

## 過剰設計の兆候

| 兆候 | 問題 |
|------|------|
| 実装より設計に時間がかかる | 分析麻痺 |
| 「将来のために」が頻出 | YAGNI違反 |
| 1機能に5+ファイル | 過度な分離 |
| 設定可能な点が10+ | 過剰な柔軟性 |
| 継承階層が3+レベル | 過度な抽象化 |
| インターフェースの実装が1つだけ | 不要な抽象化 |

## 追加前チェックリスト

- [ ] 今この機能は必要か？（YAGNI）
- [ ] より簡単な方法はないか？（KISS）
- [ ] 同じコードが3回以上あるか？（抽象化判断）
- [ ] この複雑さは価値に見合うか？
- [ ] 削除するのは追加より難しいか？

## 格言

> "Perfection is achieved not when there is nothing more to add, but when there is nothing left to take away." - Antoine de Saint-Exupery


# 編集前 Dylint 実行

コード編集前に、対象モジュールのカスタム lint を先に実行してから作業する。

## 目的

- 編集前に構造制約を可視化し、手戻りを減らす
- ルール本文の無差別読み込みを避け、コンテキスト消費を抑える
- 失敗した lint だけを読んで修正方針を確定する

## 基本ルール

1. 編集前に対象モジュールへ次を実行する  
   `./scripts/ci-check.sh dylint -m <module>`
2. lint が失敗した場合のみ、該当 lint の実装・テストを読む
3. 編集後に同コマンドを再実行し、対象テストを通す
4. 全タスク完了時は `./scripts/ci-check.sh all` を通す

## 読み込み範囲

- 失敗した lint のみ読む（例: `lints/module-wiring-lint/`）
- 成功した lint は原則読まない
- 例外: lint を追加・変更するタスクでは全 lint を確認する

## 失敗時の対応

- `allow` で回避しない
- 既存設計パターンに寄せて修正する
- ルールに矛盾がある場合は、人間に確認してから進める


# Prefer Immutability

Rust以外の言語では、常に不変（immutable）なデータ操作を優先する。

## 基本原則

**データを変更せず、新しいデータを作成する。**

ミューテーション（破壊的変更）は予測困難なバグの温床となる。参照を共有するオブジェクトを変更すると、
プログラムの別の場所で予期せぬ副作用が発生する。不変性を保つことで、コードの予測可能性と安全性が向上する。

## 適用範囲

| 言語 | 適用 | 備考 |
|------|------|------|
| JavaScript/TypeScript | ✅ | スプレッド構文、`Object.freeze`、Immutable.js等 |
| Python | ✅ | タプル、frozenset、dataclass(frozen=True)等 |
| Java | ✅ | レコード、Immutableコレクション、Builderパターン |
| Kotlin | ✅ | data class、`copy()`、不変コレクション |
| Scala | ✅ | case class、`copy()`、不変コレクションがデフォルト |
| Go | ✅ | 新しい構造体を返す、スライスのコピー |
| Ruby | ✅ | `freeze`、新しいオブジェクトを返す |
| **Rust** | ❌ | 所有権システムにより安全なミューテーションが可能 |

## ルール

### MUST（必須）

- オブジェクト/構造体の更新時は、元を変更せず新しいインスタンスを返す
- 配列/リストへの追加・削除は、新しいコレクションを返す
- 関数の引数を変更しない

### MUST NOT（禁止）

- 引数として受け取ったオブジェクトのプロパティを直接変更
- グローバルな状態のミューテーション
- 配列の `push`, `pop`, `splice` 等の破壊的メソッドの使用（代替手段がある場合）

## 言語別コード例

### JavaScript / TypeScript

```javascript
// ❌ WRONG: Mutation
function updateUser(user, name) {
  user.name = name  // 引数を直接変更！
  return user
}

// ✅ CORRECT: Immutability
function updateUser(user, name) {
  return {
    ...user,
    name
  }
}
```

```javascript
// ❌ WRONG: Array mutation
function addItem(items, item) {
  items.push(item)  // 元の配列を破壊！
  return items
}

// ✅ CORRECT: New array
function addItem(items, item) {
  return [...items, item]
}
```

```javascript
// ❌ WRONG: Nested mutation
function updateAddress(user, city) {
  user.address.city = city
  return user
}

// ✅ CORRECT: Deep copy
function updateAddress(user, city) {
  return {
    ...user,
    address: {
      ...user.address,
      city
    }
  }
}
```

### Python

```python
# ❌ WRONG: Mutation
def update_user(user: dict, name: str) -> dict:
    user["name"] = name  # 引数を直接変更！
    return user

# ✅ CORRECT: Immutability
def update_user(user: dict, name: str) -> dict:
    return {**user, "name": name}
```

```python
# ❌ WRONG: List mutation
def add_item(items: list, item) -> list:
    items.append(item)  # 元のリストを破壊！
    return items

# ✅ CORRECT: New list
def add_item(items: list, item) -> list:
    return [*items, item]
```

```python
# ✅ BETTER: dataclass with frozen=True
from dataclasses import dataclass, replace

@dataclass(frozen=True)
class User:
    name: str
    age: int

def update_name(user: User, name: str) -> User:
    return replace(user, name=name)
```

### Java

```java
// ❌ WRONG: Mutation
public User updateUser(User user, String name) {
    user.setName(name);  // 引数を直接変更！
    return user;
}

// ✅ CORRECT: Immutability with Record (Java 16+)
public record User(String name, int age) {}

public User updateUser(User user, String name) {
    return new User(name, user.age());
}
```

```java
// ❌ WRONG: Collection mutation
public List<String> addItem(List<String> items, String item) {
    items.add(item);  // 元のリストを破壊！
    return items;
}

// ✅ CORRECT: New collection
public List<String> addItem(List<String> items, String item) {
    var newItems = new ArrayList<>(items);
    newItems.add(item);
    return Collections.unmodifiableList(newItems);
}

// ✅ BETTER: Stream API
public List<String> addItem(List<String> items, String item) {
    return Stream.concat(items.stream(), Stream.of(item))
                 .toList();
}
```

### Kotlin

```kotlin
// ❌ WRONG: Mutation
fun updateUser(user: MutableUser, name: String): MutableUser {
    user.name = name  // 引数を直接変更！
    return user
}

// ✅ CORRECT: data class + copy()
data class User(val name: String, val age: Int)

fun updateUser(user: User, name: String): User {
    return user.copy(name = name)
}
```

```kotlin
// ❌ WRONG: MutableList
fun addItem(items: MutableList<String>, item: String): List<String> {
    items.add(item)  // 元のリストを破壊！
    return items
}

// ✅ CORRECT: Immutable List
fun addItem(items: List<String>, item: String): List<String> {
    return items + item
}
```

### Scala

```scala
// ❌ WRONG: var + mutation
class User(var name: String, var age: Int)

def updateUser(user: User, name: String): User = {
  user.name = name  // 引数を直接変更！
  user
}

// ✅ CORRECT: case class + copy()
case class User(name: String, age: Int)

def updateUser(user: User, name: String): User = {
  user.copy(name = name)
}
```

```scala
// ✅ Scalaは不変コレクションがデフォルト
def addItem(items: List[String], item: String): List[String] = {
  items :+ item  // 新しいリストを返す
}
```

### Go

```go
// ❌ WRONG: Pointer mutation
func UpdateUser(user *User, name string) *User {
    user.Name = name  // 引数を直接変更！
    return user
}

// ✅ CORRECT: Return new struct
func UpdateUser(user User, name string) User {
    return User{
        Name: name,
        Age:  user.Age,
    }
}
```

```go
// ❌ WRONG: Slice mutation
func AddItem(items []string, item string) []string {
    return append(items, item)  // 容量次第で元を変更する可能性！
}

// ✅ CORRECT: Explicit copy
func AddItem(items []string, item string) []string {
    newItems := make([]string, len(items)+1)
    copy(newItems, items)
    newItems[len(items)] = item
    return newItems
}
```

### Ruby

```ruby
# ❌ WRONG: Mutation
def update_user(user, name)
  user[:name] = name  # 引数を直接変更！
  user
end

# ✅ CORRECT: Immutability
def update_user(user, name)
  user.merge(name: name).freeze
end
```

```ruby
# ❌ WRONG: Array mutation
def add_item(items, item)
  items << item  # 元の配列を破壊！
  items
end

# ✅ CORRECT: New array
def add_item(items, item)
  [*items, item].freeze
end
```

## 例外

以下の場合は、パフォーマンス上の理由でミューテーションを許容する：

- **大量データのバッチ処理**：ループ内で大量のオブジェクトを生成するとGC負荷が高い
- **ローカルスコープ内での一時変数**：関数外に漏れない場合
- **明示的にドキュメント化された場合**：副作用があることをコメントで明記

```javascript
// 例外: パフォーマンスが重要な場合（明示的にコメント）
function processLargeData(items) {
  // NOTE: Performance optimization - mutating in place
  const result = []
  for (const item of items) {
    result.push(transform(item))  // 許容
  }
  return result
}
```

## 理由

- **予測可能性**: 関数が引数を変更しないことが保証される
- **デバッグ容易性**: データの変更履歴を追跡しやすい
- **並行処理安全**: 共有状態のミューテーションによる競合を防ぐ
- **テスト容易性**: 入力と出力の関係が明確


# Single Type Per File

コード生成時に「1公開型 = 1ファイル」を強制する。言語を問わず適用する。

## 原則

**1つの公開型につき1つのファイルを作成する。**

## 公開型の定義

| 言語 | 公開型 |
|------|--------|
| Java/Kotlin/Scala | `public`な `class`, `trait`, `object`, `enum` |
| Rust | `pub struct`, `pub trait`, `pub enum` |
| Go | 大文字始まりの `type` |
| Python | モジュールレベルの `class` |
| TypeScript/JavaScript | `export`された `class`, `interface`, `type`, オブジェクト |
| Swift | `public class`, `public protocol`, `public enum` |
| C# | `public class`, `public interface`, `public enum` |

## ルール

### MUST（必須）

- 1つの公開型につき1つのファイルを作成
- ファイル名は公開型の名前を反映（例: `UserRepository` → `user_repository.py`）
- 既存ファイルに新しい公開型を追加しない

### ALLOWED（許可）

- 公開型に必要な**プライベート実装型**は同居可
- 公開型の**内部ネスト型**は同居可
- **sealed interface/trait**とその閉じた実装群は同居可

### MUST NOT（禁止）

- 1ファイルに複数の公開クラス/構造体/インターフェース
- 「関連しているから」という理由での型の集約

## 判断基準

1. この型は公開型か？ → Yes なら新規ファイル作成
2. 既存の公開型の内部実装か？ → Yes なら同居可
3. sealed interface/traitの閉じた実装か？ → Yes なら同居可
4. 上記以外 → 新規ファイル作成

## 理由

- ナビゲーション性の向上（ファイル名 = 型名）
- 責任の明確化（ファイル肥大化 = 設計の問題）
- Git履歴の追跡容易性


# Rust Rules


# fraktor-rs CQS 原則

## 原則

**CQS (Command-Query Separation) をできるだけ守ること。**

- **Query**: 状態を読み取る（`&self`、戻り値あり）
- **Command**: 状態を変更する（`&mut self`、戻り値なし or `Result<(), E>`）

## 判定フロー

```
1. このメソッドは状態を変更するか？
   ├─ No → &self + 戻り値（Query）
   └─ Yes → 次へ

2. 戻り値が必要か？
   ├─ No → &mut self + () または Result<(), E>（Command）
   └─ Yes → 次へ

3. CQS 違反なしでロジックが書けるか？
   ├─ Yes → 2つのメソッドに分離
   └─ No → 人間の許可を得て CQS 違反を許容
```

## 許容される違反（人間許可前提）

| ケース | 理由 |
|--------|------|
| `Vec::pop` 相当 | 読み取りだが更新が不可避 |
| `Iterator::next` | プロトコル上 `&mut self` + `Option<T>` が必要 |
| Builder パターン | メソッドチェーンのため `&mut self` を返す |

## コード例

```rust
// ❌ WRONG: CQS 違反（状態変更 + 値返却）
fn process_and_get(&mut self) -> ProcessedData {
    self.state += 1;
    ProcessedData::new(self.state)
}

// ✅ CORRECT: 分離
fn process(&mut self) {
    self.state += 1;
}
fn processed_data(&self) -> ProcessedData {
    ProcessedData::new(self.state)
}

// ✅ ACCEPTABLE: Vec::pop 相当（人間の許可前提）
// NOTE: ロジック上分離不可のため CQS 違反を許容
fn pop_item(&mut self) -> Option<Item> {
    self.items.pop()
}
```

## 禁止パターン

- `&mut self` + 戻り値を安易に使用
- 「便利だから」という理由で CQS 違反
- 内部可変性で `&self` + 戻り値に変更して CQS 違反を隠蔽


# fraktor-rs 内部可変性ポリシー

## 原則

**内部可変性をデフォルトでは禁止する。可変操作はまず `&mut self` で設計すること。**

`&self` メソッド + 内部可変性を安易に使うと Rust の借用システムの価値が失われる。

## 判定フロー

```
1. この型は共有される必要があるか？
   ├─ No → &mut self で設計（第1選択）
   └─ Yes → 次へ

2. 状態変更メソッドが必要か？
   ├─ No → ArcShared<T> で共有（読み取り専用）
   └─ Yes → AShared パターンを新設（第2選択）

AShared パターン:
  inner に ArcShared<ToolboxMutex<A, TB>> を保持する AShared 構造体を新設
  → 詳細は docs/guides/shared_vs_handle.md を参照
```

## ルール

### trait の `&mut self` メソッド

- セマンティクスを重視した設計になっている
- 戻り値を返さないで状態を変えるメソッドは `&self` ではなく `&mut self` が原則
- 安易に `&self` + 内部可変性にリファクタリングしないこと
- **変更する場合は人間から許可を取ること**

### AShared パターン（内部可変性の唯一の許容ケース）

`&mut self` メソッドを持つ型 A が複数箇所から共有される場合のみ許容：

```rust
// ロジック本体: &mut self
pub struct XyzGeneric<TB: RuntimeToolbox> { /* state */ }

impl<TB: RuntimeToolbox> XyzGeneric<TB> {
    pub fn do_something(&mut self, arg: T) -> Result<()> { /* logic */ }
    pub fn snapshot(&self) -> Snapshot { /* read-only */ }
}

// 共有ラッパー: 内部可変性はここだけ
#[derive(Clone)]
pub struct XyzSharedGeneric<TB: RuntimeToolbox> {
    inner: ArcShared<ToolboxMutex<XyzGeneric<TB>, TB>>,
}
```

### 命名

- 薄い同期ラッパー → `*Shared`
- ライフサイクル/管理責務 → `*Handle`
- 所有権一意・同期不要 → サフィックスなし

## 禁止パターン

- 既存の `&mut self` trait メソッドを `&self` + 内部可変性に変更（人間許可なし）
- 共有不要な型に `ArcShared<ToolboxMutex<T>>` を使用
- `AShared` パターン適用時に元の型を削除
- ガードやロックを外部に返す（ロック区間はメソッド内に閉じる）


# fraktor-rs 命名規約

## 原則

**名前は責務・境界・依存方向を最小限の語で符号化する。曖昧な名前は設計が未完成であることを示す。**

## 禁止サフィックス（ambiguous-suffix-lint で機械的に強制）

| サフィックス | 問題 | 代替案 |
|--------------|------|--------|
| Manager | 「全部やる箱」になる | Registry, Coordinator, Dispatcher, Controller |
| Util | 設計されていない再利用コード | 具体的な動詞を含む名前（例: DateFormatter） |
| Facade | 責務の境界が不明確 | Gateway, Adapter, Bridge |
| Service | 層や責務が未整理 | Executor, Scheduler, Evaluator, Repository, Policy |
| Runtime | 何が動くのか不明 | Executor, Scheduler, EventLoop, Environment |
| Engine | 実行体の責務が不明確 | Executor, Evaluator, Processor, Pipeline |

### 例外

- 外部 API / OSS / フレームワーク由来の名称は `#[allow(ambiguous_suffix::ambiguous_suffix)]` で明示的に許可

### 判定フロー

```
1. 禁止サフィックスを含むか？
   ├─ No → OK
   └─ Yes → 次へ

2. この名前だけで責務を一文で説明できるか？
   ├─ Yes → 外部API由来なら #[allow] で許可
   └─ No → 代替案テーブルから具体名を選ぶ
```

## Shared / Handle 命名

| サフィックス | 用途 | 条件 |
|--------------|------|------|
| `*Shared` | 薄い同期ラッパー | `ArcShared<ToolboxMutex<T>>` を内包するだけ |
| `*Handle` | ライフサイクル / 管理責務 | 起動・停止・リソース解放・複数構成要素の束ね |
| サフィックスなし | 所有権一意・同期不要 | `ArcShared` やロックを持たない |

### 詳細

- `*Shared` は `SharedAccess` 準拠の `with_read` / `with_write` に API を絞る
- `*Handle` も基本は `with_write` / `with_read` を提供し、複合操作をまとめる
- 管理対象が複数の場合は `*HandleSet` / `*Context` で「束ね役」であることを明示
- 詳細は `docs/guides/shared_vs_handle.md` を参照

## 責務別命名パターン

| 責務 | 推奨パターン |
|------|------------|
| データ保持・管理 | `*Registry`, `*Catalog`, `*Index`, `*Table`, `*Store` |
| 選択・分岐・方針 | `*Policy`, `*Selector`, `*Router` |
| 仲介・調停・制御 | `*Coordinator`, `*Dispatcher`, `*Controller` |
| 生成・構築 | `*Factory`, `*Builder` |
| 変換・適合 | `*Adapter`, `*Bridge`, `*Mapper` |
| 実行・評価 | `*Executor`, `*Scheduler`, `*Evaluator` |

## ファイル・ディレクトリ・型の命名

| 対象 | 規約 | 例 |
|------|------|-----|
| ファイル | `snake_case.rs` | `actor_cell.rs` |
| ディレクトリ | `snake_case/` | `actor_cell/` |
| 型 / trait | `PascalCase` | `ActorCell` |
| クレート | `fraktor-<domain>-rs` | `fraktor-actor-rs` |
| Cargo features | `kebab-case` | `tokio-executor` |
| TB ジェネリクス付き | `*Generic` サフィックス | `ActorCellGeneric<TB>` |

## ドキュメント言語

- rustdoc（`///`, `//!`）→ 英語
- それ以外のコメント・Markdown → 日本語

## 最終チェック

「この名前だけ見て、何に依存してよいか分かるか？」

分からないなら、その名前はまだ設計途中である。


# fraktor-rs 参照実装からの逆輸入手順

## 原則

**protoactor-go と Apache Pekko を参照しつつ、Rust の所有権と no_std 制約に合わせた最小 API を優先する。**

## 参照実装の位置

| 実装 | パス | 言語 |
|------|------|------|
| protoactor-go | `references/protoactor-go/` | Go |
| Apache Pekko | `references/pekko/` | Scala/Java |

## 逆輸入ワークフロー

```
1. 概念の特定
   対象機能に対応する参照実装のソースを特定する
   ├─ protoactor-go: Go のチャネル・goroutine ベースの設計
   └─ pekko: Scala の trait 階層・型クラスベースの設計

2. 型数の比較
   参照実装の公開型数を数え、fraktor-rs が同等以上に多い場合は過剰設計を疑う
   目安: fraktor-rs の公開型数 ≤ 参照実装の 1.5 倍

3. Rust イディオムへの変換
   ├─ Go goroutine + channel → async + mailbox
   ├─ Go interface{} → Rust の型パラメータまたは dyn Trait
   ├─ Scala trait 階層 → Rust trait + 合成（継承より合成）
   ├─ Scala implicit → Rust ジェネリクス + RuntimeToolbox
   └─ Scala Actor DSL → Rust Behavior パターン

4. no_std 制約の適用
   ├─ ヒープ割り当て → ArcShared / heapless を検討
   ├─ std 依存 → std モジュールに隔離
   └─ スレッド → ToolboxMutex で抽象化

5. 最小 API の原則
   ├─ 参照実装の全機能を移植しない
   ├─ 現在の要件で必要な機能のみ
   └─ YAGNI: 使われていない機能は作らない
```

## 変換時の注意点

### Go → Rust

| Go パターン | Rust パターン |
|-------------|--------------|
| `interface{}` | `dyn Any` / 型パラメータ `T` |
| `func(ctx Context)` | `&mut self` メソッド |
| `go func()` | `spawn` / async task |
| `chan T` | mailbox / mpsc channel |
| `sync.Mutex` | `ToolboxMutex` |
| `struct embedding` | trait 実装 + 委譲 |

### Scala/Pekko → Rust

| Pekko パターン | Rust パターン |
|----------------|--------------|
| `trait Actor` | `BehaviorGeneric<TB, M>` |
| `ActorRef[T]` | `TypedActorRefGeneric<TB, M>` |
| `Props` | `PropsGeneric<TB>` |
| `Supervision Strategy` | `SupervisorStrategyGeneric<TB>` |
| `implicit ActorSystem` | `TB: RuntimeToolbox` パラメータ |
| `sealed trait` + case classes | `enum` |
| `akka.pattern.ask` | `ask` Future |

## 比較レビューの実施タイミング

- 新機能の設計開始時
- 型の過剰設計が疑われるとき（`reviewing-fraktor-types` スキルと併用）
- 命名に迷ったとき（参照実装の命名を確認）

## 禁止パターン

- 参照実装の設計をそのまま移植（言語特性の違いを無視）
- 「pekko にあるから」という理由だけで型や機能を追加（YAGNI）
- 参照実装を読まずに独自設計を進める（先行事例の無視）


# fraktor-rs 型配置ルール

## 原則

**1つの公開型につき1つのファイルを作成する（type-per-file-lint で機械的に強制）。**

ただし、以下の判定フローに従い、例外として同居を許可する場合がある。

## lint との関係

現在の `type-per-file-lint` はこの例外基準を認識しない（全公開型に分離を強制する）。同居が妥当と判断した場合は **人間に相談し、lint エラーへの対処方針を確認すること**。

## 判定フロー

```
1. この型は公開型（pub struct / pub trait / pub enum）か？
   ├─ No → 同居可（プライベート型は制約なし）
   └─ Yes → 次へ

2. 以下の除外対象に該当するか？
   - エラー型（*Error, *Failure）→ 常に独立ファイル
   - Shared/Handle 型 → 常に独立ファイル
   - テスト対象となる型 → 常に独立ファイル
   - ドメインプリミティブ（newtype）→ 常に独立ファイル
   ├─ 該当 → 独立ファイルに分離（例外不可）
   └─ 非該当 → 次へ

3. 以下の同居条件をすべて満たすか？
   a) 型が ≤20行（※計測基準を参照）
   b) 親型のフィールド・メソッド引数・戻り値としてのみ使われている
   c) 他のモジュールから直接参照されない（mcp__serena__find_referencing_symbols で確認）
   d) 同居先ファイルが同居後も 200行 を超えない
   ├─ すべて Yes → 同居可
   └─ 1つでも No → 独立ファイルに分離
```

## 除外対象の理由

| 型の種類 | 理由 |
|----------|------|
| エラー型（`*Error`, `*Failure`） | 独自の `From` / `Display` / `Error` 実装が伸びる |
| Shared/Handle 型 | 独自の同期責務・ライフサイクル責務を持つ |
| テスト対象となる型 | `<name>/tests.rs` との紐づけが曖昧になる |
| ドメインプリミティブ（newtype） | 独立した型安全性を提供する単位 |

## 同居条件の補足

### a) ≤20行の計測基準

以下をすべて含めて20行以下であること：
- `///` doc コメント
- `#[derive(...)]` 等の属性マクロ
- 型定義本体
- 関連する `impl` ブロック（ある場合）

### b) 「親型のフィールド・メソッド引数・戻り値としてのみ使われている」の確認方法

`mcp__serena__find_referencing_symbols` で参照元を調査し、すべての参照が親型の定義内（フィールド型、メソッドシグネチャ）に限定されていること。

## コード例

```rust
// ✅ 同居可: TickDriverConfig (親型) + TickMetricsMode (≤20行, フィールド型としてのみ使用)
// tick_driver_config.rs

/// Configuration for tick driver.
pub struct TickDriverConfig {
    kind: TickDriverKind,
    metrics_mode: TickMetricsMode,
}

/// Metrics publishing strategy (used only within TickDriverConfig).
pub enum TickMetricsMode {
    AutoPublish { interval: Duration },
    OnDemand,
}
```

```rust
// ❌ 同居不可: エラー型は除外対象（ステップ2で判定）
// tick_driver_error.rs

/// Errors during tick driver operation.
pub enum TickDriverError {
    AlreadyRunning,
    NotStarted,
    ConfigInvalid(String),
}

impl fmt::Display for TickDriverError { /* ... */ }
impl std::error::Error for TickDriverError {}
```

```rust
// ❌ 同居不可: ドメインプリミティブは除外対象（ステップ2で判定）
// tick_driver_id.rs

/// Unique identifier for a tick driver instance.
pub struct TickDriverId(u64);
```

## 禁止パターン

- 「関連しているから」という理由だけでの型の集約（判定フロー3の条件をすべて確認すること）
- 200行超のファイルへの型の追加
- 除外対象（エラー型・Shared型・Handle型・ドメインプリミティブ）の同居
- lint の `#[allow]` による type-per-file-lint の無効化（人間の許可なしで）

根拠: `claudedocs/actor-module-overengineering-analysis.md`（Phase 1-4 の分析実績）

