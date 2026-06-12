#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function configuredLanguage(root) {
  const configPath = resolve(root, ".takt", "config.yaml");
  if (!existsSync(configPath)) {
    return undefined;
  }
  const match = readFileSync(configPath, "utf8").match(/^language:\s*(en|ja)\s*$/m);
  return match?.[1];
}

export function resolveWorkflowPath(root, workflowName) {
  const preferredLanguage = configuredLanguage(root);
  const languageOrder = preferredLanguage === "en" ? ["en", "ja"] : ["ja", "en"];
  const workflowCandidates = [
    resolve(root, ".takt", "workflows", `${workflowName}.yaml`),
    ...languageOrder.map((language) => resolve(root, ".takt", language, "workflows", `${workflowName}.yaml`)),
  ];
  return workflowCandidates.find((path) => existsSync(path));
}

function stripTaskFlagForHelp(args) {
  if (!args.includes("--help") && !args.includes("-h")) {
    return args;
  }
  return args.filter((arg) => arg !== "-t" && arg !== "--task");
}

export function buildTaktArgs(workflowPath, forwardedArgs) {
  const argsForTakt = collapseTaskPayload(stripTaskFlagForHelp(forwardedArgs));
  const taskArgIndex = argsForTakt.findIndex((arg) => arg === "-t" || arg === "--task");
  return taskArgIndex === -1
    ? [...argsForTakt, "-w", workflowPath]
    : [...argsForTakt.slice(0, taskArgIndex), "-w", workflowPath, ...argsForTakt.slice(taskArgIndex)];
}

export function collapseTaskPayload(args) {
  const taskArgIndex = args.findIndex((arg) => arg === "-t" || arg === "--task");
  if (taskArgIndex === -1 || taskArgIndex === args.length - 1) {
    return args;
  }

  const rest = args.slice(taskArgIndex + 1);
  const separatorIndex = rest.indexOf("--");

  // `--` 区切りがある場合: 区切り前は takt のオプションとして -t の前へ移し、
  // 区切り後だけを task 本文として結合する（オプションを task に飲み込まない）
  if (separatorIndex !== -1) {
    const options = rest.slice(0, separatorIndex);
    const payloadParts = rest.slice(separatorIndex + 1);
    return payloadParts.length === 0
      ? [...args.slice(0, taskArgIndex), ...options]
      : [...args.slice(0, taskArgIndex), ...options, args[taskArgIndex], payloadParts.join(" ")];
  }

  // 区切りなしでオプションらしき引数が先頭に来ている場合は、task 本文へ
  // 飲み込まず明示的に使い方エラーにする（--provider 等の値の区切りが曖昧なため）
  if (rest[0]?.startsWith("-")) {
    throw new Error(
      "Takt options must be separated from the task text: " +
        'npm run kiro:<command> -- [takt options...] -- "task text"',
    );
  }

  const taskPayload = rest.join(" ");
  return [...args.slice(0, taskArgIndex + 1), taskPayload];
}

export function main(argv = process.argv.slice(2)) {
  const [workflowName, ...forwardedArgs] = argv;

  if (!workflowName) {
    console.error("Usage: node scripts/kiro-staged.mjs <workflow-name> [takt args...]");
    return 1;
  }

  const workflowPath = resolveWorkflowPath(repoRoot, workflowName);

  if (!workflowPath) {
    console.error(`Kiro workflow '${workflowName}' is not installed yet.`);
    console.error("This command is part of the staged Kiro workflow surface.");
    console.error("Install or merge the downstream Kiro workflow implementation before running it.");
    return 1;
  }

  // mise env と TAKT_*_CLI_PATH を設定する既存の起動経路（run-takt.sh）を経由する
  const taktWrapper = resolve(repoRoot, "scripts", "run-takt.sh");
  const command = existsSync(taktWrapper) ? taktWrapper : "takt";
  let taktArgs;
  try {
    taktArgs = buildTaktArgs(workflowPath, forwardedArgs);
  } catch (error) {
    console.error(error.message);
    return 1;
  }
  // run-takt.sh は ACCOUNT 未設定だと先頭位置引数（--pipeline 等）をアカウント名として
  // 消費するため、既定アカウントを env で明示して takt 引数を温存する
  const result = spawnSync(command, taktArgs, {
    stdio: "inherit",
    env: { ...process.env, ACCOUNT: process.env.ACCOUNT || "corporate" },
  });

  if (result.error) {
    console.error(result.error.message);
    return 1;
  }

  return result.status ?? 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.exit(main());
}
