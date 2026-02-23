## GitHub Issue #125: 🐛 [BugBot] `is_bound` semantics changed unexpectedly

> **BugBot が PR #117 で検出したバグ**
> https://github.com/j5ik2o/fraktor-rs/pull/117

---

### `is_bound` semantics changed unexpectedly

**Medium Severity**

<!-- DESCRIPTION START -->
`is_bound()` changed from “either `journal_actor_ref` or `snapshot_actor_ref` is set” to “both are set”. Any code path that relied on partial binding being considered “bound” (e.g., to prevent rebind or to gate behavior differently from `is_ready()`) may now behave incorrectly.
<!-- DESCRIPTION END -->

<!-- BUGBOT_BUG_ID: ee74b543-7046-45f6-b94b-1a24e376af02 -->

<!-- LOCATIONS START
modules/persistence/src/core/persistence_context.rs#L374-L377
LOCATIONS END -->
<p><a href="https://cursor.com/open?data=eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImJ1Z2JvdC12MiJ9.eyJ2ZXJzaW9uIjoxLCJ0eXBlIjoiQlVHQk9UX0ZJWF9JTl9DVVJTT1IiLCJkYXRhIjp7InJlZGlzS2V5IjoiYnVnYm90OmM3YzY5NjdhLWQyOTktNGMxYy1iYzRiLTQwYTM2M2UyYjI4ZiIsImVuY3J5cHRpb25LZXkiOiIzYkhlVUFrdV9YVFBIY19Sa0pPNzl4WDhiRm9uNlJ5TG1MRFRBdGFtRERBIiwiYnJhbmNoIjoicmVmYWN0b3ItMDItMjIiLCJyZXBvT3duZXIiOiJqNWlrMm8iLCJyZXBvTmFtZSI6ImZyYWt0b3ItcnMifSwiaWF0IjoxNzcxODQ1ODI0LCJleHAiOjE3NzQ0Mzc4MjR9.cEh0raBdDzGYmARGLHj08gJ5jTxMpDYyakQSxFgyW12cXaWiTu2v2GFNypUYH0fFSZpqvi-Qu_VpeBE-wWPfA0LK71tr6Hxo1hwKDSgOwrSIFInupKLT9KO4fJiZ7-QMiPBSBhjwGy2HNDyKbfOMNi_0FYhPGaFNuGR7-xwpBu2SMdY_K78K7svISlA5cXf1gDKBBS6MTtRJEcqJH1dHN-jVWqmxb37UqXdWpSKlrgBX8zu2Hps5eYq3sVLeWE_h8KAGYE6IbXd4C-yGtMVpidc2euO7Dl7WGmnbstxDDVGkI1DLmaPEGCIywvMOupHTJNQ2D1aqPykJD-Y3THbl0Q" target="_blank" rel="noopener noreferrer"><picture><source media="(prefers-color-scheme: dark)" srcset="https://cursor.com/assets/images/fix-in-cursor-dark.png"><source media="(prefers-color-scheme: light)" srcset="https://cursor.com/assets/images/fix-in-cursor-light.png"><img alt="Fix in Cursor" width="115" height="28" src="https://cursor.com/assets/images/fix-in-cursor-dark.png"></picture></a>&nbsp;<a href="https://cursor.com/agents?data=eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImJ1Z2JvdC12MiJ9.eyJ2ZXJzaW9uIjoxLCJ0eXBlIjoiQlVHQk9UX0ZJWF9JTl9XRUIiLCJkYXRhIjp7InJlZGlzS2V5IjoiYnVnYm90OmM3YzY5NjdhLWQyOTktNGMxYy1iYzRiLTQwYTM2M2UyYjI4ZiIsImVuY3J5cHRpb25LZXkiOiIzYkhlVUFrdV9YVFBIY19Sa0pPNzl4WDhiRm9uNlJ5TG1MRFRBdGFtRERBIiwiYnJhbmNoIjoicmVmYWN0b3ItMDItMjIiLCJyZXBvT3duZXIiOiJqNWlrMm8iLCJyZXBvTmFtZSI6ImZyYWt0b3ItcnMiLCJwck51bWJlciI6MTE3LCJjb21taXRTaGEiOiI2MmI1OWUzOTQ0ODQzODM5ZmI2YzVjMGUzODAzYzkyOTdhOGYyODA1IiwicHJvdmlkZXIiOiJnaXRodWIifSwiaWF0IjoxNzcxODQ1ODI0LCJleHAiOjE3NzQ0Mzc4MjR9.BamFdAA8LuSFa55iHgTvYgh78FYHgy-LDZmPSv86lJF6KGnIyVCRSyhkGHRgU32Ft6-FFVL5ko_75o6CBkp_fmJLwQBYImcPgPEXwLMR-J3-k5yXpwnamNfGcYj7J7aivZAHhj3v12XR_1JbYgj-Tu1kVdK81HFZXzZWhbDq9co4BqpDJLSdfYMZpsEXbDGFMyvslwJKihSY2-zDX0k5DT27DmdxZoHybUjeRykmA9lMVA4fgJ5CDg9IhKuH7wKP1E45GLdwje8mx2pfsffzL7mR_r_awYoBMBiaANDjF3iqpbvgHbihQsruPujKKxeDywXaspuK69C76wJc0vUVoA" target="_blank" rel="noopener noreferrer"><picture><source media="(prefers-color-scheme: dark)" srcset="https://cursor.com/assets/images/fix-in-web-dark.png"><source media="(prefers-color-scheme: light)" srcset="https://cursor.com/assets/images/fix-in-web-light.png"><img alt="Fix in Web" width="99" height="28" src="https://cursor.com/assets/images/fix-in-web-dark.png"></picture></a></p>



---

**元コメント**: https://github.com/j5ik2o/fraktor-rs/pull/117#discussion_r2840335410


### 関連 Issue（重複として統合済み）

- GitHub Issue #127: `is_bound` now requires both refs（#125 に統合しクローズ済み）
  - `is_bound()` が both を要求するようになったことで、partial binding を中間状態として使うコードパスが `PersistenceError::StateMachine` で失敗する可能性がある
  - 元コメント: https://github.com/j5ik2o/fraktor-rs/pull/117#discussion_r2840529516
  - 対象箇所: `modules/persistence/src/core/persistence_context.rs#L374-L381`

### Labels
bug