## GitHub Issue #124: 🐛 [BugBot] Title extraction fails on empty comment

> **BugBot が PR #123 で検出したバグ**
> https://github.com/j5ik2o/fraktor-rs/pull/123

---

### Title extraction fails on empty comment

**Medium Severity**

<!-- DESCRIPTION START -->
The “Extract issue title” step can fail when `github.event.comment.body` is empty or only whitespace because `grep -m1 -v` returns exit code `1` and GitHub Actions runs bash with `-e`/`pipefail`. That aborts the job before the default title fallback (`${title:-...}`) is written to `GITHUB_OUTPUT`.
<!-- DESCRIPTION END -->

<!-- BUGBOT_BUG_ID: fa1eb744-7253-45eb-a9c8-81d603b741d3 -->

<!-- LOCATIONS START
.github/workflows/bugbot-to-issue.yml#L36-L45
LOCATIONS END -->
<p><a href="https://cursor.com/open?data=eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImJ1Z2JvdC12MiJ9.eyJ2ZXJzaW9uIjoxLCJ0eXBlIjoiQlVHQk9UX0ZJWF9JTl9DVVJTT1IiLCJkYXRhIjp7InJlZGlzS2V5IjoiYnVnYm90OmVkZTM3NmI2LTA3ZTgtNDhiMS05MjE4LTM3Yzk5MGFmODY0MyIsImVuY3J5cHRpb25LZXkiOiJmWmU3XzY0aFN6Wkg1Z2Z5V3pjLWluNVBNRk45V2pEMXcxWnhwLWo2cWpBIiwiYnJhbmNoIjoidG8taXNzdWUteWFtbCIsInJlcG9Pd25lciI6Imo1aWsybyIsInJlcG9OYW1lIjoiZnJha3Rvci1ycyJ9LCJpYXQiOjE3NzE4NDUzNDgsImV4cCI6MTc3NDQzNzM0OH0.lNjbryo9pr9ct_SZkptxjYUgkDjC3OrV20hR_yTYQay7iP0o8NK2mJ2Sy_OoHqSG9iUN8Xh_4WbelmGAlJZeihkORG_2De2Dh1VAGagIHE0im4uli5KiyBQx0cdC8wo-SH1uhf4f7w2HRCEY80txjg8s2rSzv9M_kf7IcRpwDejkVsKHzoD1OmhEZA9uJsRETGme7J6EykWbtyNDvHH9YQZmGDyK-AmCJgx-luO7NDmuBJxEO8Hpq21sgYcl-EZjfihMBeQYOR5UIEySFCtmx1esYtxylRMcR_PosfeAtIQIdHtzf4jwLWq1GHIzUUE65e_csDksEAGJ8lhDr8s-nw" target="_blank" rel="noopener noreferrer"><picture><source media="(prefers-color-scheme: dark)" srcset="https://cursor.com/assets/images/fix-in-cursor-dark.png"><source media="(prefers-color-scheme: light)" srcset="https://cursor.com/assets/images/fix-in-cursor-light.png"><img alt="Fix in Cursor" width="115" height="28" src="https://cursor.com/assets/images/fix-in-cursor-dark.png"></picture></a>&nbsp;<a href="https://cursor.com/agents?data=eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImJ1Z2JvdC12MiJ9.eyJ2ZXJzaW9uIjoxLCJ0eXBlIjoiQlVHQk9UX0ZJWF9JTl9XRUIiLCJkYXRhIjp7InJlZGlzS2V5IjoiYnVnYm90OmVkZTM3NmI2LTA3ZTgtNDhiMS05MjE4LTM3Yzk5MGFmODY0MyIsImVuY3J5cHRpb25LZXkiOiJmWmU3XzY0aFN6Wkg1Z2Z5V3pjLWluNVBNRk45V2pEMXcxWnhwLWo2cWpBIiwiYnJhbmNoIjoidG8taXNzdWUteWFtbCIsInJlcG9Pd25lciI6Imo1aWsybyIsInJlcG9OYW1lIjoiZnJha3Rvci1ycyIsInByTnVtYmVyIjoxMjMsImNvbW1pdFNoYSI6ImNhMmZmOTZiZGMwMjYzODQzMWY5MmM2NmVhNTg0Mzk3NDIxNWEzZjAiLCJwcm92aWRlciI6ImdpdGh1YiJ9LCJpYXQiOjE3NzE4NDUzNDgsImV4cCI6MTc3NDQzNzM0OH0.n6iQ--3xqIts7byYxFSaRUYAOCHwOGfRGZUhXl9J6kQf5z8jvoIIPcLJopjZ5KkdCaq1-vVGu1rEgeNL6kF_PqrhDTvZCloAUYEVyZVdZzpYunI-rMDBai3o4-F9MBDocpExw049mzuYLHqStJPtWkFND4KIRRLal-HSMXVVGYZ6B3oxn3MPWN3Q6YH6F7M0wfWySapNSl4I0KKX0jAGqsJUZT8RHwMiK44Y4lxcb_ZXUxxUm21GUBkT-B7Mik1Kniiq6qrbBa5L6uf62K-sPpnc3fo5jT6TITnI01YNJtWhCECb1iU39Lopgc7v2z1A4OnvaKoSpTnE59NdHCcb7w" target="_blank" rel="noopener noreferrer"><picture><source media="(prefers-color-scheme: dark)" srcset="https://cursor.com/assets/images/fix-in-web-dark.png"><source media="(prefers-color-scheme: light)" srcset="https://cursor.com/assets/images/fix-in-web-light.png"><img alt="Fix in Web" width="99" height="28" src="https://cursor.com/assets/images/fix-in-web-dark.png"></picture></a></p>



---

**元コメント**: https://github.com/j5ik2o/fraktor-rs/pull/123#discussion_r2840300120


### Labels
bug