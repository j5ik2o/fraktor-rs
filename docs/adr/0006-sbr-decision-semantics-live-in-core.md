# SBR decision semantics は core に置く

Split Brain Resolution (スプリットブレイン解決) の decision semantics は `cluster-core` / downing-provider contract に置き、std/provider code は lifecycle binding と具体的な lease backend integration を所有する。core は port-style vocabulary 経由で lease acquisition outcome を受け取ることで、`no_std` 下でも downing policy を testable に保ち、provider ごとに strategy semantics を再実装して分岐することを防ぐ。

**Considered Options**

- provider ごとに Split Brain Resolution (スプリットブレイン解決) の strategy semantics を実装する案: backend 差分と policy 差分が混ざり、同じ membership snapshot でも provider ごとに判断が分岐しうるため不採用。
- std/provider code に lease majority の判断まで置く案: concrete lease backend との接続は単純になるが、`no_std` core で downing policy を検証できなくなるため不採用。
- core に decision semantics を置き、provider は lease acquisition outcome を渡す案: policy を一箇所で検証でき、std/provider は lifecycle binding と backend integration に集中できるため採用。
