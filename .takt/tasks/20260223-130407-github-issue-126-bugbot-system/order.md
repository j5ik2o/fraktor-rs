## GitHub Issue #126: 🐛 [BugBot] SystemQueue CAS fallback can reorder messages

> **BugBot が PR #117 で検出したバグ**
> https://github.com/j5ik2o/fraktor-rs/pull/117

---

### SystemQueue CAS fallback can reorder messages

**Medium Severity**

<!-- DESCRIPTION START -->
On `pending.compare_exchange` failure, `return_to_head()` pushes the FIFO chain back onto the LIFO `head` one node at a time. This likely reverses relative ordering for that batch, so subsequent pops can deliver system messages out of FIFO order under contention.
<!-- DESCRIPTION END -->

<!-- BUGBOT_BUG_ID: 29287ae6-71bd-4881-8505-d68c45f7c0c6 -->

<!-- LOCATIONS START
modules/actor/src/core/dispatch/mailbox/system_queue.rs#L73-L104
LOCATIONS END -->
<p><a href="https://cursor.com/open?data=eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImJ1Z2JvdC12MiJ9.eyJ2ZXJzaW9uIjoxLCJ0eXBlIjoiQlVHQk9UX0ZJWF9JTl9DVVJTT1IiLCJkYXRhIjp7InJlZGlzS2V5IjoiYnVnYm90OmU0ZWFmNjJmLTdjYjktNGE0NC1hNzdjLWMzMzc2ZTk3YTRiYiIsImVuY3J5cHRpb25LZXkiOiI1cTZIQ0pUX1NPam1oRzJremZmVEN3NENYeDRwZ1BDdDVxTThMZXh4amhvIiwiYnJhbmNoIjoicmVmYWN0b3ItMDItMjIiLCJyZXBvT3duZXIiOiJqNWlrMm8iLCJyZXBvTmFtZSI6ImZyYWt0b3ItcnMifSwiaWF0IjoxNzcxODQ1ODI0LCJleHAiOjE3NzQ0Mzc4MjR9.TjUueCvzz1_bmNDJ5wA1kbC0dW6KQupI9MQUBpInjG-MvEsrdVVfb_4btxHSJ245e1k1ZJaceJlw-Q7GXC3yb4qNB41OV8VknwPbqMUx5xVk-EQxHQ0B1oReHlgsQnRAmgtsr4JLt-p0Qc-taHaM1QWQ96LUXAjXnZ-BI6wL7atAyjvBwlC-9Wnw2_XAq110SL1iFnyNOeio1iFCXnLTgmRJ-NM4eJwCDbSd5h67LKPb2Miv5NaFgQdovoOuOGTapEyo58UBEX5CtcBYnvYveSOb3jrTxLf3BaHZCR5uXnuMUdf29tSWanBItzL64_u_LUjkjGbK3utRv0LZm1YHDQ" target="_blank" rel="noopener noreferrer"><picture><source media="(prefers-color-scheme: dark)" srcset="https://cursor.com/assets/images/fix-in-cursor-dark.png"><source media="(prefers-color-scheme: light)" srcset="https://cursor.com/assets/images/fix-in-cursor-light.png"><img alt="Fix in Cursor" width="115" height="28" src="https://cursor.com/assets/images/fix-in-cursor-dark.png"></picture></a>&nbsp;<a href="https://cursor.com/agents?data=eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImJ1Z2JvdC12MiJ9.eyJ2ZXJzaW9uIjoxLCJ0eXBlIjoiQlVHQk9UX0ZJWF9JTl9XRUIiLCJkYXRhIjp7InJlZGlzS2V5IjoiYnVnYm90OmU0ZWFmNjJmLTdjYjktNGE0NC1hNzdjLWMzMzc2ZTk3YTRiYiIsImVuY3J5cHRpb25LZXkiOiI1cTZIQ0pUX1NPam1oRzJremZmVEN3NENYeDRwZ1BDdDVxTThMZXh4amhvIiwiYnJhbmNoIjoicmVmYWN0b3ItMDItMjIiLCJyZXBvT3duZXIiOiJqNWlrMm8iLCJyZXBvTmFtZSI6ImZyYWt0b3ItcnMiLCJwck51bWJlciI6MTE3LCJjb21taXRTaGEiOiI2MmI1OWUzOTQ0ODQzODM5ZmI2YzVjMGUzODAzYzkyOTdhOGYyODA1IiwicHJvdmlkZXIiOiJnaXRodWIifSwiaWF0IjoxNzcxODQ1ODI0LCJleHAiOjE3NzQ0Mzc4MjR9.yzReeM7f06B0cS3Py3mU8Writrt03xRP5Oq_SHu__N7wYVNCtfLl5HJBGYTzEoacEH-WmbLNdReVlmHW3KfFIAHu6mx2jyO8EjBp44BsS6aw_rFf1rJq12iYUiIap72RLdvT6EiDmLOhshXjOg2j8YJFyw5xcsl-ICjdqGRhp2RTH_bALvAdEPrKWfLb7sI_y0PFc1DMo-965a2cHTwmZmP6_2swgJHbF0Pl2NE1ZFqC0U68bF7UQONbGCCY35lnjzgo3hBS0tmquEMPbfsH3BnODSNLvWM8Z8zgwu0rYAo4IXpbjUF4UvJMVddW3G6TI_SmGPNO3M7RrDPIBxGNfQ" target="_blank" rel="noopener noreferrer"><picture><source media="(prefers-color-scheme: dark)" srcset="https://cursor.com/assets/images/fix-in-web-dark.png"><source media="(prefers-color-scheme: light)" srcset="https://cursor.com/assets/images/fix-in-web-light.png"><img alt="Fix in Web" width="99" height="28" src="https://cursor.com/assets/images/fix-in-web-dark.png"></picture></a></p>



---

**元コメント**: https://github.com/j5ik2o/fraktor-rs/pull/117#discussion_r2840335413


### Labels
bug