#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "task routing matrix smoke failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

build_binary_if_needed() {
  if [ -z "$CODEINSIGHT_BIN" ]; then
    require_command cargo
    cargo build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml" >/dev/null
    CODEINSIGHT_BIN="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT_DIR/Cargo.toml" | jq -r '.target_directory')/release/codeinsight"
  fi

  if [ ! -x "$CODEINSIGHT_BIN" ]; then
    fail "CODEINSIGHT_BIN is not executable: $CODEINSIGHT_BIN"
  fi
}

write_file() {
  local path="$1"
  local content="$2"

  mkdir -p "$(dirname "$path")"
  printf "%s\n" "$content" >"$path"
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

require_jq() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query" "$file" >/dev/null; then
    echo "query: $query" >&2
    fail "$description"
  fi
}

create_fixture() {
  local repo="$1"

  write_file "$repo/src/main.ts" 'import { createRouter } from "./router";
import { authenticate } from "./auth";
import { loadConfig } from "./config";
import { bootStartup } from "./startup";

export function main() {
  return bootStartup(createRouter(), authenticate("demo"), loadConfig());
}

main();'
  write_file "$repo/src/router.ts" 'export function createRouter() {
  return { route: "/health" };
}'
  write_file "$repo/src/auth.ts" 'export function authenticate(user: string) {
  return { user, status: "accepted" };
}'
  write_file "$repo/src/permissions.ts" 'export function authorizePermission(token: string) {
  // Authorization permission checks validate the bearer token.
  return { token, permission: "admin" };
}'
  write_file "$repo/src/config.ts" 'export function loadConfig() {
  return { mode: "test" };
}'
  write_file "$repo/src/feature_flags.ts" 'export function evaluateFeatureFlag(flagKey: string) {
  // Feature flag rollout toggles experiment variants for selected users.
  return { flagKey, rollout: "gradual", variant: "enabled" };
}'
  write_file "$repo/src/network.ts" 'export function configureProxyTransport(proxyUrl: string) {
  // Network HTTP adapter follows redirects through the configured proxy.
  return { proxy: proxyUrl, redirect: "follow", transport: "http" };
}'
  write_file "$repo/src/validation.ts" 'export function bindJsonValidationSchema(payload: unknown) {
  // JSON binding validates payloads against the request schema.
  return { payload, schema: "user", validator: "strict" };
}'
  write_file "$repo/src/database.ts" 'export function connectDatabase() {
  // Persist user records in durable storage.
  return { repository: "users", storage: "postgres" };
}'
  write_file "$repo/src/errors.ts" 'export function handleError(error: Error) {
  // Retry timeout failures before falling back to the caller.
  return { retry: true, timeout: error.message.includes("timeout") };
}'
  write_file "$repo/src/retry_transport.ts" 'export function sendWithRetryTimeout(request: { url: string }) {
  // Transport send path handles retry failures and timeout recovery.
  return { request, retry: "once", timeout: 30, recovery: "fallback" };
}'
  write_file "$repo/src/router.test.ts" 'import { createRouter } from "./router";

export function routerRegressionSpec() {
  // Regression coverage for router behavior.
  return createRouter();
}'
  write_file "$repo/src/handler.ts" 'export function handleRequest(request: { path: string }) {
  // API endpoint handler returns the response payload.
  return { response: request.path };
}'
  write_file "$repo/src/cache.ts" 'export function readCachedProfile(cacheKey: string) {
  // Cache performance path optimizes latency for repeated reads.
  return { cacheKey, latency: "low", optimization: "memory-cache" };
}'
  write_file "$repo/src/telemetry.ts" 'export function recordTelemetry(eventName: string) {
  // Observability telemetry emits logs and metrics for monitoring.
  return { eventName, logs: true, metrics: "request_count", trace: "span" };
}'
  write_file "$repo/src/security.ts" 'export function sanitizeSecurityInput(input: string) {
  // Security sanitization guards against injection vulnerabilities.
  return input.replace(/[<>]/g, "");
}'
  write_file "$repo/src/billing.ts" 'export function createCheckoutSession(subscriptionId: string) {
  // Billing payment checkout creates a subscription invoice.
  return { subscription: subscriptionId, payment: "pending", invoice: "draft" };
}'
  write_file "$repo/src/component.tsx" 'export function UserCardComponent() {
  // Frontend UI component renders the profile page layout.
  return <section className="profile-card">profile</section>;
}'
  write_file "$repo/src/worker.ts" 'export function runBackgroundWorker(queueName: string) {
  // Background job worker drains the scheduled queue.
  return { queue: queueName, job: "scheduled-refresh" };
}'
  write_file "$repo/docs/usage.ts" 'export const usageGuide = {
  documentation: "setup examples and usage workflows",
};'
  write_file "$repo/src/startup.ts" 'export function bootStartup(router: unknown, auth: unknown, config: unknown) {
  return { router, auth, config };
}'
  write_file "$repo/src/application.ts" 'export function attach(handler: unknown) {
  return dispatchRequest(handler, { path: "/health" });
}

