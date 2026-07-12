---
marp: true
theme: default
paginate: true
html: true
size: 16:9
title: Rustで宣言的ストリームDSLを設計する
description: 関数型まつり 2026 Track C 発表資料
style: |
  @import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;600&family=Noto+Sans+JP:wght@400;600;700;900&display=swap');

  :root {
    --bg: #f7f9fc;
    --surface: #ffffff;
    --surface-2: #eaf1f5;
    --ink: #172033;
    --muted: #4f6075;
    --cyan: #006d77;
    --orange: #ad4e00;
    --red: #b42318;
    --green: #16734a;
    --line: #c5cfda;
  }

  section {
    background:
      radial-gradient(circle at 90% 10%, rgba(0, 109, 119, 0.06), transparent 28%),
      linear-gradient(145deg, #fbfcfe 0%, #f2f6f9 100%);
    color: var(--ink);
    font-family: 'Noto Sans JP', sans-serif;
    font-size: 27px;
    line-height: 1.45;
    padding: 58px 72px 56px;
  }

  section::after {
    color: #64748b;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 16px;
  }

  h1, h2, h3, p { margin-top: 0; }

  h1 {
    color: var(--ink);
    font-size: 47px;
    font-weight: 800;
    letter-spacing: -0.03em;
    margin-bottom: 34px;
  }

  h2 {
    color: var(--cyan);
    font-size: 31px;
    font-weight: 700;
    margin-bottom: 22px;
  }

  strong { color: var(--orange); }

  code {
    background: #eef2f6;
    border: 1px solid #d3dce6;
    border-radius: 6px;
    color: #0f2942;
    font-family: 'IBM Plex Mono', monospace;
    padding: 0.08em 0.28em;
  }

  pre {
    background: #f8fafc;
    border: 1px solid #cbd5e1;
    border-left: 5px solid var(--cyan);
    border-radius: 10px;
    box-shadow: 0 14px 30px rgba(31, 41, 55, 0.12);
    padding: 22px 26px;
  }

  pre code {
    background: transparent;
    border: 0;
    color: var(--ink);
    font-size: 21px;
    line-height: 1.42;
    padding: 0;
  }

  ul, ol { padding-left: 1.25em; }
  li { margin: 0.3em 0; }
  li::marker { color: var(--cyan); }

  .title-slide {
    background:
      linear-gradient(90deg, rgba(251, 252, 254, 0.98) 0 56%, rgba(242, 246, 249, 0.86) 100%),
      repeating-linear-gradient(120deg, transparent 0 32px, rgba(0, 109, 119, 0.08) 33px 34px);
  }

  .title-slide h1 {
    font-size: 66px;
    line-height: 1.18;
    max-width: 1000px;
    margin-top: 95px;
  }

  .title-slide .subtitle {
    color: var(--cyan);
    font-family: 'IBM Plex Mono', monospace;
    font-size: 29px;
    margin-top: 26px;
  }

  .title-slide .meta {
    color: var(--muted);
    font-size: 24px;
    margin-top: 74px;
  }

  .eyebrow {
    color: var(--cyan);
    font-family: 'IBM Plex Mono', monospace;
    font-size: 19px;
    font-weight: 600;
    letter-spacing: 0.08em;
    margin-bottom: 14px;
    text-transform: uppercase;
  }

  .lead {
    font-size: 34px;
    line-height: 1.45;
    max-width: 1050px;
  }

  .muted { color: var(--muted); }
  .accent { color: var(--cyan); }
  .warn { color: var(--orange); }
  .danger { color: var(--red); }
  .good { color: var(--green); }

  .two-col {
    align-items: stretch;
    display: grid;
    gap: 38px;
    grid-template-columns: 1fr 1fr;
  }

  .two-col.wide-left { grid-template-columns: 1.25fr 0.75fr; }
  .two-col.wide-right { grid-template-columns: 0.75fr 1.25fr; }

  .profile-grid {
    align-items: center;
    display: grid;
    gap: 64px;
    grid-template-columns: 1.5fr 0.7fr;
    margin-top: 42px;
  }

  section.profile .profile-grid { margin-top: 16px; }
  section.profile li { font-size: 25px; margin: 0.18em 0; }
  section.profile li .muted.small { font-size: 19px; }

  .profile-photo {
    aspect-ratio: 1 / 1;
    background: rgba(255, 255, 255, 0.88);
    border: 1px solid var(--line);
    border-radius: 18px;
    display: block;
    object-fit: cover;
    width: 100%;
  }

  .panel {
    background: rgba(255, 255, 255, 0.92);
    border: 1px solid var(--line);
    border-radius: 14px;
    box-shadow: 0 8px 20px rgba(31, 41, 55, 0.06);
    padding: 25px 28px;
  }

  .panel h2, .panel h3 { margin-top: 0; }

  .metric-row {
    display: grid;
    gap: 18px;
    grid-template-columns: repeat(3, 1fr);
    margin-top: 30px;
  }

  .metric-row.four { grid-template-columns: repeat(4, 1fr); }

  .metric {
    border-top: 4px solid var(--cyan);
    padding-top: 14px;
  }

  .metric strong {
    color: var(--ink);
    display: block;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 52px;
    line-height: 1.1;
  }

  .metric span {
    color: var(--muted);
    font-size: 20px;
  }

  .agenda {
    display: grid;
    gap: 11px;
    grid-template-columns: repeat(3, 1fr);
    margin-top: 44px;
  }

  .agenda .item {
    background: rgba(255, 255, 255, 0.78);
    border-left: 4px solid #cad2dc;
    color: #6b778c;
    font-size: 21px;
    min-height: 78px;
    padding: 16px 18px;
  }

  .agenda .item b {
    display: block;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 16px;
    margin-bottom: 4px;
  }

  .agenda .current {
    background: rgba(0, 109, 119, 0.10);
    border-left-color: var(--cyan);
    color: var(--ink);
  }

  .flow {
    align-items: center;
    display: flex;
    gap: 13px;
    justify-content: center;
    margin: 34px 0;
  }

  .node {
    background: var(--surface-2);
    border: 1px solid #9dafc0;
    border-radius: 12px;
    color: var(--ink);
    font-family: 'IBM Plex Mono', monospace;
    font-size: 21px;
    padding: 16px 19px;
    text-align: center;
  }

  .node.hot { border-color: var(--cyan); box-shadow: inset 0 0 0 1px var(--cyan); }
  .node.boundary { border-color: var(--orange); color: var(--orange); }

  .concept-flow {
    align-items: center;
    display: flex;
    gap: 14px;
    justify-content: center;
  }

  .concept-block {
    background: var(--surface);
    border: 2px solid var(--cyan);
    border-radius: 14px;
    min-width: 180px;
    padding: 18px 22px;
    text-align: center;
  }

  .concept-block b {
    color: var(--cyan);
    display: block;
    font-size: 25px;
    margin-bottom: 5px;
  }

  .concept-block b.type { font-family: 'IBM Plex Mono', monospace; }

  section.actor-basics pre { margin-top: 34px; }

  .concept-block span { color: var(--muted); font-size: 18px; }

  .mailbox {
    display: flex;
    gap: 6px;
  }

  .mailbox span {
    background: rgba(173, 78, 0, 0.11);
    border: 1px solid var(--orange);
    height: 38px;
    width: 38px;
  }

  .compare-visual {
    display: grid;
    gap: 22px;
    margin-top: 28px;
  }

  .compare-lane {
    align-items: center;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 14px;
    display: grid;
    gap: 20px;
    grid-template-columns: 150px 1fr 270px;
    padding: 22px 26px;
  }

  .compare-lane.hot { border: 2px solid var(--orange); }
  .compare-lane h2 { margin: 0; }
  .compare-lane .lane-note { color: var(--muted); font-size: 19px; }

  .compare-lane .lane-code {
    color: #0f2942;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 16px;
    margin-top: 14px;
    text-align: center;
  }

  .pipeline {
    align-items: center;
    display: flex;
    gap: 9px;
    justify-content: center;
  }

  .pipeline .step {
    background: var(--surface-2);
    border: 1px solid #9dafc0;
    border-radius: 9px;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 17px;
    padding: 10px 12px;
    white-space: nowrap;
  }

  .pipeline .step.boundary-step { border-color: var(--orange); color: var(--orange); }

  .dispatch-path { display: grid; gap: 18px; margin-top: 22px; }

  .dispatch-lane {
    align-items: center;
    display: grid;
    gap: 10px;
    grid-template-columns: 86px 1fr 36px 1.15fr 36px 1.2fr 36px 1.2fr;
  }

  .dispatch-lane-label {
    color: var(--cyan);
    font-family: 'IBM Plex Mono', monospace;
    font-size: 18px;
    font-weight: 600;
    text-align: center;
  }

  .dispatch-box {
    align-items: center;
    background: var(--surface);
    border: 2px solid #9dafc0;
    border-radius: 11px;
    display: flex;
    flex-direction: column;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 18px;
    font-weight: 600;
    justify-content: center;
    min-height: 74px;
    padding: 9px 10px;
    text-align: center;
  }

  .dispatch-box.hot { background: rgba(173, 78, 0, 0.06); border-color: var(--orange); color: var(--orange); }
  .dispatch-box small { color: var(--muted); font-family: 'IBM Plex Mono', monospace; font-size: 13px; font-weight: 400; margin-top: 5px; }
  .dispatch-lane .arrow { font-size: 27px; text-align: center; }

  .materialize-layout {
    align-items: start;
    display: grid;
    gap: 34px;
    grid-template-columns: 1.15fr 0.85fr;
    margin-top: 14px;
  }

  .numbered.compact { gap: 10px; }
  .numbered.compact li { font-size: 19px; gap: 10px; padding: 9px 13px; }

  .materialize-diagram {
    align-items: center;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .materialize-diagram .block {
    background: var(--surface);
    border: 2px solid var(--cyan);
    border-radius: 10px;
    box-sizing: border-box;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 18px;
    padding: 12px 15px;
    text-align: center;
    width: 100%;
  }

  .materialize-diagram .block.hot { border-color: var(--orange); color: var(--orange); }
  .materialize-diagram .split { display: grid; gap: 8px; grid-template-columns: 1fr 1fr; width: 100%; }
  .materialize-diagram .down { color: var(--cyan); font-size: 23px; line-height: 1; }

  .code-focus pre { padding: 26px 30px; }
  .code-focus pre code { font-size: 26px; line-height: 1.5; }

  .arrow {
    color: var(--cyan);
    font-family: 'IBM Plex Mono', monospace;
    font-size: 31px;
  }

  .arrow .label {
    display: block;
    font-size: 13px;
    white-space: nowrap;
  }

  .layers {
    display: grid;
    gap: 13px;
    margin-top: 24px;
  }

  .layer {
    align-items: center;
    background: rgba(255, 255, 255, 0.90);
    border: 1px solid var(--line);
    border-left: 6px solid var(--cyan);
    border-radius: 12px;
    display: grid;
    gap: 25px;
    grid-template-columns: 290px 1fr 340px;
    padding: 19px 24px;
  }

  .layer b { color: var(--cyan); font-size: 25px; }
  .layer .path { font-family: 'IBM Plex Mono', monospace; font-size: 20px; }
  .layer .desc { color: var(--muted); font-size: 19px; }

  .workspace-grid {
    display: grid;
    gap: 7px;
    grid-template-columns: 176px repeat(6, 1fr);
    margin-top: 23px;
  }

  .workspace-grid > div {
    align-items: center;
    background: var(--surface);
    border: 1px solid #d2dae4;
    display: flex;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 16px;
    justify-content: center;
    min-height: 58px;
    padding: 8px;
    text-align: center;
  }

  .workspace-grid .head { background: transparent; border: 0; color: var(--muted); }
  .workspace-grid .rowhead { color: var(--cyan); flex-direction: column; font-size: 15px; }
  .workspace-grid .focus { background: rgba(0, 109, 119, 0.10); border-color: var(--cyan); color: var(--ink); }

  .big-contrast {
    align-items: center;
    display: grid;
    gap: 48px;
    grid-template-columns: 1fr auto 1fr;
    margin-top: 105px;
    text-align: center;
  }

  .big-contrast .term {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 48px;
    font-weight: 700;
    line-height: 1.25;
  }

  .big-contrast .term.compact {
    font-size: 42px;
    white-space: nowrap;
  }

  .big-contrast .neq { color: var(--red); font-size: 76px; font-weight: 900; }

  .scope-grid .panel { padding-left: 16px; padding-right: 16px; }
  .scope-grid h2 { font-size: 25px; white-space: nowrap; }

  .split-diagram {
    display: grid;
    gap: 28px;
    grid-template-columns: 0.82fr 1.18fr;
    margin-top: 20px;
  }

  .split-diagram h2 { font-size: 25px; }

  .island-row {
    align-items: center;
    display: grid;
    gap: 12px;
    grid-template-columns: 1fr auto 1fr;
  }

  .island {
    background: rgba(0, 109, 119, 0.06);
    border: 2px solid var(--cyan);
    border-radius: 15px;
    padding: 20px 16px;
    text-align: center;
  }

  .island b { color: var(--cyan); }
  .island .mini { font-family: 'IBM Plex Mono', monospace; font-size: 15px; margin-top: 10px; }

  .boundary-loop { margin-top: 18px; }

  .boundary-status {
    align-items: center;
    background: rgba(173, 78, 0, 0.08);
    border-left: 6px solid var(--orange);
    display: flex;
    gap: 28px;
    justify-content: center;
    padding: 10px 18px;
  }

  .boundary-status strong { font-family: 'IBM Plex Mono', monospace; font-size: 27px; }
  .boundary-status span { font-size: 20px; }

  .boundary-data {
    align-items: center;
    display: grid;
    gap: 12px;
    grid-template-columns: 1fr 145px 1.4fr 145px 1fr;
    margin-top: 25px;
  }

  .loop-island {
    background: rgba(0, 109, 119, 0.06);
    border: 2px solid var(--cyan);
    border-radius: 14px;
    color: var(--cyan);
    font-size: 23px;
    font-weight: 700;
    padding: 24px 12px;
    text-align: center;
  }

  .loop-island small { color: var(--muted); display: block; font-family: 'IBM Plex Mono', monospace; font-size: 16px; margin-top: 7px; }

  .transfer-step {
    align-items: center;
    color: var(--orange);
    display: grid;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 28px;
    gap: 6px;
    grid-template-columns: auto 1fr;
    justify-items: center;
    text-align: center;
  }

  .transfer-step small {
    color: var(--muted);
    font-family: 'Noto Sans JP', sans-serif;
    font-size: 15px;
    grid-column: 1 / -1;
    white-space: nowrap;
  }

  .token {
    background: var(--orange);
    border: 4px solid rgba(173, 78, 0, 0.18);
    border-radius: 50%;
    box-sizing: border-box;
    height: 34px;
    width: 34px;
  }

  .boundary-buffer {
    border: 2px solid var(--cyan);
    border-radius: 12px;
    overflow: hidden;
  }

  .buffer-caption {
    color: var(--cyan);
    font-family: 'IBM Plex Mono', monospace;
    font-size: 17px;
    padding: 8px 10px;
    text-align: center;
  }

  .buffer-bar { display: grid; grid-template-columns: 1fr 68px; height: 66px; }

  .buffer-used, .buffer-space {
    align-items: center;
    display: flex;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 19px;
    font-weight: 600;
    justify-content: center;
  }

  .buffer-used { background: rgba(0, 109, 119, 0.18); color: var(--cyan); }
  .buffer-space { background: #fff; border-left: 2px dashed var(--orange); color: var(--orange); }

  .boundary-demand {
    align-items: center;
    color: var(--cyan);
    display: flex;
    gap: 22px;
    margin: 27px auto 0;
    width: 78%;
  }

  .boundary-demand .shaft { border-top: 4px solid var(--cyan); flex: 1; position: relative; }
  .boundary-demand .shaft::before { content: "←"; font-size: 38px; left: -14px; position: absolute; top: -29px; }
  .boundary-demand b { color: var(--cyan); font-size: 21px; white-space: nowrap; }

  .state-row {
    align-items: center;
    display: grid;
    gap: 14px;
    grid-template-columns: 0.8fr auto repeat(3, 1fr);
    margin-top: 34px;
  }

  .state {
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 10px;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 18px;
    padding: 17px 12px;
    text-align: center;
  }

  .state.open { border-color: var(--green); }
  .state.terminal { border-color: var(--orange); }

  .boundary-cut {
    align-items: center;
    color: var(--orange);
    display: flex;
    flex-direction: column;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 14px;
    line-height: 1.15;
    min-width: 68px;
  }

  .boundary-cut .cut-line {
    border-left: 4px dashed var(--orange);
    height: 48px;
    margin: 5px 0;
  }

  .dispatcher-visual {
    align-items: end;
    display: grid;
    gap: 22px;
    grid-template-columns: 1fr 300px 1fr;
    margin-top: 54px;
  }

  .dispatcher-marker { text-align: center; }

  .dispatcher-marker .attribute {
    background: rgba(173, 78, 0, 0.08);
    border: 2px solid var(--orange);
    border-radius: 10px;
    color: var(--orange);
    font-family: 'IBM Plex Mono', monospace;
    font-size: 18px;
    padding: 12px 10px;
  }

  .boundary-timeline {
    align-items: stretch;
    display: flex;
    gap: 10px;
    margin-top: 38px;
  }

  .boundary-frame {
    background: var(--surface);
    border: 2px solid var(--line);
    border-radius: 12px;
    flex: 1;
    padding: 16px 14px;
    text-align: center;
  }

  .boundary-frame b { color: var(--cyan); display: block; font-size: 20px; margin-bottom: 12px; }
  .boundary-frame small { color: var(--muted); display: block; font-size: 16px; margin-top: 9px; }
  .time-arrow { align-items: center; color: var(--cyan); display: flex; font-size: 28px; }

  .mini-buffer {
    border: 2px solid var(--cyan);
    display: grid;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 16px;
    grid-template-columns: 15fr 1fr;
    height: 44px;
  }

  .mini-buffer.full { grid-template-columns: 1fr; }
  .mini-buffer .filled { align-items: center; background: rgba(0, 109, 119, 0.18); display: flex; justify-content: center; }
  .mini-buffer .empty { align-items: center; background: #fff; border-left: 2px dashed var(--orange); color: var(--orange); display: flex; justify-content: center; }
  .pending-dot { color: var(--orange); font-size: 20px; margin-top: 10px; }

  .state-choice { margin-top: 8px; text-align: center; }
  .state-choice .state.open { margin: 0 auto; width: 190px; }
  .branch-arrows { color: var(--cyan); font-family: 'IBM Plex Mono', monospace; font-size: 34px; letter-spacing: 1.8em; margin: 7px 0 4px 1.8em; }
  .terminal-options { display: grid; gap: 10px; grid-template-columns: repeat(3, 1fr); }

  .terminal-lanes { display: grid; gap: 10px; margin-top: 12px; }
  .terminal-lane { align-items: center; display: grid; gap: 18px; grid-template-columns: 150px 1fr; }
  .terminal-lane .flow { justify-content: flex-start; margin: 0; }
  .quote.tight { margin-top: 22px; }
  .panel ul.compact { padding-left: 1.1em; }
  .panel ul.compact li { font-size: 21px; margin: 0.35em 0; }
  .terminal-lane > b { color: var(--cyan); font-family: 'IBM Plex Mono', monospace; font-size: 20px; }

  .command-flow .node { font-size: 16px; padding: 14px 12px; }
  .command-flow .arrow { font-size: 25px; }

  .comparison {
    background: transparent;
    border-collapse: collapse;
    color: var(--ink);
    font-size: 21px;
    margin-top: 22px;
    width: 100%;
  }

  .comparison th, .comparison td {
    background: var(--surface) !important;
    border-bottom: 1px solid var(--line);
    padding: 14px 14px;
    text-align: left;
  }

  .comparison th { color: var(--cyan); }

  .quote {
    border-left: 7px solid var(--orange);
    font-size: 39px;
    font-weight: 700;
    line-height: 1.5;
    margin: 74px auto 0;
    max-width: 1000px;
    padding-left: 34px;
  }

  .numbered {
    counter-reset: item;
    display: grid;
    gap: 16px;
    list-style: none;
    padding: 0;
  }

  .numbered li {
    background: rgba(255, 255, 255, 0.92);
    border: 1px solid var(--line);
    border-radius: 12px;
    counter-increment: item;
    display: grid;
    gap: 18px;
    grid-template-columns: 46px 1fr;
    margin: 0;
    padding: 13px 18px;
  }

  .numbered li::before {
    color: var(--cyan);
    content: counter(item, decimal-leading-zero);
    font-family: 'IBM Plex Mono', monospace;
    font-size: 19px;
    font-weight: 600;
  }

  .small { font-size: 21px; }
  .tiny { font-size: 18px; }
  .nowrap { white-space: nowrap; }
  .center { text-align: center; }
  .no-page::after { display: none; }
  .code-compact pre code { font-size: 18px; }
---

<!-- _class: title-slide no-page -->

# Rustで宣言的ストリームDSLを設計する

<div class="subtitle">async boundary と island 実行モデル</div>

<div class="meta">関数型まつり 2026 · Track C<br>かとじゅん（@j5ik2o）</div>

<!--
[目安 30秒]
本トークでは、Rustで宣言的なストリームDSL、つまりストリーム処理を宣言的に記述するための専用の小さな言語を設計するとき、APIの背後にどのような実行系が必要になるかを扱います。題材は開発中のfraktor-rsです。
特に、async boundaryとislandという言葉が、Rustのasync/awaitとは別の実行モデルを指すことを解きほぐします。
-->

---

<!-- slide_id: S002 -->
<!-- _class: profile -->

<div class="eyebrow">01 · Intro</div>

# 自己紹介

<div class="profile-grid">
  <div>
    <p class="lead"><strong>かとじゅん（@j5ik2o）</strong></p>
    <ul>
      <li>所属：IDEO PLUS合同会社 代表社員<br><span class="muted small">2014〜2024年末は Kubell（旧Chatwork）に在籍</span></li>
      <li>主な活動：SaaS企業の技術顧問<br><span class="muted small">ZOZO / Leverages / Precena / カンリー</span></li>
      <li>言語：ながらく Scala、最近は言語問わず<br><span class="muted small">GitHub（github.com/j5ik2o）では Rust がメイン</span></li>
      <li>OSS：Amadeus（AI-DLC v2 + <span class="nowrap">選挙に基づくマルチエージェント</span>）/ TAKT / <strong>fraktor</strong><br><span class="muted small">fraktor が本トークの題材</span></li>
      <li>個人エンジニア支援：月額1万円で実施中<br><span class="muted small">キャリア相談 / モブプロ・ペアプロ / コードレビュー ほか（<a href="https://utopian-cyclamen-728.notion.site/3051c086c12c809c9662eccc50dbf132">案内ページ</a>）</span></li>
    </ul>
  </div>
  <img class="profile-photo" src="./self-profile.jpg" alt="プロフィール画像">
</div>

<!--
[目安 30秒]
IDEO PLUS合同会社の代表社員として、ZOZO、Leverages、Precena、カンリーなどSaaS企業の技術顧問をしています。2014年から2024年末まではKubell、旧Chatworkに在籍していました。
ながらくScalaをやってきましたが、最近は言語を問わず活動していて、GitHubではRustがメインです。
OSSでは、AI駆動開発ライフサイクルの新版であるAI-DLC v2と、選挙に基づくマルチエージェントを支援するAmadeusのほか、TAKT、そして本トークの題材であるfraktorを開発しています。
また、月額1万円で個人エンジニアの支援もやっています。キャリア相談、モブプロ・ペアプロ、コードレビューなどです。興味があればお声がけください。
-->

---

<div class="eyebrow">01 · Intro</div>

# fraktor-rs は、Rust のアクターランタイムである

<p class="lead">Apache Pekko / Proto.Actor の<strong>アクターモデルを Rust 向けに再設計</strong></p>

<div class="two-col" style="margin-top: 36px">
  <div class="panel">
    <h2><code>#![no_std]</code> core</h2>
    <p>組込みでもサーバでも使える<br>移植性の高い契約と状態機械</p>
  </div>
  <div class="panel">
    <h2><code>std</code> adaptor</h2>
    <p>Tokio・ネットワークなど<br>ホスト固有の実装を分離<br><span class="muted small">Tokio = Rust の代表的な非同期ランタイム</span></p>
  </div>
</div>

<p class="center muted small" style="margin-top: 22px"><code>std</code> = OS を前提とする Rust の標準ライブラリ ／ <code>no_std</code> = それに依存しない構成（組込みなど OS なしでも動く）</p>

<p class="muted" style="margin-top: 16px">pre-release。API は Pekko に学び、実行系は Rust 向けに再設計している。</p>

<!--
[目安 1分10秒]
fraktor-rsはApache PekkoとProto.Actorのアクターモデルを参照しつつ、Rustの所有権とno_std制約に合わせて再設計しているアクターランタイムです。
coreはno_std、つまりRustの標準ライブラリに依存しない構成で、ホスト環境に依存しない契約と状態機械を持ちます。Rustの代表的な非同期ランタイムであるTokioや、ネットワークへの接続はstd adaptorへ分離しています。
-->

---

<!-- _class: actor-basics -->

<div class="eyebrow">01 · Intro</div>

# actor は、状態と mailbox を持つ実行単位

<div class="concept-flow" style="margin-top: 42px">
  <div class="concept-block">
    <b>送信者</b>
    <span>メッセージを作る</span>
  </div>
  <div class="arrow">→</div>
  <div class="concept-block">
    <b class="type">ActorRef</b>
    <span>actor への送信窓口</span>
  </div>
  <div class="arrow">→</div>
  <div>
    <p class="center muted small">mailbox<br>受信待ちの列</p>
    <div class="mailbox"><span></span><span></span><span></span><span></span></div>
  </div>
  <div class="arrow"><span class="label">Mailbox::run</span>→</div>
  <div class="concept-block">
    <b>actor</b>
    <span>状態 + 処理</span>
  </div>
</div>

```rust
// 送信者は ActorRef へ送るだけ。actor 本体には触れない
cart_ref.tell(CartCommand::AddItem { item_id: 1, qty: 1 });
```

<p class="center muted" style="margin-top: 10px">mailbox runner が1件ずつ取り出し、actor のハンドラへ渡す。</p>
<p class="center muted small" style="margin-top: 12px">後半に登場する <code>Drive</code> / <code>Cancel</code> も、この mailbox へ届くコマンドである。</p>

<!--
[目安 1分20秒]
このトークで必要なactorの知識は三つだけです。actorは状態と処理を持ち、ActorRefへ送られたメッセージはmailboxへ並び、原則として一つずつ処理されます。
送信者はactor本体を直接触らず、送信窓口であるActorRefへメッセージを渡します。コード例のとおり、送信はActorRefへのtell一行で完結し、応答を待たない送りっぱなし、いわゆるfire-and-forgetです。実装ではMailbox::runが一件ずつ取り出してactorのハンドラへ渡します。
なお、図の中で等幅フォントになっているのは実在する型やAPIで、小文字の英語はactorやmailboxのような概念です。この使い分けは以降のスライドでも同じです。
逐次処理なので、actor内部の状態はロックを前提にせず更新できます。
後半に出てくるDriveやCancelも、このmailboxへ届くコマンドです。
-->

---

<div class="eyebrow">01 · Intro</div>

# Actor があるのに、なぜ Stream なのか？

<div class="compare-visual">
  <div class="compare-lane">
    <h2>Actor</h2>
    <div>
      <div class="pipeline">
        <span class="step">ActorRef</span><span class="arrow">→</span>
        <span class="step">mailbox</span><span class="arrow">→</span>
        <span class="step">actor</span>
      </div>
      <div class="lane-code">cart_ref.tell(CartCommand::AddItem { item_id: 1, qty: 1 })</div>
    </div>
    <div class="lane-note">メッセージ配送・逐次処理・<br>実行単位</div>
  </div>
  <div class="compare-lane hot">
    <h2 class="warn">Stream</h2>
    <div>
      <div class="pipeline">
        <span class="step">Source</span><span class="arrow">→</span>
        <span class="step">Flow</span><span class="arrow">→</span>
        <span class="step">Sink</span>
      </div>
      <div class="lane-code">Source::single(41).map(|v| v + 1).into_mat(Sink::head(), KeepRight)</div>
    </div>
    <div class="lane-note">処理グラフ・需要量（demand）・終端伝播</div>
  </div>
</div>

<p class="lead center" style="margin-top: 28px"><strong>Actor は実行基盤。Stream は、その上に載るデータフローモデル。</strong></p>

<!--
[目安 1分10秒]
Actorは、メッセージをmailboxへ届け、一つずつ処理する実行単位を提供します。しかし複数の処理をどう接続し、下流の処理能力をどう上流へ返し、完了や失敗をどう伝えるかまでは決めません。Actorだけでも専用プロトコルを書けば実現できますが、データフローごとに同じ制御を設計することになります。
StreamはSource、Flow、Sinkとして処理グラフを宣言し、需要量と終端伝播を共通の実行モデルへ任せます。この需要量と終端伝播が何かは、後半の中心テーマとして順に説明します。コード例のとおり、Actor側はtellの一行、Stream側はSourceからSinkへ至る一つの宣言として書けます。そして物理実行では、そのグラフをislandへ分け、一つのislandを一つのactorとして動かします。
つまりActorとStreamは競合しません。Actorがどう動かすかを担い、Streamが何をどう流すかを担います。次に、Streamの実行場所を支えるdispatcherだけ確認します。
-->

---

<div class="eyebrow">01 · Intro</div>

# dispatcher は、mailbox の実行をスケジュールする

<div class="dispatch-path">
  <div class="dispatch-lane">
    <div class="dispatch-lane-label">送信</div>
    <div class="dispatch-box">送信者<small>cart_ref.tell(msg)</small></div><div class="arrow">→</div>
    <div class="dispatch-box">ActorRef::tell<small>tell(msg: AnyMessage)</small></div><div class="arrow">→</div>
    <div class="dispatch-box hot">dispatcher.dispatch<small>dispatch(cell, envelope)</small></div><div class="arrow">→</div>
    <div class="dispatch-box">mailbox<small>enqueue(envelope)</small></div>
  </div>
  <div class="dispatch-lane">
    <div class="dispatch-lane-label">実行</div>
    <div class="dispatch-box hot">dispatcher<small>register_for_execution</small></div><div class="arrow">→</div>
    <div class="dispatch-box">executor / worker<small>executor.execute(task)</small></div><div class="arrow">→</div>
    <div class="dispatch-box">Mailbox::run<small>run(throughput, deadline)</small></div><div class="arrow">→</div>
    <div class="dispatch-box hot">actor handler<small>invoke(message)</small></div>
  </div>
</div>

<p class="center" style="font-size: 30px; margin-top: 25px"><strong>dispatcher</strong> = enqueue + schedule　／　<strong>Mailbox::run</strong> = dequeue + invoke</p>

<p class="center muted small" style="margin-top: 14px">後半に出てくる stream の実行単位（island）も、この dispatcher で実行場所を選ぶ。</p>

<!--
[目安 1分10秒]
各ボックスの下の等幅表記は、対応する実装の呼び出しです。
上段が送信経路です。ActorRefへのtellを契機に、MessageDispatcherのdispatchがメッセージをmailboxへenqueueします。コード片にあるenvelopeはメッセージを運ぶ封筒、cellはactor本体を収める実行時の入れ物です。actorからdispatcherを呼ぶ流れではありません。
下段が実行経路です。dispatcherのregister_for_executionがmailbox.runをexecutorへ登録し、worker上でMailbox::runがメッセージをdequeueしてactorのmessage handlerをinvokeします。
概念上はdispatcherがメッセージ処理の実行を調停しますが、実コードではdequeueとhandler呼び出しをMailbox::runへ分離しています。後のasync_with_dispatcherは、このexecutor側の実行場所を選ぶ指定です。
-->

---

<div class="eyebrow">01 · Intro</div>

# 6領域 × 2層。その中の stream を掘り下げる

<div class="workspace-grid">
  <div class="head"></div><div class="head">utils</div><div class="head">actor</div><div class="head">persistence</div><div class="head">remote</div><div class="head">cluster</div><div class="head focus">stream</div>
  <div class="rowhead">core<br><span class="muted">#![no_std]</span></div><div>core</div><div>core-kernel<br>core-typed</div><div>core-kernel<br>core-typed</div><div>core</div><div>core-kernel<br>core-typed</div><div class="focus">core-kernel<br>core-actor-typed</div>
  <div class="rowhead">adaptor<br><span class="muted">Tokio 等</span></div><div>adaptor-std</div><div>adaptor-std</div><div>adaptor-std</div><div>adaptor-std</div><div>adaptor-std</div><div class="focus">adaptor-std</div>
</div>

<p class="tiny center muted" style="margin-top: 12px">core = 共通契約（kernel = 中心ロジック、typed = 型安全な公開 API）、adaptor-std = std 環境の実装</p>

<div class="metric-row four">
  <div class="metric"><strong>236</strong><span>3 crate の public 型宣言</span></div>
  <div class="metric"><strong>94%</strong><span>Pekko 主要50概念中 47 実装</span></div>
  <div class="metric"><strong>約4.6万</strong><span>3 crate の実装コード行数</span></div>
  <div class="metric"><strong>約4.4万</strong><span>3 crate のテストコード行数</span></div>
</div>

<!--
[目安 1分30秒]
ここまでのactor、mailbox、dispatcherが、本題に必要な前提知識です。次に、その本題であるstreamが、fraktor-rs全体のどこに位置するかを確認します。
fraktor-rs全体は六つの領域を持ち、それぞれをno_stdのcoreとstd環境向けadaptorに分けています。coreはさらに、中心ロジックのkernelと、型安全な公開APIを提供するtypedに分かれている領域があります。本トークで掘り下げるのは右端のstreamです。
数値は今月コードを走査した値です。236は三つのstream crateにあるpublicなstruct、enum、trait、type aliasの合計です。94パーセントは、リポジトリのギャップ分析文書で定義した、Pekkoの主要50概念のうち47を実装している割合です。実装コードは三つのstream crateの合計で約4万6千行、テストコードは約4万4千行で、実装とほぼ同量のテストを備えています。計測コマンドもリポジトリで公開しているので、手元で再計測できます。
規模を誇るためではなく、ここから示す設計が試作に留まらず、相応の実装面積で使われていることを示すための数値です。
-->

---

<div class="eyebrow">01 · Intro</div>

# API から実行の底まで、3層を降りる

<div class="layers">
  <div class="layer"><b>DSL</b><span class="path">Source → Flow → Sink</span><span class="desc">処理を組み立てる API</span></div>
  <div class="layer"><b>Materializer</b><span class="path">設計図 → 実行計画</span><span class="desc">設計図を実行可能な形へ変換</span></div>
  <div class="layer"><b>Actor System</b><span class="path">actor × N ← tick</span><span class="desc">actor の生成・実行を管理する基盤</span></div>
</div>

<p class="center muted small" style="margin-top: 18px">ステージ = Source / Flow / Sink など、処理グラフを構成する1つの処理単位<br>設計図（blueprint）= ステージ・接続・属性を保持する、実行前のデータ<br>tick = 一定間隔で実行を前へ進める合図（詳細は後半）</p>

<p class="lead center" style="margin-top: 26px">使うのは簡単。<strong>難しいのは、それを成立させる下2層だ。</strong></p>

<!--
[目安 45秒]
本トークの主張は一つです。宣言的ストリームDSLは使う側にとっては簡単ですが、その簡単さを成立させる下の実行系が難しいのです。
以降は三層を上から順に降りていきます。まずSource、Flow、Sinkで実行前の設計図を作ります。このSourceやFlowのような一つひとつの処理単位を、以降はステージと呼びます。
次にMaterializerが設計図を実行計画へ変換し、最後にActor Systemが複数のactorとして駆動します。図にあるtickは、一定間隔で実行を前へ進める合図で、後半で詳しく扱います。
この順序を覚えておくと、後半の型名や内部処理を位置づけやすくなります。
-->

---

<div class="eyebrow">02 · Declarative DSL</div>

# 要素型と materialized value を、別々に型で持つ

<div class="flow" style="margin-top: 56px">
  <div class="node hot">Source&lt;Out, Mat&gt;</div><div class="arrow">→</div>
  <div class="node hot">Flow&lt;In, Out, Mat&gt;</div><div class="arrow">→</div>
  <div class="node hot">Sink&lt;In, Mat&gt;</div>
</div>

<div class="two-col" style="margin-top: 30px">
  <div><h2>要素型</h2><p><code>In</code> / <code>Out</code><br><span class="muted">ステージを流れる値</span></p></div>
  <div><h2>materialized value</h2><p><code>Mat</code><br><span class="muted">実行時に得られる値</span></p></div>
</div>

<!--
[目安 1分20秒]
Source、Flow、Sinkは、流れる要素の型とmaterialized valueの型を別々に持ちます。
なぜ型が二つ要るのか。ストリームは実行を開始すると呼び出し元の手を離れて流れ続けるので、中を流れる値とは別に、完了値や制御ハンドルを呼び出し元が受け取る口が必要だからです。
OutやInはステージ間を流れるデータです。一方のMatがその受け取る口で、ストリームを実行した結果として外側へ返すハンドルや完了値です。
データ経路と実行結果を同じ型引数へ押し込まず、二つの関心を型レベルで分けています。
-->

---

<!-- _class: code-focus -->

<div class="eyebrow">02 · Declarative DSL</div>

# `run` は実行を開始し、完了値は別に受け取る

```rust
let graph = Source::single(41_u32)
  .map(|value| value + 1)
  .into_mat(Sink::head(), KeepRight);

let running = graph.run(&mut materializer)?;
let result = running.materialized()
  .wait_blocking(&StdBlocker::new())?;
```

<div class="flow" style="margin-top: 14px">
  <div class="node">graph.run()</div><div class="arrow">→</div><div class="node">StreamFuture&lt;u32&gt;</div><div class="arrow">→ wait</div><div class="node hot">42</div>
</div>

<p class="center muted tiny"><code>StreamFuture</code> = 完了値をあとで受け取るためのハンドル。wait して初めて 42 が得られる</p>

<!--
[目安 1分10秒]
41を一要素だけ生成し、mapで1を加え、先頭要素を受け取るSinkへ接続しています。graphを組み立てた段階では、まだ値は流れません。
runは実行を開始し、Sink::headのmaterialized valueであるStreamFutureを、Materializedという実行ハンドルに包んで返します。処理結果の42は、そのfutureの完了を待って初めて得られます。wait_blockingに渡しているStdBlockerは、std環境で完了を待つための待機実装です。
runの戻り値とストリームを流れる値を混同しないことが、この後のMaterializerの役割を理解する入口になります。
into_matの第2引数に渡しているKeepRightの意味は、少し後で説明します。
-->

---

<!-- _class: code-focus -->

<div class="eyebrow">02 · Declarative DSL</div>

# 合成しても、宣言は一本道に読める

```rust
let graph = Source::from_array([1_u32, 2])
  .via(Flow::new().concat_lazy(Source::from_array([3_u32, 4])))
  .into_mat(Sink::collect(), KeepRight);
```

<div class="flow" style="margin-top: 30px">
  <div class="node">[1, 2]</div><div class="arrow">＋</div><div class="node">[3, 4]</div><div class="arrow">→</div><div class="node hot">[1, 2, 3, 4]</div>
</div>

<!--
[目安 1分]
合成が増えても、利用者にはSourceからSinkへ向かう一つの宣言として読める形を保ちます。
viaはFlowをつなぎ、concat_lazyは後続のSourceを必要になった時点で連結し、collectするSinkへ渡します。
この例は実際のshowcaseとして実行でき、結果が1、2、3、4になることも確認しています。
-->

---

<div class="eyebrow">02 · Declarative DSL</div>

# どちらの `Mat` を残すかは、型レベルの規則で選ぶ

<p class="center muted small" style="margin-top: -6px">先ほどから <code>into_mat</code> の第2引数に渡していた <code>KeepRight</code> の正体</p>

```rust
pub trait MatCombineRule<Left, Right> {
  type Out;
  fn combine(left: Left, right: Right) -> Self::Out;
}

pub struct KeepRight; // ほかに KeepLeft / KeepBoth / KeepNone

impl<Left, Right> MatCombineRule<Left, Right> for KeepRight {
  type Out = Right; // 右 = Sink 側の Mat 型を残す
  fn combine(_left: Left, right: Right) -> Right { right }
}
```

<p class="center muted small" style="margin-top: 18px"><code>into_mat(sink, combine)</code> の第2引数が <code>C: MatCombineRule&lt;Mat, Mat2&gt;</code>。<br><code>into_mat(Sink::head(), KeepRight)</code> なら合成後の Mat は <code>C::Out</code> = Sink 側の <code>StreamFuture</code> に確定</p>

<!--
[目安 1分]
ここで、先ほどからinto_matに渡していたKeepRightの正体を説明します。
ステージを合成すると、左右どちらのmaterialized valueを外へ返すかという選択が生じます。
この選択を実行時の分岐ではなく、MatCombineRuleという型レベルの規則で表します。関連型Outが合成後の型を決め、combineが値を合成します。
KeepRightは、この規則を実装した中身のない構造体で、関連型OutをRight、つまりSink側の型へ固定します。ほかにKeepLeft、KeepBoth、KeepNoneがあります。into_matの第2引数がこの規則を受け取るので、先ほど渡したKeepRightは、Sink側のStreamFutureを残す指定になります。
規則が型なので、合成後にどのMatが得られるかはコンパイル時に確定します。
-->

---

<div class="eyebrow">02 · Declarative DSL</div>

# `RunnableGraph` は、実行前の不変な設計図

```rust
pub struct RunnableGraph<Mat> {
  plan: StreamPlan,       // ステージ・接続・属性
  materialized: Mat,      // 実行時に外へ返す値
}
```

<div class="two-col" style="margin-top: 28px">
  <div class="panel"><h2>組み立てる</h2><p><code>via</code> / <code>to</code> / <code>into_mat(..., KeepRight)</code></p><p class="muted">合成規則が <code>Mat</code> を決める。まだ何も流れない</p></div>
  <div class="panel"><h2>解釈する</h2><p><code>run(&mut materializer)</code></p><p class="muted">ここで初めて実行される</p></div>
</div>

<!--
[目安 1分15秒]
こうして合成規則まで含めて組み立てた最終形が、RunnableGraphです。ステージ、接続、属性を並べたStreamPlanと、外へ返すmaterialized valueを保持します。StreamPlanはまだ実行器ではなく、実行前データです。
RunnableGraphまで作っても副作用は起きず、runを呼ぶまでは実行前の設計図です。
記述と実行を分離することで、同じ設計図を検査し、属性を付け、実行計画へ変換できます。
-->

---

<div class="eyebrow">03 · Materializer</div>

# Materializer は、設計図を actor へ変換する

<div class="materialize-layout">
  <ol class="numbered compact">
    <li><span><code>StreamPlan</code> を読む<br><span class="muted">ステージ・接続・属性の一覧</span></span></li>
    <li><span>async の印で分ける<br><span class="muted">同じ actor で動くまとまり = island</span></span></li>
    <li><span>island 間へ有限 FIFO（ファイフォ）を置く<br><span class="muted">容量上限つきの先入れ先出しバッファ</span></span></li>
    <li><span>各 island の実行器を作る<br><span class="muted"><code>GraphInterpreter</code></span></span></li>
    <li><span>actor を生成し、最初の <code>Drive</code> を送る</span></li>
  </ol>
  <div class="materialize-diagram">
    <div class="block">RunnableGraph&lt;Mat&gt;</div>
    <div class="down">↓</div>
    <div class="block">StreamPlan</div>
    <div class="down">↓ async の印で分割</div>
    <div class="split">
      <div class="block hot">island 1</div>
      <div class="block hot">island 2</div>
    </div>
    <div class="down">↓ FIFO + 実行器 + actor</div>
    <div class="split">
      <div class="block">actor 1</div>
      <div class="block">actor 2</div>
    </div>
  </div>
</div>

<!--
[目安 1分15秒]
Materializerの仕事は、実行前の設計図をactorとして動く形へ変換することです。まずRunnableGraphから、ステージ、接続、属性を持つStreamPlanを読みます。
次にasyncの印で、同じactor上で動かすステージのまとまりへ分けます。このまとまりをislandと呼びます。islandをまたぐ接続には有限FIFO、つまり容量に上限のある先入れ先出しのバッファを置き、各islandのGraphInterpreterを作って、一つずつactorとして生成します。最後に、各actorへ最初のDriveを送って駆動を始めます。
図の上側は論理的な設計図、中央は分割された実行計画、下側は実際に駆動されるactorです。
-->

---

<div class="eyebrow">03 · Materializer</div>

# 「記述」は、ここで物理的な実行単位になる

<div class="layers">
  <div class="layer"><b>DSL</b><span class="path">RunnableGraph&lt;Mat&gt;</span><span class="desc">型付きの不変な設計図</span></div>
  <div class="layer"><b>Materializer</b><span class="path">StreamPlan → split → compile</span><span class="desc">境界と実行器を具体化</span></div>
  <div class="layer"><b>Actor System</b><span class="path">actor × N + Drive tick</span><span class="desc">既定10ms間隔で協調的に前進</span></div>
</div>

<p class="center" style="margin-top: 30px"><strong>宣言的 DSL の本体は、記述と解釈の分離である。</strong></p>

<!--
[目安 1分10秒]
三層の責務をもう一度対応づけます。DSLは型付きの設計図を作り、Materializerは境界と実行器を具体化します。
Actor Systemはactorの生成と実行を管理し、既定では10ミリ秒間隔のDriveで各islandを協調的に進めます。
宣言的DSLの本体は、見た目のメソッドチェーンではなく、記述と解釈を切り離せることです。
-->

---

<div class="eyebrow">03 · Materializer</div>

# 実行時は型を消す。ただし別スレッドへ渡せる型だけ

<div class="big-contrast" style="margin-top: 70px">
  <div><div class="term accent">Source&lt;Out, Mat&gt;</div><p class="muted">コンパイル時に型付け</p></div>
  <div class="arrow">→</div>
  <div><div class="term compact warn">Box&lt;dyn Any + Send&gt;</div><p class="muted">実行系内部の DynValue</p></div>
</div>

<div class="two-col" style="margin-top: 42px">
  <p><code>Any</code> = 具体的な型を実行時まで隠す</p>
  <p><code>Send</code> = 値を別スレッドへ移せる</p>
</div>
<p class="center small" style="margin-top: 22px">取り出した型が違えば <code>StreamError::TypeMismatch</code> になる。</p>

<!--
[目安 1分20秒]
DSLの外側ではSourceのOutやFlowのInとOutがコンパイル時に検査されます。しかし実行時には、異なるステージを同じグラフ構造へ格納する必要があります。
そこで内部の値をBox dyn Any plus Sendへ型消去し、DynValueとして扱います。Anyは具体型を隠すため、Sendは別スレッドへ値を渡せることを保証するために付きます。
型安全をすべて捨てたのではなく、静的な境界と動的な境界を分けた設計で、境界を越えた不一致はTypeMismatchとして扱います。
-->

---

<div class="eyebrow">03 · Execution model</div>

# `GraphInterpreter` は、Drive ごとに1ステップ進む

<p class="center muted small" style="margin-top: -6px">Drive = 前進の合図。actor はメッセージが届いたときにしか動かないので、<br>合図そのものを <code>Drive</code> というメッセージにして mailbox へ送り込む（送り手と間隔は 04 章）</p>

<div class="flow" style="margin-top: 42px">
  <div class="node">保留中の仕事を<br>再試行</div><div class="arrow">→</div>
  <div class="node">初回だけ<br>Sink を開始</div><div class="arrow">→</div>
  <div class="node hot">demand があれば<br>Source → Flow → Sink</div><div class="arrow">→</div>
  <div class="node">全要素と終端を<br>確認</div>
</div>

<p class="center muted small" style="margin-top: 10px">demand = 下流が受け取れる要素数（詳細は次ページ）</p>

<p class="quote"><code>drive()</code> は待たない。進められなければ <code>Idle</code>。</p>

<!--
[目安 1分40秒]
先ほどMaterializerが各islandに作るとした実行器が、このGraphInterpreterです。
まず、なぜDriveという合図が要るのかです。actorはメッセージが届いたときにしか動きません。一方ストリームは、外部から新しい入力が来なくても、Futureの完了確認やバッファの再送のために進み続ける必要があります。この差を埋めるため、前進の合図そのものをDriveというメッセージとしてmailboxへ送り込みます。Driveが来なければ、islandは止まったままです。
そのためGraphInterpreterは、専用スレッドを占有して回り続けるループではなく、Driveを受けるたびに少しだけ進む協調的ステートマシンとして設計されています。
一回のdriveでは、保留中の非同期処理や、island境界のFIFOへ入りきらなかった要素の送信を再試行し、初回だけSinkを開始します。demand、つまり下流がまだ受け取れる要素数が残っていればSourceからpullし、Flowを進め、Sinkへ一要素を渡します。最後に終端条件を確認します。
進められればProgressed、待つしかなければIdleを返し、次のDriveへ制御を戻します。
この一ステップ性が、後でislandをactorのmailboxから駆動する設計につながります。
-->

---

<div class="eyebrow">03 · Execution model</div>

# 上流が送り続けるのではなく、下流が要求量を伝える

<div class="center" style="margin-top: 70px">
  <div class="flow">
    <div class="node">Source</div><div class="arrow">←</div><div class="node">Flow</div><div class="arrow">←</div><div class="node hot">Sink</div>
  </div>
  <p class="accent" style="font-family: 'IBM Plex Mono'; margin-top: -15px">demand = 下流が受け取れる要素数</p>
  <div class="flow" style="margin-top: 30px">
    <div class="node">Source</div><div class="arrow">→</div><div class="node">Flow</div><div class="arrow">→</div><div class="node hot">Sink</div>
  </div>
  <p class="warn" style="font-family: 'IBM Plex Mono'; margin-top: -15px">要求された数だけ要素を送る</p>
</div>

<p class="lead center" style="margin-top: 38px">下流の処理能力を上流へ伝える。<br>これがバックプレッシャーである。</p>

<!--
[目安 1分40秒]
前のページに出てきたdemandを、ここで正式に定義します。
その前に、なぜ送る量を制御するのかです。上流が全力で送り続けると、遅い下流の手前で要素が際限なく積み上がり、メモリを使い果たすか、要素を捨てるしかなくなります。これを防ぐには、送る量を下流の都合で決める必要があります。
そこで、上流が生成できるだけ送り続けるのではなく、下流が受け取れる要素数をdemandとして上流へ伝えます。
要求は右から左へ進み、要素は要求された数だけ左から右へ進みます。下流が要求しなければ、上流から新しい要素は流れません。
したがって、下流の処理能力が上流の流量を制約します。この逆向きの情報伝播がバックプレッシャーです。
-->

---

<div class="eyebrow">03 · Execution model</div>

# `DemandTracker` は「あと何件受け取れるか」を守る

```rust
// Sink::on_start
demand.request(1)

// GraphInterpreter::drive_sink_once（抜粋）
if !self.demand.has_demand() {
  return Ok(progressed);
}
// 入力取得と型確認は中略
self.demand.consume(1)?;
```

<div class="flow" style="margin-top: 24px">
  <div class="node">残量 0</div><div class="arrow">request(1) →</div><div class="node hot">残量 1</div><div class="arrow">consume(1) →</div><div class="node">残量 0</div>
</div>

<!--
[目安 1分15秒]
DemandTrackerは、下流があと何件受け取れるかを管理します。Sinkがrequestで要求数を増やし、interpreterはhas_demandが真のときだけ上流からpullします。
一要素をSinkへ渡す直前にconsumeで残量を減らします。図のように一件要求し、一件渡せば残量はゼロへ戻ります。このrequestとconsumeの対が、グラフ全体の流量制御の基礎になります。
ここまでは一つの実行単位の中を見てきました。次は同じ需要量の契約を保ったまま、グラフを複数のactorへ分けます。
-->

---

<!-- _class: no-page -->

<div class="eyebrow">04 · async boundary / island</div>

# 同じ “async” でも、指しているものが違う

<div class="big-contrast">
  <div><div class="term accent">async boundary</div><p>actor 境界<br><span class="muted">並行実行単位の境界</span></p></div>
  <div class="neq">≠</div>
  <div><div class="term warn">Rust async/await</div><p>Future の構文<br><span class="muted">言語の非同期抽象</span></p></div>
</div>

<!--
[目安 1分15秒]
最も重要な用語の整理です。ここでいうasync boundaryは、Rustのasync fnやawaitとは別物です。
Pekko由来のasync boundaryは、グラフを別々のactorへ分割する境界、つまり並行実行単位の境界を指します。
一方、Rustのasync/awaitはFutureを記述するための言語機能です。同じasyncという語でも、抽象の層が違います。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# `r#async()` は、最後のステージに印を付けるだけ

```rust
#[must_use]
pub fn r#async(mut self) -> Flow<In, Out, Mat> {
  self.graph.mark_last_node_async();
  self
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncBoundaryAttr;
```

<p class="center muted small"><code>r#async</code> = <code>async</code> が Rust の予約語のため、生識別子 <code>r#</code> を付けた表記</p>

<p class="lead center">実行単位への分割は、Materializer の解釈時まで起きない。</p>

<!--
[目安 1分10秒]
r#asyncメソッドがその場でタスクやスレッドを生成するわけではありません。Rustではasyncが予約語なので生識別子になっていますが、処理は最後のノードへ属性を付けるだけです。
AsyncBoundaryAttrもデータを持たないマーカー型です。
実際の分割はMaterializerが設計図を解釈するときまで遅延されます。印だけに留めるのは、設計図の段階では論理構造だけを持たせ、物理的な分割の判断をMaterializerへ集約するためです。記述と解釈の分離を、ここでも守っています。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# マーカー位置で、1本のグラフを2つの island に分ける

<div class="split-diagram">
  <div class="panel">
    <h2>分割前</h2>
    <div class="pipeline" style="gap: 7px; margin-top: 48px">
      <span class="step">Source</span><span class="arrow">→</span>
      <span class="step">map</span><span class="arrow">→</span>
      <span class="step">filter</span>
      <span class="boundary-cut"><span>async</span><span class="cut-line"></span><span>boundary</span></span>
      <span class="step">map_async</span><span class="arrow">→</span>
      <span class="step">Sink</span>
    </div>
  </div>
  <div class="panel">
    <h2>分割後</h2>
    <div class="materialize-diagram" style="margin: 12px auto 0; width: 88%">
      <div class="block">Island 1 = 1 actor<br><span class="tiny muted">Source → map → filter → BoundarySink</span></div>
      <div class="down">↓</div>
      <div class="block hot">IslandBoundary<br><span class="tiny">既定容量16 FIFO</span></div>
      <div class="down">↓</div>
      <div class="block">Island 2 = 1 actor<br><span class="tiny muted">BoundarySource → map_async → Sink</span></div>
    </div>
  </div>
</div>

<!--
[目安 1分30秒]
左は分割前の一本の論理グラフです。filterの後ろにasync boundaryの印があります。その先のmap_asyncはFutureを返す非同期ステージで、詳しくは後で扱います。
Materializerはそこでステージ間の接続を切り、上流側をIsland 1、下流側をIsland 2として扱います。各islandは一つのactorになり、境界にはBoundarySink、有限FIFO、BoundarySourceが挿入されます。
論理的なメソッドチェーンは一本のままですが、物理的には独立して駆動される二つの実行単位へ変わります。
-->

---

<!-- _class: code-compact -->

<div class="eyebrow">04 · async boundary / island</div>

# 切断後もつながっているまとまりが、island になる

<p class="center muted small">edge（辺）= ステージとステージの間の接続。<code>StreamPlan</code> はステージ一覧と edge 一覧を持つ</p>

```rust
for edge in &plan.edges {
  if plan.stages[from_stage].attributes().is_async() {
    if let Some(dispatcher) = plan.stages[from_stage].attributes().get::<DispatcherAttribute>() {
      dispatcher_candidates[to_stage].push(String::from(dispatcher.name()));
    }
    continue;
  }

  adjacency[from_stage].push(to_stage);
  adjacency[to_stage].push(from_stage);
}
```

<p class="muted small">edge のうち、async 印のステージから出るものは切断点として除く。<br>残った edge でつながるまとまり（= 連結成分）を BFS（幅優先探索）で求め、上流→下流の順に island ID を振る。</p>

<!--
[目安 1分20秒]
分割アルゴリズムを見ます。コードに出てくるedge、つまり辺とは、ステージとステージの間の接続のことです。
async属性を持つ上流ステージから出る辺は、つながりとして数えません。そこが切断点になります。
残った辺でつながっているステージのまとまりを求めます。探索には、隣り合うステージから順にたどるBFS、幅優先探索を使います。このまとまりがグラフ理論でいう連結成分で、一つのまとまりが一つのislandになります。
最後に、上流から下流へという元の順序を保ってisland IDを割り当てるため、分割後もデータの向きは失われません。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# dispatcher 属性は、下流 island の実行場所を決める

<p class="center muted small" style="margin-top: -6px"><code>r#async()</code> の変種 <code>async_with_dispatcher("blocking")</code> を使うと、この属性が付く</p>

<div class="dispatcher-visual">
  <div class="island"><b>Island 1</b><div class="mini">actor A</div></div>
  <div class="dispatcher-marker">
    <div class="attribute">属性: dispatcher = "blocking"</div>
    <div class="arrow" style="font-size: 52px; line-height: 1.2">→</div>
    <div class="danger small">async boundary（切断点）</div>
  </div>
  <div class="island"><b>Island 2</b><div class="mini">actor B<br>dispatcher: blocking</div></div>
</div>

<p class="lead center" style="margin-top: 42px">境界は actor を分け、属性は下流 actor の実行場所を選ぶ。</p>

<!--
[目安 1分20秒]
なぜ実行場所を選びたいのか。例えばブロッキングI/Oを含むステージを他のactorと同じ実行環境で動かすと、スレッドを塞いで周りまで止めてしまいます。そうしたislandだけを専用のdispatcherへ逃がしたい、という要求に応えるのがこの属性です。
async_with_dispatcherでは二つの指定が重なっています。async boundaryがactorを分け、dispatcher属性が下流側actorの実行場所を決めます。属性はデータが通過するステージではなく、切断点へ付く設定です。
この例ではIsland 2のactor Bがblockingという名前のdispatcherで生成されます。
境界そのものと、境界の先をどこで動かすかを分けて考えるのが要点です。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# 満杯なら上流を止め、1枠空いたら再開する

<p class="center muted small" style="margin-top: -6px">island へ分割したあとも、demand の契約を保つ仕組み</p>

<div class="boundary-timeline">
  <div class="boundary-frame">
    <b>初期：満杯</b>
    <div class="mini-buffer full"><span class="filled">FIFO 16 / 16</span></div>
    <div class="pending-dot">pending ●</div>
    <small>上流への demand を停止</small>
  </div>
  <div class="time-arrow">→</div>
  <div class="boundary-frame">
    <b>① 下流が pull(1)</b>
    <div class="mini-buffer"><span class="filled">15</span><span class="empty">1</span></div>
    <div class="pending-dot">pending ●</div>
    <small>FIFO に1枠空く</small>
  </div>
  <div class="time-arrow">→</div>
  <div class="boundary-frame">
    <b>② pending を再送</b>
    <div class="mini-buffer full"><span class="filled">FIFO 16 / 16</span></div>
    <div class="pending-dot">pending なし</div>
    <small>空いた枠へ push</small>
  </div>
  <div class="time-arrow">→</div>
  <div class="boundary-frame" style="border-color: var(--orange)">
    <b>③ push 成功後</b>
    <div class="mini-buffer full"><span class="filled">FIFO 16 / 16</span></div>
    <div class="pending-dot">demand(1) ↑</div>
    <small>上流へ次の1件を要求</small>
  </div>
</div>

<p class="center muted small" style="margin-top: 24px">時間は左から右へ進む。FIFO本体に加え、拒否された1件だけを pending として保持する。<br><span class="tiny">枠の塗りは残量のゲージで、位置に意味はない。要素は先に入れたものから順に下流へ渡る。</span></p>

<!--
[目安 1分40秒]
ここまでで、グラフをactorへ分ける仕組みを見てきました。残る問いは、前半で予告した、分けた後もdemandの契約が保たれるのか、です。その答えがこの境界FIFOの循環です。
時間は左から右へ進みます。island間のFIFOは、設定がなければ16要素が上限です。満杯のとき、BoundarySinkは拒否された一要素をpendingとして保持し、新しいdemandを出しません。
①で下流actorが一要素pullすると、FIFOに一枠の空きができます。②でpending要素をその空きへpushします。③でpushが成功して初めて、上流actorへ次の一要素をdemandします。
この循環により、別々のactorで動いていても下流の速度が上流へ伝わり、メモリ使用量を有限に保てます。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# 終端シグナルは、データを追い越してはいけない

<p class="center muted tiny" style="margin-top: -6px">island に分けると、上流の完了・失敗の通知が、境界 FIFO にまだ残っているデータより先に下流へ届き得る。<br>そのままでは、最後の要素を渡す前にストリームが終わったことになってしまう</p>

<div class="terminal-lanes">
  <div class="terminal-lane">
    <b>data lane</b>
    <div class="flow" style="justify-content: flex-start; margin: 0">
      <div class="node">buffered data</div><div class="arrow">→</div>
      <div class="node">buffered data</div><div class="arrow">→</div>
      <div class="node hot">FIFO empty</div>
    </div>
  </div>
  <div class="terminal-lane">
    <b>control lane</b>
    <div class="flow" style="justify-content: flex-start; margin: 0">
      <div class="node boundary">terminal signal</div><div class="arrow">→</div>
      <div class="node boundary">保留</div><div class="arrow">→ FIFO empty?</div>
      <div class="node hot">Completed / Failed</div>
    </div>
  </div>
</div>

<p class="quote tight">データ列が空になるまで、制御列の終端を見せない。</p>

<p class="center muted tiny" style="margin-top: 8px"><code>Open</code> → <code>Completed</code> / <code>Failed</code> / <code>DownstreamCancelled</code></p>

<!--
[目安 1分30秒]
IslandBoundaryはFIFOとライフサイクル状態を一緒に持ちます。Openから完了、失敗、下流キャンセルのいずれか一つへ遷移し、単なる一時的な空と終端を区別します。
上流が完了しても、FIFOやpendingにはまだ配送すべき要素が残っている場合があります。そこで完了や失敗のシグナルをデータ列とは別の制御状態として保留します。
下流は残ったデータをすべてpullした後で初めてCompletedまたはFailedを観測します。
この順序保証がなければ、最後の要素より先に終了だけが届くことになります。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# 下流の cancel は、データとは別の制御経路で上流へ返す

<div class="flow" style="margin-top: 70px">
  <div class="island"><b>Upstream island</b><div class="mini">StreamIslandActor</div></div>
  <div class="arrow">←</div>
  <div class="node boundary">Cancel<br><span class="tiny">制御経路</span></div>
  <div class="arrow">←</div>
  <div class="island"><b>Downstream island</b><div class="mini">cancel demand</div></div>
</div>

<div class="two-col" style="margin-top: 55px">
  <div><h2 class="good">配送成功</h2><p>上流 actor が <code>Cancel</code> を処理</p></div>
  <div><h2 class="danger">配送失敗</h2><p>kill switch で全 island を即座に失敗させる</p></div>
</div>

<!--
[目安 1分20秒]
データは上流から下流へ流れますが、キャンセルは逆向きに伝える必要があります。下流のBoundarySourceがキャンセルされると、データとは別の制御経路で、上流islandのactorへCancelコマンドを送ります。
配送できれば上流actorが処理を止めます。actorが既に消えているなど配送自体が失敗した場合は、処理を宙ぶらりんにせず、kill switch、つまり全islandを一斉に失敗させて止める緊急停止の仕組みで全体を落とします。
データ経路と制御経路を分けつつ、失敗時の全体整合性を保つ設計です。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# island は、mailbox のコマンドで1ステップずつ進む

<p class="muted small" style="margin-top: -4px">tick = scheduler が既定10ms間隔で発する、前進の合図。tick が <code>Drive</code> コマンドになる。</p>

<div class="dispatch-path" style="margin-top: 20px">
  <div>
    <h2>配送</h2>
    <div class="flow command-flow" style="justify-content: flex-start; margin: 0">
      <div class="node">scheduler tick</div><div class="arrow">→</div>
      <div class="node">ChildRef::try_tell(Drive)</div><div class="arrow">→</div>
      <div class="node boundary">dispatcher.dispatch</div><div class="arrow">→</div>
      <div class="node hot">mailbox に enqueue</div>
    </div>
  </div>
  <div>
    <h2>実行</h2>
    <div class="flow command-flow" style="justify-content: flex-start; margin: 0">
      <div class="node boundary">dispatcher が実行登録</div><div class="arrow">→</div>
      <div class="node">Mailbox::run<br>Drive を dequeue</div><div class="arrow">→</div>
      <div class="node">actor.receive(Drive)</div><div class="arrow">→</div>
      <div class="node hot">GraphInterpreter::drive()</div>
    </div>
  </div>
</div>

<p class="center muted small" style="margin-top: 28px">Cancel / Shutdown / Abort も同じ mailbox で逐次処理する。</p>

<!--
[目安 1分30秒]
物理実行では、一つのislandが一つのStreamIslandActorになります。actorは届いたメッセージでしか動かないので、ストリームが進み続けるには、誰かが前進の合図を送り続ける必要があります。その送り手がschedulerで、tickとは、schedulerが既定10ミリ秒間隔で発する前進の合図です。schedulerのtickはChildRefのtry_tellでDriveを送ります。dispatcherがmailboxへenqueueして実行登録し、Mailbox::runがDriveをdequeueして、actorのメッセージハンドラであるreceiveへ渡します。
StreamIslandActorがDriveを受け取るとGraphInterpreterのdriveを一回呼びます。schedulerからactor本体を直接呼んでいるわけではありません。
同じmailboxにはCancel、Shutdown、Abortも入り、データ処理とライフサイクル制御を逐次処理します。
ストリームが完了または失敗の終端へ達したactorは、自分自身をstopして実行単位を閉じます。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# tick ポーリングは、意図的なトレードオフである

<p class="muted small" style="margin-top: -4px"><code>map_async</code>（Future を返すステージ）の完了検知は、前ページの tick → <code>Drive</code> の中で行う poll だけである。</p>

<div class="two-col wide-left">
  <div>

<p class="tiny muted" style="margin-bottom: -8px">① poll に渡す Waker（完了時に起こしてもらう配線）を、あえて「何もしない」実装にする</p>

```rust
pub(crate) const fn noop_waker() -> Waker {
  unsafe { Waker::from_raw(noop_raw_waker()) }
}

const fn noop_wake(_: *const ()) {}
```

<p class="tiny muted" style="margin-bottom: -8px">② 代わりに Drive のたびに poll し、Ready になったものだけ完了へ進める</p>

```rust
if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
  *entry = MapAsyncEntry::Completed(output);
}
```

  </div>
  <div>
    <table class="comparison">
      <tr><th>wake 通知</th><th>tick ポーリング</th></tr>
      <tr><td>完了時に<br>再スケジュール</td><td>Drive ごとに<br>再 poll</td></tr>
      <tr><td>低レイテンシ</td><td class="warn">最大 drive 間隔</td></tr>
      <tr><td>wake 配線が必要</td><td class="good">wake 配線が不要</td></tr>
    </table>
    <p class="muted small">どちらも <code>no_std</code> で実装できる。現在は wake 統合を避け、既定10ms間隔で駆動する。</p>
  </div>
</div>

<!--
[目安 1分40秒]
このtick駆動には、一つ論点が残ります。分割の例に出てきたmap_async、つまり要素ごとにFutureを返す非同期ステージの完了検知です。RustのFutureはpollで進み、完了したら起こしてもらうためのWakerを受け取ります。Tokioのようなランタイムは、このwakeを合図にタスクを再スケジュールします。
一方、islandは前ページのとおりtickごとに必ずDriveでpollされるので、起こしてもらう必要がありません。そこでWakerには、あえて何もしない実装を渡します。これがnoop_wakerです。Driveのたびにpollし、完了したFutureだけをCompletedへ遷移させます。
wake通知で再スケジュールする方式とtick方式は、どちらもno_stdで実装できます。違いはno_std対応の可否ではなく、wake通知の配線を実行系へ要求するかどうかです。
現在の実装はwake統合を避ける代わりに、既定設定では完了検知が最悪10ミリ秒遅れます。この実行方式の選択に続いて、次は所有権と共有状態がAPIの形をどう決めるかを見ます。
-->

---

<div class="eyebrow">05 · Rust constraints</div>

# まず `&mut self`。共有は必要な場所だけ

```rust
// ロジックは所有権の中で &mut self として素直に更新（例: demand 管理で見た DemandTracker）
pub struct DemandTracker { demand: Demand }
impl DemandTracker {
  pub fn request(&mut self, amount: u64) -> Result<(), StreamError> { /* 加算 */ }
  pub fn has_demand(&self) -> bool { /* 読み取りは &self */ }
}
```

```rust
// 共有が必要な箇所だけ SharedLock で包み、with_read / with_write に閉じる（ガードは外へ返さない）
pub struct SharedLock<T> {
  inner: ArcShared<dyn SharedLockBackend<T>>, // ArcShared = Arc 相当の共有参照。backend がロック実装を隠す
}
```

<p class="quote tight">内部可変性を、設計の出発点にしない。</p>

<!--
[目安 1分20秒]
基本方針は、状態を持つロジックをまず所有権の中へ置き、&mut selfで素直に更新することです。
なぜか。共有可変状態を増やすほど、どこから書き換わるかを追いにくくなり、借用チェッカによる保護も効かなくなるからです。
上のコードは、先ほどdemandの管理で見たDemandTrackerです。状態変更のrequestは&mut self、読み取りのhas_demandは&selfと、所有権の中で素直に書けています。
複数actorや境界から共有する必要が生じた箇所だけSharedLockで包み、with_readとwith_writeのクロージャ内へアクセスを閉じます。下のコードがSharedLockの実際の構造で、Arc相当の共有参照ArcSharedの内側に、ロック実装を隠すbackendを持ちます。
スライド下部の内部可変性とは、共有参照のまま中身を書き換えられる仕組みのことです。最初からすべてを共有可変状態にせず、共有範囲を設計上の例外として狭くします。
この「共有が必要な箇所」の実例が、次に見るisland境界の共有FIFOです。
-->

---

<div class="eyebrow">05 · Rust constraints</div>

# 所有権を返す共有 API は、`FnOnce + R` で表現する

<p class="center muted small" style="margin-top: -6px">境界 FIFO が満杯のとき、値を捨てず所有権ごと返して pending 再試行する（04 の循環）を支える API</p>

<p class="tiny muted" style="margin-bottom: -8px">前ページの SharedLock も実装する、SharedAccess 契約の with_write。FnOnce = 一度だけ呼べるクロージャ、R = その戻り値型</p>

```rust
fn with_write<R>(&self, f: impl FnOnce(&mut B) -> R) -> R;

let result = boundary.with_write(move |inner| {
  inner.try_push(value) // Result<(), DynValue>
});
```

<div class="flow" style="margin-top: 20px">
  <div class="node">value を move</div><div class="arrow">→</div><div class="node boundary">FnOnce</div><div class="arrow">→</div><div class="node hot">Ok / Err(value)</div>
</div>

<p class="muted center small"><code>R</code> として所有権を外へ戻せる。所有権を返すために、ロックそのものは必須ではない。</p>

<!--
[目安 1分20秒]
island境界のFIFOには、満杯のとき値を捨てず、所有権ごと呼び出し元へ返して再試行させるという要件があります。共有された状態に触りながら所有権を外へ戻す、この組み合わせがRustでは設計上の課題になります。
答えがこのシグネチャです。前のページのSharedLockが実装するSharedAccessのwith_writeは、FnOnceを受け、任意の戻り値Rを返せます。値をクロージャへmoveし、FIFOが満杯ならtry_pushが拒否した値をResultのErrとして、同じ所有権のまま外へ戻せます。呼び出し元はそれをpendingとして再試行します。
したがって、所有権を返すためにロックが必須なわけではありません。現在のIslandBoundarySharedはSharedLockへ寄せられるリファクタリング候補で、重要なのは共有API自体をFnOnceとRで設計することです。
-->

---

<div class="eyebrow">05 · Rust constraints</div>

# island をまたぐ値は、別スレッドへ渡せる必要がある

<p class="center muted small" style="margin-top: -12px">03 · Materializer で見た型消去を、今度は所有権の視点から見直す</p>

<p class="lead center" style="margin-top: 20px"><code>Send</code> = 値の所有権を別スレッドへ安全に移せること</p>

<div class="big-contrast" style="margin-top: 34px">
  <div><div class="term accent">型付き DSL</div><p><code>Out</code> / <code>In</code></p></div>
  <div class="arrow">→</div>
  <div><div class="term warn">DynValue</div><p><code>Box&lt;dyn Any + Send&gt;</code></p></div>
</div>

<p class="center" style="margin-top: 38px"><strong>Send 境界</strong> = island 間で、要素型に <code>Send</code> を要求する箇所</p>

<!--
[目安 1分]
Materializerの節で見た型消去を、ここでは所有権の視点から見直します。
Sendは、値の所有権を別スレッドへ安全に移せることを表すRustのマーカートレイトです。
型消去後のDynValueはBox dyn Any plus Sendであり、island間の接続が要素型へSendを要求する地点、つまりSend境界になります。Send境界という専用オブジェクトが存在するわけではありません。
この制約は内部だけに閉じず、Source、Flow、Sinkで扱うInやOutの型制約へ伝播します。実行単位を分ける判断が、利用者から見える型にも影響します。
※Q&A想定メモ: Syncを問われたら「island間では値を共有参照で見せず所有権ごと移すため、要素型への要求はSendだけでSyncは不要」と回答。
-->

---

<div class="eyebrow">05 · Rust constraints</div>

# Tokio との結線点は Actor 層に置く

<div class="layers">
  <div class="layer"><b>stream-core-kernel</b><span class="path">#![no_std]</span><span class="desc">DSL / Materializer / GraphInterpreter</span></div>
  <div class="layer"><b>actor-core-kernel</b><span class="path">ActorSystem</span><span class="desc">dispatcher / tick driver の契約</span></div>
  <div class="layer"><b>actor-adaptor-std</b><span class="path">StdTickDriver</span><span class="desc">Tokio など std 環境へ接続</span></div>
</div>

<div class="flow" style="margin-top: 26px">
  <div class="node">StdTickDriver</div><div class="arrow">→</div><div class="node">ActorSystem</div><div class="arrow">→</div><div class="node hot">ActorMaterializer</div>
</div>

<!--
[目安 1分20秒]
stream-core-kernelはDSL、Materializer、GraphInterpreterを持ちますが、Tokioを直接知りません。
tick driverやdispatcherの契約はactor-core-kernelに置き、Tokioへ接続するStdTickDriverはactor-adaptor-stdで実装します。
これまでMaterializerと呼んできたものの実体であるActorMaterializerは、Actor Systemを受け取るだけなので、std環境との差分をstream層へ持ち込まずに済みます。
実装にはGraphDslによる合流と分岐、つまりfan-inとfan-out、Actor System間をつなぐStreamRef、複数のストリームを動的につなぐhub系、失敗時に再起動するrestartや流量を絞るthrottleも存在します。
これらも同じ記述、解釈、islandの基盤に載りますが、本トークでは個別機能へ広げず実行基盤に焦点を絞りました。
-->

---

<div class="eyebrow">05 · Rust constraints</div>

# Rust だから楽になったこと、難しくなったこと

<div class="two-col">
  <div class="panel">
    <h2 class="good">楽になった — 所有権が味方</h2>
    <ul class="compact">
      <li>リソース解放が自然。actor の stop で island の状態も FIFO も Future も <code>Drop</code> で消える</li>
      <li>データ競合はコンパイル時に排除。型消去後も <code>Send</code> が越境を守る</li>
      <li>設計図は <code>run(self)</code> で消費。実行済みグラフの再利用ミスを型が防ぐ</li>
    </ul>
  </div>
  <div class="panel">
    <h2 class="warn">難しくなった — 所有権と戦う</h2>
    <ul class="compact">
      <li>共有可変状態が毎回設計論点になる。境界 FIFO には <code>FnOnce + R</code> が必要だった</li>
      <li>言語の Future と実行系の接続が自明でない。tick 駆動はその回避策</li>
      <li>参照実装を直訳できない。継承と GC の前提を、trait 合成と所有権へ翻訳する</li>
    </ul>
  </div>
</div>

<p class="quote tight">制約は消えない。制約を設計の入力にする。</p>

<!--
[目安 1分10秒]
05の締めくくりとして、Rustで作って何が楽で何が大変だったかを整理します。
楽になったのは、まずリソース管理です。actorがstopすれば、islandの状態、境界FIFO、進行中のFutureまで、所有権にぶら下がったものがDropで消えます。GCもfinalizerも要らず、これがno_stdを成立させる土台でもあります。データ競合もコンパイル時に排除されます。型消去したDynValueにもSendが付くので、island間の越境は安全なままです。さらにRunnableGraphのrunはselfを消費するので、実行済みの設計図をもう一度runするミスは、型レベルで起きません。
難しくなったのは、このセクションで見てきたところです。共有可変状態は毎回設計の論点になり、境界FIFOにはFnOnceとRの設計が必要でした。言語のFutureと実行系の接続も自明ではなく、tick駆動はその一つの答えです。そして参照実装のPekkoは継承とGCを前提にしているので、そのまま持ち込めず、trait合成と所有権の語彙へ翻訳し直す必要があります。
制約は消えません。制約を設計の入力として扱うことが、Rustでランタイムを書くコツだと考えています。
-->

---

<!-- _class: no-page -->

<div class="eyebrow">06 · What comes next</div>

# 残った課題も、実行系の境界にある

<div class="two-col" style="margin-top: 52px">
  <div class="panel">
    <h2>std 統合</h2>
    <p class="lead"><strong>TCP / TLS</strong></p>
    <p class="muted">残るアダプタ統合を詰める</p>
  </div>
  <div class="panel">
    <h2>実行器の分割</h2>
    <p class="lead"><strong>GraphInterpreter</strong></p>
    <p class="muted">demand / scheduling を壊さない単位で段階的に分ける</p>
  </div>
</div>

<p class="center" style="font-family: 'IBM Plex Mono'; font-size: 31px; margin-top: 58px">github.com/j5ik2o/fraktor-rs</p>
<p class="center muted">API の先にある実行モデルを、一緒に考えたい。</p>

<!--
[目安 50秒]
残る課題は二つあります。std側ではTCPとTLSのアダプタ統合を詰める必要があります。
core側ではGraphInterpreterが大きくなっているため、demandやschedulingの不変条件を壊さない単位で段階的に分けたいと考えています。
fraktor-rsはpre-releaseで、コードとshowcaseは公開しています。質問や設計上の異論も含め、リポジトリでフィードバックを歓迎します。
-->

---

<div class="eyebrow">06 · Takeaways</div>

# 宣言的 DSL の難所は、API ではなく実行系にある

<ol class="numbered">
  <li><span><strong>記述と解釈を分ける</strong><br><span class="muted">設計図は不変データ。実行は interpreter の責務</span></span></li>
  <li><span><strong>async boundary ≠ async/await</strong><br><span class="muted">並行性の単位は island。island = 1 actor</span></span></li>
  <li><span><strong><code>no_std</code> を境界に、実行系を分ける</strong><br><span class="muted">core は tick で前進し、std adaptor が実行環境へ接続する</span></span></li>
</ol>

<!--
[目安 1分20秒]
最後に、持ち帰ってほしいことを三つにまとめます。
一つ目は、宣言的DSLの本体を記述と解釈の分離として捉えることです。不変な設計図があるから、実行前に分割や属性解釈を行えます。
二つ目は、async boundaryとasync/awaitを混同しないことです。ここでの並行性の単位はislandで、一つのislandが一つのactorになります。
三つ目は、no_stdを単なる制約ではなく、実行系を分ける境界として使うことです。coreはOSやTokioのような外部のイベント通知機構に依存せずtickで前進し、std adaptorがTokioなどの実行環境へ接続します。その選択にはポータビリティと引き換えのレイテンシがあります。
簡単なAPIを成立させる難所は、この三つを整合させる実行系にあります。
-->

---

<!-- _class: title-slide no-page -->

<div class="eyebrow">Thank you</div>

# ありがとうございました

<div class="subtitle">Rustで宣言的ストリームDSLを設計する</div>

<div class="meta">github.com/j5ik2o/fraktor-rs<br>@j5ik2o</div>

<!--
[目安 10秒]
ご清聴ありがとうございました。
-->
