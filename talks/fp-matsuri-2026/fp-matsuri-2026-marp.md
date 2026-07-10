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
  .dispatch-box small { color: var(--muted); font-family: 'Noto Sans JP', sans-serif; font-size: 14px; font-weight: 400; margin-top: 5px; }
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

  .terminal-lanes { display: grid; gap: 24px; margin-top: 42px; }
  .terminal-lane { align-items: center; display: grid; gap: 18px; grid-template-columns: 150px 1fr; }
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
    padding: 14px 18px;
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
本トークでは、Rustで宣言的なストリームDSLを設計するとき、APIの背後にどのような実行系が必要になるかを扱う。題材は開発中のfraktor-rsである。
特に、async boundaryとislandという言葉が、Rustのasync/awaitとは別の実行モデルを指すことを解きほぐす。
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
      <li>所属・役割：正式版で追記</li>
      <li>主な活動：正式版で追記</li>
      <li>登壇・著書など：正式版で追記</li>
    </ul>
    <p class="muted small" style="margin-top: 34px">※プロフィール情報は正式版で差し替え予定です。</p>
  </div>
  <img class="profile-photo" src="./self-profile.jpg" alt="プロフィール画像">
</div>

<!--
[目安 30秒]
所属、現在の活動、このトークとの関係だけを短く伝える。数値実績や経歴は、正式なプロフィールへ差し替えるまでは追加しない。
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
    <p>Tokio・ネットワークなど<br>ホスト固有の実装を分離</p>
  </div>
</div>

<p class="muted" style="margin-top: 30px">pre-release。API は Pekko に学び、実行系は Rust 向けに再設計している。</p>

<!--
[目安 1分10秒]
fraktor-rsはApache PekkoとProto.Actorのアクターモデルを参照しつつ、Rustの所有権とno_std制約に合わせて再設計しているアクターランタイムである。
ここで重要なのは、仕様駆動は開発手法であり、ランタイムが仕様駆動で動作するわけではない点である。
coreはホスト環境に依存しない契約と状態機械を持ち、Tokioやネットワークへの接続はstd adaptorへ分離している。
-->

---

<div class="eyebrow">01 · Intro</div>

# actor は、状態と mailbox を持つ実行単位

<div class="concept-flow" style="margin-top: 58px">
  <div class="concept-block">
    <b>送信者</b>
    <span>メッセージを作る</span>
  </div>
  <div class="arrow">→</div>
  <div class="concept-block">
    <b>ActorRef</b>
    <span>actor への送信窓口</span>
  </div>
  <div class="arrow">→</div>
  <div>
    <p class="center muted small">mailbox<br>受信待ちの列</p>
    <div class="mailbox"><span></span><span></span><span></span><span></span></div>
  </div>
  <div class="arrow"><span style="display: block; font-size: 13px; white-space: nowrap">Mailbox::run</span>→</div>
  <div class="concept-block">
    <b>actor</b>
    <span>状態 + 処理</span>
  </div>
</div>

<p class="quote">送信者は ActorRef にメッセージを送る。</p>
<p class="center muted">mailbox runner が1件ずつ取り出し、actor のハンドラへ渡す。</p>

<!--
[目安 1分20秒]
このトークで必要なactorの知識は三つだけである。actorは状態と処理を持ち、ActorRefへ送られたメッセージはmailboxへ並び、原則として一つずつ処理される。
送信者はactor本体を直接触らず、送信窓口であるActorRefへメッセージを渡す。実装ではMailbox::runが一件ずつ取り出してactorのハンドラへ渡す。逐次処理であるため、actor内部の状態はロックを前提にせず更新できる。
後半に出てくるDriveやCancelも、このmailboxへ届くコマンドである。
-->

---

<div class="eyebrow">01 · Intro</div>

# Actor があるのに、なぜ Stream なのか？

<div class="compare-visual">
  <div class="compare-lane">
    <h2>Actor</h2>
    <div class="pipeline">
      <span class="step">ActorRef</span><span class="arrow">→</span>
      <span class="step">mailbox</span><span class="arrow">→</span>
      <span class="step">actor</span>
    </div>
    <div class="lane-note">メッセージ配送・逐次処理・実行単位</div>
  </div>
  <div class="compare-lane hot">
    <h2 class="warn">Stream</h2>
    <div class="pipeline">
      <span class="step">Source</span><span class="arrow">→</span>
      <span class="step">Flow</span><span class="arrow">→</span>
      <span class="step">Sink</span>
    </div>
    <div class="lane-note">処理グラフ・需要量・終端伝播</div>
  </div>