export function dispatchRequest(handler: unknown, request: { path: string }) {
  // Request lifecycle dispatch runs before hooks and response finalization.
  return finalizeResponse(handler, request);
}

export function finalizeResponse(handler: unknown, request: { path: string }) {
  return { handler, request, response: "ok" };
}'
  write_file "$repo/src/middleware.ts" 'export function attachMiddleware(handler: unknown) {
  // Registers middleware before routes are mounted.
  return { handler, stage: "middleware" };
}'
}

main() {
  require_command jq
  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local repo output_dir summary_json default_output_dir default_summary_json bad_output_dir bad_summary_json expectations_tsv bad_expectations_json
  repo="$TEMP_DIR/repo"
  output_dir="$TEMP_DIR/matrix"
  summary_json="$output_dir/summary.json"
  default_output_dir="$TEMP_DIR/matrix-default"
  default_summary_json="$default_output_dir/summary.json"
  bad_output_dir="$TEMP_DIR/matrix-bad"
  bad_summary_json="$bad_output_dir/summary.json"
  expectations_tsv="$TEMP_DIR/expectations.tsv"
  bad_expectations_json="$TEMP_DIR/bad-expectations.json"
  create_fixture "$repo"
write_file "$expectations_tsv" 'understand routing behavior	src/router.ts
understand authentication behavior	src/auth.ts
understand authorization permissions	src/permissions.ts
understand access control rules	src/permissions.ts
understand application settings	src/config.ts
understand feature flag rollout	src/feature_flags.ts
understand proxy redirect transport	src/network.ts
understand json binding validation	src/validation.ts
understand startup flow	src/startup.ts
understand persistence behavior	src/database.ts
debug retry timeout handling	src/retry_transport.ts
find regression coverage	src/router.test.ts
understand api handler behavior	src/handler.ts
understand cache performance latency	src/cache.ts
understand observability telemetry logs	src/telemetry.ts
understand security sanitization vulnerabilities	src/security.ts
understand checkout subscription payment	src/billing.ts
understand frontend component rendering	src/component.tsx
understand background job queue	src/worker.ts
understand documentation usage	docs/usage.ts
understand request lifecycle before after request handling	src/application.ts
understand middleware behavior	src/middleware.ts'
  write_file "$bad_expectations_json" '[
  {
    "task": "understand routing behavior",
    "expected_first_file": "src/auth.ts"
  }
]'

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/task-routing-matrix.sh" "$repo" \
    --output-dir "$output_dir" \
    --token-budget 1600 \
    --expect-file "$expectations_tsv"

  require_jq "$summary_json" '.status == "pass" and .task_count == 22' "matrix summary should pass"
  require_jq "$summary_json" '.expectations.status == "pass" and .expectations.count == 22' "matrix expectations should pass"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand routing behavior" and .first_file == "src/router.ts")' "routing task should choose router"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authentication behavior" and .first_file == "src/auth.ts")' "authentication task should choose auth"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authorization permissions" and .first_file == "src/permissions.ts")' "authorization task should choose permissions"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand access control rules" and .first_file == "src/permissions.ts")' "access control task should choose permissions"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand application settings" and .first_file == "src/config.ts")' "settings task should choose config"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand feature flag rollout" and .first_file == "src/feature_flags.ts")' "feature flag task should choose feature flags"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand proxy redirect transport" and .first_file == "src/network.ts")' "network task should choose network"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand json binding validation" and .first_file == "src/validation.ts")' "validation task should choose validation"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand startup flow" and .first_file == "src/startup.ts")' "startup task should choose startup"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand persistence behavior" and .first_file == "src/database.ts")' "persistence task should choose database"
  require_jq "$summary_json" '.tasks[] | select(.task == "debug retry timeout handling" and .first_file == "src/retry_transport.ts")' "debug task should choose retry transport"
  require_jq "$summary_json" '.tasks[] | select(.task == "find regression coverage" and .first_file == "src/router.test.ts")' "coverage task should choose test"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand api handler behavior" and .first_file == "src/handler.ts")' "api handler task should choose handler"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand cache performance latency" and .first_file == "src/cache.ts")' "performance task should choose cache"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand observability telemetry logs" and .first_file == "src/telemetry.ts")' "observability task should choose telemetry"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand security sanitization vulnerabilities" and .first_file == "src/security.ts")' "security task should choose security"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand checkout subscription payment" and .first_file == "src/billing.ts")' "billing task should choose billing"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand frontend component rendering" and .first_file == "src/component.tsx")' "frontend task should choose component"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand background job queue" and .first_file == "src/worker.ts")' "background task should choose worker"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand documentation usage" and .first_file == "docs/usage.ts")' "documentation task should choose docs"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand request lifecycle before after request handling" and .first_file == "src/application.ts")' "request lifecycle task should choose application"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand middleware behavior" and .first_file == "src/middleware.ts")' "middleware task should choose middleware"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authentication behavior" and (.first_reading_question | contains("authentication decisions")))' "authentication task should report an auth-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authentication behavior" and (.first_reading_focus | contains("authentication")))' "authentication task should report an auth-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authorization permissions" and (.first_reading_question | contains("authentication decisions")))' "authorization task should report an auth-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authorization permissions" and (.first_reading_focus | contains("authentication")))' "authorization task should report an auth-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand access control rules" and (.first_reading_question | contains("authentication decisions")))' "access control task should report an auth-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand access control rules" and (.first_reading_focus | contains("authentication")))' "access control task should report an auth-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand application settings" and (.first_reading_question | contains("configuration options")))' "settings task should report a config-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand application settings" and (.first_reading_focus | contains("configuration")))' "settings task should report a config-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand feature flag rollout" and (.first_reading_question | contains("feature flags")))' "feature flag task should report a feature-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand feature flag rollout" and (.first_reading_focus | contains("feature flag")))' "feature flag task should report a feature-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand proxy redirect transport" and (.first_reading_question | contains("network requests")))' "network task should report a network-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand proxy redirect transport" and (.first_reading_focus | contains("network client")))' "network task should report a network-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand json binding validation" and (.first_reading_question | contains("inputs validated")))' "validation task should report a validation-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand json binding validation" and (.first_reading_focus | contains("validation")))' "validation task should report a validation-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand startup flow" and (.first_reading_question | contains("startup entrypoint")))' "startup task should report a startup-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand startup flow" and (.first_reading_focus | contains("startup")))' "startup task should report a startup-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand persistence behavior" and (.first_reading_question | contains("database access")))' "persistence task should report a database-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand persistence behavior" and (.first_reading_focus | contains("persistence")))' "persistence task should report a persistence-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "debug retry timeout handling" and (.first_reading_question | contains("retries")))' "debug task should report an error-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "debug retry timeout handling" and (.first_reading_focus | contains("error handling")))' "debug task should report an error-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "find regression coverage" and (.first_reading_question | contains("regression cases")))' "coverage task should report a test-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "find regression coverage" and (.first_reading_focus | contains("regression coverage")))' "coverage task should report a test-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand api handler behavior" and (.first_reading_question | contains("API requests")))' "api handler task should report a handler-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand api handler behavior" and (.first_reading_focus | contains("API handler")))' "api handler task should report a handler-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand cache performance latency" and (.first_reading_question | contains("cache reads")))' "performance task should report a cache-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand cache performance latency" and (.first_reading_focus | contains("cache")))' "performance task should report a cache-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand observability telemetry logs" and (.first_reading_question | contains("logs")))' "observability task should report a telemetry-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand observability telemetry logs" and (.first_reading_focus | contains("logging")))' "observability task should report a telemetry-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand security sanitization vulnerabilities" and (.first_reading_question | contains("security checks")))' "security task should report a security-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand security sanitization vulnerabilities" and (.first_reading_focus | contains("security")))' "security task should report a security-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand checkout subscription payment" and (.first_reading_question | contains("subscription decisions")))' "billing task should report a payment-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand checkout subscription payment" and (.first_reading_focus | contains("billing")))' "billing task should report a payment-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand frontend component rendering" and (.first_reading_question | contains("frontend component")))' "frontend task should report a component-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand frontend component rendering" and (.first_reading_focus | contains("frontend UI")))' "frontend task should report a component-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand background job queue" and (.first_reading_question | contains("scheduled runs")))' "background task should report a worker-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand background job queue" and (.first_reading_focus | contains("background jobs")))' "background task should report a worker-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand documentation usage" and (.first_reading_question | contains("documented workflow")))' "documentation task should report a docs-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand documentation usage" and (.first_reading_focus | contains("documentation")))' "documentation task should report a docs-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand request lifecycle before after request handling" and (.first_reading_question | contains("request lifecycle hooks")))' "request lifecycle task should report a lifecycle-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand request lifecycle before after request handling" and (.first_reading_focus | contains("request lifecycle")))' "request lifecycle task should report a lifecycle-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand request lifecycle before after request handling" and (.first_selection_reason | contains("request lifecycle task")))' "request lifecycle task should report lifecycle selection evidence"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand middleware behavior" and (.first_reading_question | contains("handler boundaries")))' "middleware task should report a middleware-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand middleware behavior" and (.first_reading_focus | contains("middleware")))' "middleware task should report a middleware-specific reading focus"
  grep -Fq '| Task | Seed strategy | First file | Focus | Question |' "$output_dir/task-routing-matrix.md" ||
    fail "matrix markdown should include the Focus column"

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/task-routing-matrix.sh" "$repo" \
    --output-dir "$default_output_dir" \
    --token-budget 1600
  require_jq "$default_summary_json" '.status == "pass" and .task_count == 22' "default matrix summary should include all default tasks"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand authorization permissions" and .first_file == "src/permissions.ts")' "default matrix should include authorization task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand access control rules" and .first_file == "src/permissions.ts")' "default matrix should include access control task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand feature flag rollout" and .first_file == "src/feature_flags.ts")' "default matrix should include feature flag task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand proxy redirect transport" and .first_file == "src/network.ts")' "default matrix should include network task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand json binding validation" and .first_file == "src/validation.ts")' "default matrix should include validation task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand api handler behavior" and .first_file == "src/handler.ts")' "default matrix should include api handler task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand cache performance latency" and .first_file == "src/cache.ts")' "default matrix should include performance task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand observability telemetry logs" and .first_file == "src/telemetry.ts")' "default matrix should include observability task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand security sanitization vulnerabilities" and .first_file == "src/security.ts")' "default matrix should include security task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand checkout subscription payment" and .first_file == "src/billing.ts")' "default matrix should include billing task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand frontend component rendering" and .first_file == "src/component.tsx")' "default matrix should include frontend task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand background job queue" and .first_file == "src/worker.ts")' "default matrix should include background task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "find regression coverage" and .first_file == "src/router.test.ts")' "default matrix should include coverage task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand documentation usage" and .first_file == "docs/usage.ts")' "default matrix should include documentation task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand request lifecycle before after request handling" and .first_file == "src/application.ts")' "default matrix should include request lifecycle task"
  require_jq "$default_summary_json" '.tasks[] | select(.task == "understand middleware behavior" and .first_file == "src/middleware.ts")' "default matrix should include middleware task"

  if CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/task-routing-matrix.sh" "$repo" \
    --output-dir "$bad_output_dir" \
    --token-budget 1600 \
    --expect-file "$bad_expectations_json" >/dev/null 2>&1; then
    fail "matrix should fail when an expected first file does not match"
  fi
  require_jq "$bad_summary_json" '.expectations.status == "fail"' "failed matrix should report expectation failure"
  require_jq "$bad_summary_json" '.expectations.checks[] | select(.task == "understand routing behavior" and .expected_first_file == "src/auth.ts" and .actual_first_file == "src/router.ts" and .status == "fail")' "failed matrix should report expected and actual first files"

  echo "task routing matrix smoke passed"
  echo "summary: $summary_json"
  jq -r '.tasks[] | "\(.task): \(.first_file) (\(.line_reduction))"' "$summary_json"
}

main "$@"
