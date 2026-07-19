#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codeinsight-benchmark-local.XXXXXX")"

cleanup() {
  rm -rf "$TEMP_DIR"
}

trap cleanup EXIT INT TERM

repo="$TEMP_DIR/repo"
report="$TEMP_DIR/benchmark-local.md"
summary_json="$TEMP_DIR/summary.json"
output_log="$TEMP_DIR/output.log"
mkdir -p "$repo/src"

cat >"$repo/src/main.ts" <<'TS'
import { createRouter } from "./router";

export function bootstrap() {
  const router = createRouter();
  router.register("/health", () => "ok");
  return router;
}

bootstrap();
TS

cat >"$repo/src/router.ts" <<'TS'
type Handler = () => string;

export function createRouter() {
  const routes = new Map<string, Handler>();
  return {
    register(path: string, handler: Handler) {
      routes.set(path, handler);
    },
    handle(path: string) {
      return routes.get(path)?.() ?? "missing";
    },
  };
}
TS

git -C "$repo" init --quiet
git -C "$repo" config user.email "codeinsight@example.com"
git -C "$repo" config user.name "CodeInsight Smoke"
git -C "$repo" add src/main.ts src/router.ts
git -C "$repo" commit --quiet -m "fixture"

CODEINSIGHT_BENCH_PROFILE=local \
  CODEINSIGHT_BENCH_LOCAL_ROOT="$repo" \
  CODEINSIGHT_BENCH_LOCAL_NAME=local-fixture \
  CODEINSIGHT_BENCH_LOCAL_LANGUAGE=TypeScript \
  CODEINSIGHT_BENCH_LOCAL_CONTEXT_FILE=src/main.ts \
  CODEINSIGHT_BENCH_LOCAL_TASK="understand local app bootstrap flow" \
  CODEINSIGHT_BENCH_WORKDIR="$TEMP_DIR/work" \
  CODEINSIGHT_BENCH_OUTPUT="$report" \
  CODEINSIGHT_BENCH_SUMMARY_JSON="$summary_json" \
  "$ROOT_DIR/scripts/benchmark-smoke.sh" | tee "$output_log"

"$ROOT_DIR/scripts/benchmark-report-smoke.sh" "$report" local

grep -Fq 'Profile: `local`' "$report"
grep -Fq 'URL: local:' "$report"
grep -Fq '## local-fixture' "$report"
grep -Fq 'Context seed file: `src/main.ts`' "$report"
grep -Fq '`first_reading_focus` | present' "$report"
grep -Fq '`first_reading_question` | present' "$report"
grep -Fq 'benchmark summary' "$output_log"
grep -Fq "report: $report" "$output_log"
grep -Fq 'repositories: 1 (all)' "$output_log"
grep -Fq 'context_pack first: 1/1' "$output_log"
grep -Fq 'guardrail failures: 0' "$output_log"
grep -Fq 'next steps' "$output_log"
grep -Fq "open report: $report" "$output_log"
grep -Fq "wrote summary $summary_json" "$output_log"
grep -Fq 'inspect: Key Results, Summary, and each Context reading plan table' "$output_log"
grep -Fq 'continue with: file_outline for first files, dependency_graph for imports, impact_analysis before edits' "$output_log"

jq -e \
  --arg report "$report" \
  '.report == $report
    and .profile == "local"
    and .repositories == 1
    and .routing.context_pack_first == 1
    and .routing.total == 1
    and .context.selected_lines > 0
    and .context.estimated_tokens_total > 0
    and .failures.total == 0
    and .next_steps.open_report == $report
    and (.next_steps.continue_with | contains("impact_analysis"))' \
  "$summary_json" >/dev/null

if [ -d "$repo/.codeinsight" ]; then
  echo "local benchmark should not write .codeinsight into source repository" >&2
  exit 1
fi

echo "benchmark local smoke passed"