</div>

<p class="lead center" style="margin-top: 28px"><strong>Actor は実行基盤。Stream は、その上に載るデータフローモデル。</strong></p>

<!--
[目安 1分10秒]
Actorは、メッセージをmailboxへ届け、一つずつ処理する実行単位を提供する。しかし複数の処理をどう接続し、下流の処理能力をどう上流へ返し、完了や失敗をどう伝えるかまでは決めない。Actorだけでも専用プロトコルを書けば実現できるが、データフローごとに同じ制御を設計することになる。
StreamはSource、Flow、Sinkとして処理グラフを宣言し、需要量と終端伝播を共通の実行モデルへ任せる。そして物理実行では、そのグラフをislandへ分け、一つのislandを一つのactorとして動かす。
つまりActorとStreamは競合しない。Actorがどう動かすかを担い、Streamが何をどう流すかを担う。次に、Streamの実行場所を支えるdispatcherだけ確認する。
-->

---

<div class="eyebrow">01 · Intro</div>

# dispatcher は、mailbox の実行をスケジュールする

<div class="dispatch-path">
  <div class="dispatch-lane">
    <div class="dispatch-lane-label">送信</div>
    <div class="dispatch-box">送信者</div><div class="arrow">→</div>
    <div class="dispatch-box">ActorRef::tell</div><div class="arrow">→</div>
    <div class="dispatch-box hot">dispatcher.dispatch</div><div class="arrow">→</div>
    <div class="dispatch-box">mailbox<small>enqueue</small></div>
  </div>
  <div class="dispatch-lane">
    <div class="dispatch-lane-label">実行</div>
    <div class="dispatch-box hot">dispatcher<small>register_for_execution</small></div><div class="arrow">→</div>
    <div class="dispatch-box">executor / worker</div><div class="arrow">→</div>
    <div class="dispatch-box">Mailbox::run<small>dequeue</small></div><div class="arrow">→</div>
    <div class="dispatch-box hot">actor handler<small>invoke(message)</small></div>
  </div>
</div>

<p class="center" style="font-size: 30px; margin-top: 25px"><strong>dispatcher</strong> = enqueue + schedule　／　<strong>Mailbox::run</strong> = dequeue + invoke</p>

<!--
[目安 1分10秒]
上段が送信経路である。ActorRefへのtellを契機に、MessageDispatcherのdispatchがメッセージをmailboxへenqueueする。actorからdispatcherを呼ぶ流れではない。
下段が実行経路である。dispatcherのregister_for_executionがmailbox.runをexecutorへ登録し、worker上でMailbox::runがメッセージをdequeueしてactorのmessage handlerをinvokeする。
概念上dispatcherがメッセージ処理の実行を調停するが、実コードではdequeueとhandler呼び出しをMailbox::runへ分離している。後のasync_with_dispatcherは、このexecutor側の実行場所を選ぶ指定である。
-->

---

<div class="eyebrow">01 · Intro</div>

# 6ドメイン × 2層。その中の stream を掘り下げる

<div class="workspace-grid">
  <div class="head"></div><div class="head">utils</div><div class="head">actor</div><div class="head">persistence</div><div class="head">remote</div><div class="head">cluster</div><div class="head focus">stream</div>
  <div class="rowhead">core<br><span class="muted">#![no_std]</span></div><div>core</div><div>core</div><div>core</div><div>core</div><div>core</div><div class="focus">core-kernel</div>
  <div class="rowhead">adaptor-std<br><span class="muted">Tokio 等</span></div><div>std</div><div>std</div><div>std</div><div>std</div><div>std</div><div class="focus">adaptor-std</div>
</div>

<p class="tiny center muted" style="margin-top: 12px">core = 共通契約、adaptor-std = std 環境の実装</p>

<div class="metric-row">
  <div class="metric"><strong>236</strong><span>3 stream crate の public 型宣言</span></div>
  <div class="metric"><strong>94%</strong><span>固定50概念中 47概念</span></div>
  <div class="metric"><strong>約4.2万</strong><span>core-kernel のテスト行数</span></div>
</div>

