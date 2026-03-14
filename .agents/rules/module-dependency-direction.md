# モジュール依存方向ルール

これはレイヤードアーキテクチャの普遍的な原則であり、言語を問わず適用する。

`modules/core/typed`は`modules/core(typed以外)`に依存できる
`modules/std`は`modules/core(typed含む)`に依存できる

重要なロジックはすべて`modules/core(typed以外)`に集約する。この部分はメッセージ型をジェネリックにするとロジックが複雑化するため意図的に`untyped`になっている。
`modules/core/typed`はメッセージの型付けのための薄いラッパーとする。ただし`Behavior`などの`typed`の固有ロジックが必要な場合はこの限りではない。

