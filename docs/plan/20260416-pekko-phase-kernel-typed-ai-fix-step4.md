# ai_fix 計画

## 対象
- `AI-REV-004`

## family_tag
- silent-fallback / fail-fast

## 方針
1. `PoolRouter` の smallest-mailbox 経路で、`select_observed` が返した routee を同じ入力 slice から再解決する箇所の silent fallback を除去する。
2. routee index 解決を不変条件として扱い、矛盾時は panic で即時に表面化させる。
3. 変更範囲に限定した unit test を追加し、broken invariant が黙って吸収されないことを確認する。

## 検証
- `./scripts/ci-check.sh ai test -m actor-core --lib pool_router`