<!--
[目安 1分20秒]
fraktor-rs全体は六つのドメインを持ち、それぞれをno_stdのcoreとstd環境向けadaptorに分けている。本トークで掘り下げるのは右端のstreamである。
数値は2026年7月10日に現在のコードを走査した値である。236は三つのstream crateにあるpublicなstruct、enum、trait、type aliasの合計。94パーセントはdocs/gap-analysis/stream-gap-analysis.mdで定義した固定50概念中47概念。約4.2万行はstream-core-kernelのアンダースコアtest.rsを合計した4万2500行である。計測コマンドは講演素材に残している。
規模を誇るためではなく、ここから示す設計が試作だけではなく、相応の実装面積で使われていることを示している。
-->

---

<div class="eyebrow">01 · Intro</div>

# API から実行の底まで、3層を降りる

<div class="layers">
  <div class="layer"><b>DSL</b><span class="path">Source → Flow → Sink</span><span class="desc">処理を組み立てる API</span></div>
  <div class="layer"><b>Materializer</b><span class="path">設計図 → 実行計画</span><span class="desc">設計図を実行可能な形へ変換</span></div>
  <div class="layer"><b>Actor System</b><span class="path">actor × N ← tick</span><span class="desc">actor の生成・実行を管理する基盤</span></div>
</div>

<p class="center muted small" style="margin-top: 18px">設計図（blueprint）= ステージ・接続・属性を保持する、実行前のデータ</p>

<!--
[目安 35秒]
以降は三層を上から順に降りる。まずSource、Flow、Sinkで実行前の設計図を作る。
次にMaterializerが設計図を実行計画へ変換し、最後にActor Systemが複数のactorとして駆動する。
この順序を覚えておくと、後半の型名や内部処理を位置づけやすい。
-->

---

<div class="eyebrow">02 · Declarative DSL</div>

# 要素型と materialized value を、別々に型で持つ

<div class="flow" style="margin-top: 70px">
  <div class="node hot">Source&lt;Out, Mat&gt;</div><div class="arrow">→</div>
  <div class="node hot">Flow&lt;In, Out, Mat&gt;</div><div class="arrow">→</div>
  <div class="node hot">Sink&lt;In, Mat&gt;</div>
</div>

<div class="two-col" style="margin-top: 60px">
  <div><h2>要素型</h2><p><code>In</code> / <code>Out</code><br><span class="muted">ステージを流れる値</span></p></div>
  <div><h2>materialized value</h2><p><code>Mat</code><br><span class="muted">実行時に得られる値</span></p></div>
</div>

<!--
[目安 1分10秒]
Source、Flow、Sinkは、流れる要素の型とmaterialized valueの型を別々に持つ。
OutやInはステージ間を流れるデータである。一方のMatは、ストリームを実行した結果として外側へ返すハンドルや完了値である。
データ経路と実行結果を同じ型引数へ押し込まず、二つの関心を型レベルで分けている。
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

<!--
[目安 1分10秒]
41を一要素だけ生成し、mapで1を加え、先頭要素を受け取るSinkへ接続している。graphを組み立てた段階では、まだ値は流れない。
runは実行を開始し、Sink::headのmaterialized valueであるStreamFutureをMaterializedの中へ返す。処理結果の42は、そのfutureの完了を待って初めて得られる。
runの戻り値とストリームを流れる値を混同しないことが、この後のMaterializerの役割を理解する入口になる。
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
合成が増えても、利用者にはSourceからSinkへ向かう一つの宣言として読める形を保つ。
viaはFlowをつなぎ、concat_lazyは後続のSourceを必要になった時点で連結し、collectするSinkへ渡す。
この例は実際のshowcaseとして実行でき、結果が1、2、3、4になることも確認している。
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
RunnableGraphは、ステージ、接続、属性を並べたStreamPlanと、外へ返すmaterialized valueを保持する。StreamPlanはまだ実行器ではなく、実行前データである。
ステージを合成したときは、KeepLeft、KeepRight、KeepBoth、KeepNoneの規則が、左右どちらのmaterialized valueを外へ返すかと合成後の型を決める。
RunnableGraphまで作っても副作用は起きず、runを呼ぶまでは実行前の設計図である。
記述と実行を分離することで、同じ設計図を検査し、属性を付け、実行計画へ変換できる。
-->

---

<div class="eyebrow">03 · Materializer</div>

# Materializer は、設計図を actor へ変換する

<div class="materialize-layout">
  <ol class="numbered compact">
    <li><span><code>StreamPlan</code> を読む<br><span class="muted">ステージ・接続・属性の一覧</span></span></li>
    <li><span>async の印で分ける<br><span class="muted">同じ actor で動くまとまり = island</span></span></li>
    <li><span>island 間へ有限 FIFO を置く</span></li>
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
Materializerの仕事は、実行前の設計図をactorとして動く形へ変換することである。まずRunnableGraphから、ステージ、接続、属性を持つStreamPlanを読む。
次にasyncの印で、同じactor上で動かすステージのまとまりへ分ける。このまとまりをislandと呼ぶ。islandをまたぐ接続には有限FIFOを置き、各islandのGraphInterpreterを作って、一つずつactorとして生成する。
図の上側は論理的な設計図、中央は分割された実行計画、下側は実際に駆動されるactorである。
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
三層の責務をもう一度対応づける。DSLは型付きの設計図を作り、Materializerは境界と実行器を具体化する。
Actor Systemはactorの生成と実行を管理し、既定では10ミリ秒間隔のDriveで各islandを協調的に進める。
宣言的DSLの本体は、見た目のメソッドチェーンではなく、記述と解釈を切り離せることである。
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
DSLの外側ではSourceのOutやFlowのInとOutがコンパイル時に検査される。しかし実行時には、異なるステージを同じグラフ構造へ格納する必要がある。
そこで内部の値をBox dyn Any plus Sendへ型消去し、DynValueとして扱う。Anyは具体型を隠すため、Sendは別スレッドへ値を渡せることを保証するために付く。
型安全をすべて捨てたのではなく、静的な境界と動的な境界を分けた設計であり、境界を越えた不一致はTypeMismatchとして扱う。
-->

---

<div class="eyebrow">03 · Materializer</div>

# `GraphInterpreter` は、Drive ごとに1ステップ進む

<div class="flow" style="margin-top: 62px">
  <div class="node">保留中の仕事を<br>再試行</div><div class="arrow">→</div>
  <div class="node">初回だけ<br>Sink を開始</div><div class="arrow">→</div>
  <div class="node hot">demand があれば<br>Source → Flow → Sink</div><div class="arrow">→</div>
  <div class="node">全要素と終端を<br>確認</div>
</div>

<p class="quote"><code>drive()</code> は待たない。進められなければ <code>Idle</code>。</p>

<!--
[目安 1分20秒]
GraphInterpreterは専用スレッドを占有して回り続けるループではなく、Driveコマンドを受けるたびに少しだけ進む協調的ステートマシンである。
一回のdriveでは、保留中の非同期処理や境界pushを再試行し、初回だけSinkを開始する。demandがあればSourceからpullし、Flowを進め、Sinkへ一要素を渡す。最後に終端条件を確認する。
進められればProgressed、待つしかなければIdleを返し、次のDriveへ制御を戻す。
この一ステップ性が、後でislandをactorのmailboxから駆動する設計につながる。
-->

---

<div class="eyebrow">03 · Materializer</div>

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

<p class="lead center" style="margin-top: 38px">下流の処理能力を上流へ伝える。これがバックプレッシャである。</p>

<!--
[目安 1分25秒]
ここでバックプレッシャを定義する。上流が生成できるだけ送り続けるのではなく、下流が受け取れる要素数をdemandとして上流へ伝える。
要求は右から左へ進み、要素は要求された数だけ左から右へ進む。下流が要求しなければ、上流から新しい要素は流れない。
したがって、下流の処理能力が上流の流量を制約する。この逆向きの情報伝播がバックプレッシャである。
-->

---

<div class="eyebrow">03 · Materializer</div>

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
DemandTrackerは、下流があと何件受け取れるかを管理する。Sinkがrequestで要求数を増やし、interpreterはhas_demandが真のときだけ上流からpullする。
一要素をSinkへ渡す直前にconsumeで残量を減らす。図のように一件要求し、一件渡せば残量はゼロへ戻る。このrequestとconsumeの対が、グラフ全体の流量制御の基礎になる。
ここまでは一つの実行単位の中を見てきた。次は同じ需要量の契約を保ったまま、グラフを複数のactorへ分ける。
-->

---

<!-- _class: no-page -->

<div class="eyebrow">04 · The key message</div>

# 同じ “async” でも、指しているものが違う

<div class="big-contrast">
  <div><div class="term accent">async boundary</div><p>actor 境界<br><span class="muted">並行実行単位の境界</span></p></div>
  <div class="neq">≠</div>
  <div><div class="term warn">Rust async/await</div><p>Future の構文<br><span class="muted">言語の非同期抽象</span></p></div>
</div>

<!--
[目安 1分15秒]
最も重要な用語の整理である。ここでいうasync boundaryは、Rustのasync fnやawaitとは別物である。
Pekko由来のasync boundaryは、グラフを別々のactorへ分割する境界、つまり並行実行単位の境界を指す。
一方、Rustのasync/awaitはFutureを記述するための言語機能である。同じasyncという語でも、抽象の層が違う。
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

<p class="lead center">実行単位への分割は、materialization 時まで起きない。</p>

<!--
[目安 1分]
r#asyncメソッドがその場でタスクやスレッドを生成するわけではない。Rustではasyncが予約語なので生識別子になっているが、処理は最後のノードへ属性を付けるだけである。
AsyncBoundaryAttrもデータを持たないマーカー型である。
実際の分割はMaterializerが設計図を解釈するときまで遅延される。
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
左は分割前の一本の論理グラフである。filterの後ろにasync boundaryの印がある。
Materializerはそこで辺を切り、上流側をIsland 1、下流側をIsland 2として扱う。各islandは一つのactorになり、境界にはBoundarySink、有限FIFO、BoundarySourceが挿入される。
論理的なメソッドチェーンは一本のままだが、物理的には独立して駆動される二つの実行単位へ変わる。
-->

---

<!-- _class: code-compact -->

<div class="eyebrow">04 · async boundary / island</div>

# 切断後の連結成分が、そのまま island になる

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

<p class="muted">残った無向グラフを BFS。連結成分ごとに、トポロジカル順で island ID を振る。</p>

<!--
[目安 1分10秒]
分割アルゴリズムは、async属性を持つ上流ステージから出る辺を隣接関係へ追加しない。つまり、その辺をグラフの切断点として扱う。
残った辺は無向グラフとして接続し、BFSで連結成分を求める。一つの連結成分が一つのislandである。
最後に元のトポロジカル順を保ってisland IDを割り当てるため、分割後もデータの向きは失われない。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# dispatcher 属性は、下流 island の実行場所を決める

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
[目安 1分10秒]
async_with_dispatcherでは二つの指定が重なっている。async boundaryがactorを分け、dispatcher属性が下流側actorの実行場所を決める。属性はデータが通過するステージではなく、切断点へ付く設定である。
この例ではIsland 2のactor Bがblockingという名前のdispatcherで生成される。
境界そのものと、境界の先をどこで動かすかを分けて考えるのが要点である。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# 満杯なら上流を止め、1枠空いたら再開する

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

<p class="center muted small" style="margin-top: 24px">時間は左から右へ進む。FIFO本体に加え、拒否された1件だけを pending として保持する。</p>

<!--
[目安 1分30秒]
時間は左から右へ進む。island間のFIFOは、設定がなければ16要素が上限である。満杯のとき、BoundarySinkは拒否された一要素をpendingとして保持し、新しいdemandを出さない。
①で下流actorが一要素pullすると、FIFOに一枠の空きができる。②でpending要素をその空きへpushする。③でpushが成功して初めて、上流actorへ次の一要素をdemandする。
この循環により、別々のactorで動いていても下流の速度が上流へ伝わり、メモリ使用量を有限に保てる。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# 終端シグナルは、データを追い越してはいけない

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

<p class="quote" style="margin-top: 48px">データ列が空になるまで、制御列の終端を見せない。</p>

<p class="center muted"><code>Open</code> → <code>Completed</code> / <code>Failed</code> / <code>DownstreamCancelled</code></p>

<!--
[目安 1分30秒]
IslandBoundaryはFIFOとライフサイクル状態を一緒に持つ。Openから完了、失敗、下流キャンセルのいずれか一つへ遷移し、単なる一時的な空と終端を区別する。
上流が完了しても、FIFOやpendingにはまだ配送すべき要素が残っている場合がある。そこで完了や失敗のシグナルをデータ列とは別の制御状態として保留する。
下流は残ったデータをすべてpullした後で初めてCompletedまたはFailedを観測する。
この順序保証がなければ、最後の要素より先に終了だけが届くことになる。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# 下流の cancel は、制御プレーンで上流へ返す

<div class="flow" style="margin-top: 70px">
  <div class="island"><b>Upstream island</b><div class="mini">StreamIslandActor</div></div>
  <div class="arrow">←</div>
  <div class="node boundary">Cancel<br><span class="tiny">control plane</span></div>
  <div class="arrow">←</div>
  <div class="island"><b>Downstream island</b><div class="mini">cancel demand</div></div>
</div>

<div class="two-col" style="margin-top: 55px">
  <div><h2 class="good">配送成功</h2><p>上流 actor が <code>Cancel</code> を処理</p></div>
  <div><h2 class="danger">配送失敗</h2><p>kill switch で全 island を fail-fast</p></div>
</div>

<!--
[目安 1分20秒]
データは上流から下流へ流れるが、キャンセルは逆向きに伝える必要がある。下流のBoundarySourceがキャンセルされると、制御プレーンが上流islandのactorへCancelコマンドを送る。
配送できれば上流actorが処理を止める。actorが既に消えているなど配送自体が失敗した場合は、処理を宙ぶらりんにせずkill switchで全islandを失敗させる。
データ経路と制御経路を分けつつ、失敗時の全体整合性を保つ設計である。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# island は、mailbox のコマンドで1ステップずつ進む

<div class="dispatch-path" style="margin-top: 28px">
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
[目安 1分20秒]
物理実行では、一つのislandが一つのStreamIslandActorになる。schedulerのtickはChildRefのtry_tellでDriveを送る。dispatcherがmailboxへenqueueして実行登録し、Mailbox::runがDriveをdequeueしてactorのreceiveへ渡す。
StreamIslandActorがDriveを受け取るとGraphInterpreterのdriveを一回呼ぶ。schedulerからactor本体を直接呼んでいるわけではない。
同じmailboxにはCancel、Shutdown、Abortも入り、データ処理とライフサイクル制御を逐次処理する。
ストリームが完了または失敗の終端へ達したactorは、自分自身をstopして実行単位を閉じる。
-->

---

<div class="eyebrow">04 · async boundary / island</div>

# tick ポーリングは、意図的なトレードオフである

<div class="two-col wide-left">
  <div>

```rust
pub(crate) const fn noop_waker() -> Waker {
  unsafe { Waker::from_raw(noop_raw_waker()) }
}

const fn noop_wake(_: *const ()) {}
```

```rust
if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
  *entry = MapAsyncEntry::Completed(output);
}
```

  </div>
  <div>
    <table class="comparison">
      <tr><th>wake 通知</th><th>tick ポーリング</th></tr>
      <tr><td>完了時に再スケジュール</td><td>Drive ごとに再 poll</td></tr>
      <tr><td>低レイテンシ</td><td class="warn">最大 drive 間隔</td></tr>
      <tr><td>wake 配線が必要</td><td class="good">wake 配線が不要</td></tr>
    </table>
    <p class="muted small">どちらも <code>no_std</code> で実装できる。現在は wake 統合を避け、既定10ms間隔で駆動する。</p>
  </div>
</div>

<!--
[目安 1分30秒]
map_asyncのFutureは、何もしないWakerを使ってDriveのたびにpollする。完了したFutureだけをCompletedへ遷移させる設計である。
wake通知で再スケジュールする方式とtick方式は、どちらもno_stdで実装できる。違いはno_std対応の可否ではなく、wake通知の配線を実行系へ要求するかどうかである。
現在の実装はwake統合を避ける代わりに、既定設定では完了検知が最悪10ミリ秒遅れる。この実行方式の選択に続いて、次は所有権と共有状態がAPIの形をどう決めるかを見る。
-->

---

<div class="eyebrow">05 · Rust constraints</div>

# まず `&mut self`。共有は必要な場所だけ

<div class="two-col" style="margin-top: 66px">
  <div class="panel" style="border-color: var(--green)">
    <h2 class="good">ロジック</h2>
    <p style="font-family: 'IBM Plex Mono'; font-size: 38px">&mut self</p>
    <p class="muted">所有権の中で、状態を素直に更新</p>
  </div>
  <div class="panel">
    <h2>共有が必要な箇所</h2>
    <p style="font-family: 'IBM Plex Mono'; font-size: 31px">SharedLock</p>
    <p class="muted"><code>with_read</code> / <code>with_write</code><br>ガードを外へ返さない</p>
  </div>
</div>

<p class="quote">内部可変性を、設計の出発点にしない。</p>

<!--
[目安 1分10秒]
基本方針は、状態を持つロジックをまず所有権の中へ置き、&mut selfで素直に更新することである。
複数actorや境界から共有する必要が生じた箇所だけSharedLockで包み、with_readとwith_writeのクロージャ内へアクセスを閉じる。
最初からすべてを共有可変状態にせず、共有範囲を設計上の例外として狭くする。
-->

---

<div class="eyebrow">05 · Rust constraints</div>

# 所有権を返す共有 API は、`FnOnce + R` で表現する

```rust
fn with_write<R>(&self, f: impl FnOnce(&mut B) -> R) -> R;

let result = boundary.with_write(move |inner| {
  inner.try_push(value) // Result<(), DynValue>
});
```

<div class="flow" style="margin-top: 20px">
  <div class="node">value を move</div><div class="arrow">→</div><div class="node boundary">FnOnce</div><div class="arrow">→</div><div class="node hot">Ok / Err(value)</div>
</div>

<p class="muted center small"><code>R</code> として所有権を外へ戻せる。直接ロックは必要条件ではない。</p>

<!--
[目安 1分20秒]
FIFOが満杯ならtry_pushは拒否した値をErr valueとして返し、呼び出し元はpendingとして再試行する。
SharedAccessのwith_writeはFnOnceを受け、任意の戻り値Rを返せる。値をクロージャへmoveし、ResultのErrとして同じ所有権を外へ戻せる。
したがって直接ロックは所有権上の必然ではない。現在のIslandBoundarySharedはSharedLockへ寄せられるリファクタリング候補であり、重要なのは共有API自体をFnOnceとRで設計することである。
-->

---

<div class="eyebrow">05 · Rust constraints</div>

# island をまたぐ値は、別スレッドへ渡せる必要がある

<p class="lead center" style="margin-top: -8px"><code>Send</code> = 値の所有権を別スレッドへ安全に移せること</p>

<div class="big-contrast" style="margin-top: 34px">
  <div><div class="term accent">型付き DSL</div><p><code>Out</code> / <code>In</code></p></div>
  <div class="arrow">→</div>
  <div><div class="term warn">DynValue</div><p><code>Box&lt;dyn Any + Send&gt;</code></p></div>
</div>

<p class="center" style="margin-top: 38px"><strong>Send 境界</strong> = island 間で、要素型に <code>Send</code> を要求する箇所</p>

<!--
[目安 1分]
Sendは、値の所有権を別スレッドへ安全に移せることを表すRustのマーカートレイトである。
型消去後のDynValueはBox dyn Any plus Sendであり、island間の接続が要素型へSendを要求する地点、つまりSend境界になる。Send境界という専用オブジェクトが存在するわけではない。
この制約は内部だけに閉じず、Source、Flow、Sinkで扱うInやOutの型制約へ伝播する。実行単位を分ける判断が、利用者から見える型にも影響する。
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
stream-core-kernelはDSL、Materializer、GraphInterpreterを持つが、Tokioを直接知らない。
tick driverやdispatcherの契約はactor-core-kernelに置き、Tokioへ接続するStdTickDriverはactor-adaptor-stdで実装する。
ActorMaterializerはActor Systemを受け取るだけなので、std環境との差分をstream層へ持ち込まずに済む。
実装にはGraphDslのfan-inとfan-out、Actor System間をつなぐStreamRef、hub系、restartやthrottleも存在する。
これらも同じ記述、解釈、islandの基盤に載るが、本トークでは個別機能へ広げず実行基盤に焦点を絞った。
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
残る課題は二つある。std側ではTCPとTLSのアダプタ統合を詰める必要がある。
core側ではGraphInterpreterが大きくなっているため、demandやschedulingの不変条件を壊さない単位で段階的に分けたい。
fraktor-rsはpre-releaseであり、コードとshowcaseは公開している。質問や設計上の異論も含め、リポジトリでフィードバックを歓迎する。
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
一つ目は、宣言的DSLの本体を記述と解釈の分離として捉えることである。不変な設計図があるから、実行前に分割や属性解釈を行える。
二つ目は、async boundaryとasync/awaitを混同しないことである。ここでの並行性の単位はislandであり、一つのislandが一つのactorになる。
三つ目は、no_std自体を実行モデルと呼ぶのではなく、no_stdを境界としてcoreとstd adaptorの責務を分けることである。coreは外部reactorに依存せずtickで前進し、std adaptorがTokioなどの実行環境へ接続する。その選択にはポータビリティと引き換えのレイテンシがある。
簡単なAPIを成立させる難所は、この三つを整合させる実行系にある。
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
