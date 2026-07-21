#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "update public task routing matrix smoke failed: $*" >&2
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

create_express_like_fixture() {
  local repo="$1"

  write_file "$repo/package.json" '{
  "name": "express-like-route-fixture",
  "main": "index.js"
}'
  write_file "$repo/index.js" 'const factory = require("./lib/express");
const application = require("./lib/application");

module.exports = function startup() {
  return factory.createApplication(application.settings());
};'
  write_file "$repo/lib/express.js" 'exports.createApplication = function createApplication(settings) {
  return createExpressApplicationRoutingBehavior(settings);
};

exports.createExpressApplicationRoutingBehavior = function createExpressApplicationRoutingBehavior(settings) {
  // Express application routing behavior mounts the router and route table.
  return { route: "/health", router: "express", application: "app", settings };
};

exports.static = function staticFileServing(root) {
  // Express static file serving behavior exposes static assets from a filesystem root.
  return { root, static: true, assets: "served" };
};

exports.json = function requestBodyParser(options) {
  // Express request body parsing behavior binds JSON payloads by content type.
  return { parser: "json", body: "parsed", options };
};'
  write_file "$repo/lib/request.js" 'exports.query = function queryParameterParser(url, parser) {
  // Express query parameter parsing behavior parses request query strings.
  return parser(url).query;
};'
  write_file "$repo/lib/application.js" 'exports.settings = function settings() {
  return { env: "test", middleware: ["logger"] };
};

exports.middleware = function middleware(request, next) {
  // Express error handling behavior delegates failures to application finalhandler.
  return next(request);
};

exports.param = function routeParameter(name, fn) {
  // Express route parameter behavior registers route params before handlers.
  return { name, fn, route: "parameter" };
};

exports.method = function httpMethodRouting(method, path, handler) {
  // Express HTTP method routing behavior registers verbs and dispatches handlers.
  return { method, path, handler, route: "method" };
};

exports.mountRouter = function mountedAppRouter(path, router) {
  // Express mounted app router behavior attaches nested routers below an app path.
  return { path, router, route: "mounted" };
};

exports.dispatchRequest = function requestDispatchLifecycle(request, response, next) {
  // Express request dispatch lifecycle behavior enters middleware and finalizes responses.
  return next(request, response);
};

exports.finalHandler = function finalHandler404(request, response, callback) {
  // Express 404 not found final handler behavior decides route miss fallbacks.
  return callback ? callback(request, response) : { status: 404, route: "miss" };
};'
  write_file "$repo/lib/response.js" 'exports.render = function renderResponse(view, options) {
  // Express response rendering behavior sends rendered templates as HTTP output.
  return { view, options, response: "rendered", output: "html" };
};

exports.redirect = function redirectResponse(location, status) {
  // Express redirect response behavior sets the Location header and redirect status code.
  return { location, status, response: "redirect" };
};

exports.setHeader = function setResponseHeader(name, value) {
  // Express response header behavior sets response metadata before the body is sent.
  return { name, value, response: "header" };
};

exports.cookie = function setResponseCookie(name, value, options) {
  // Express response cookie behavior appends Set-Cookie headers with cookie options.
  return { name, value, options, response: "cookie" };
};'
}

main() {
  require_command jq
  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local repo snapshot summary_snapshot
  repo="$TEMP_DIR/repo"
  snapshot="$TEMP_DIR/public-task-routing-matrix.md"
  summary_snapshot="$TEMP_DIR/public-task-routing-matrix-summary.json"
  create_express_like_fixture "$repo"

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/update-public-task-routing-matrix.sh" \
    --output "$snapshot" \
    --summary-output "$summary_snapshot" \
    --case express \
    --root "express=$repo" \
    --token-budget 1600 >/dev/null

  grep -Fq 'Snapshot generated by `scripts/update-public-task-routing-matrix.sh`' "$snapshot" ||
    fail "snapshot should include generation note"
  grep -Fq "expectations: 17/17" "$snapshot" ||
    fail "snapshot should include expectation pass count"
  grep -Fq "line_reduction:" "$snapshot" ||
    fail "snapshot should include aggregate line reduction"
  grep -Fq 'Summary JSON: `<output-dir>/summary.json`' "$snapshot" ||
    fail "snapshot should normalize summary path"
  grep -Fq '| Task | First file | Focus | Question | Suggested tool | Seed strategy | Reduction | Tokens | Impact |' "$snapshot" ||
    fail "snapshot should include first-read focus and question columns"
  grep -Fq '| understand express application routing behavior | `lib/express.js` |' "$snapshot" ||
    fail "snapshot should include express first-read route"
  grep -Fq '| `file_outline` | `auto_task_match` |' "$snapshot" ||
    fail "snapshot should include suggested tool handoff"
  grep -Fq 'docs/task-routing-expectations/express.tsv' "$snapshot" ||
    fail "snapshot should normalize expectation file path"
  if grep -Fq "$TEMP_DIR" "$snapshot"; then
    fail "snapshot should not include temporary absolute paths"
  fi
  jq -e \
    '.generated_by == "scripts/update-public-task-routing-matrix.sh"
      and .status == "pass"
      and .aggregate.task_count == 17
      and .aggregate.expectation_count == 17
      and .aggregate.total_task_source_lines > .aggregate.total_selected_lines
      and .aggregate.line_reduction > 0
      and .cases[0].repository == "<case-root>/express"
      and .cases[0].summary_json == "<output-dir>/express/summary.json"
      and .cases[0].expect_file == "docs/task-routing-expectations/express.tsv"
      and (.cases[0].routes[0].first_reading_focus | type == "string" and length > 0)
      and (.cases[0].routes[0].first_reading_question | type == "string" and length > 0)
      and (.cases[0].routes[0].first_suggested_tool | type == "string" and length > 0)' \
    "$summary_snapshot" >/dev/null ||
    fail "summary snapshot should be normalized JSON evidence"

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/update-public-task-routing-matrix.sh" \
    --check \
    --output "$snapshot" \
    --summary-output "$summary_snapshot" \
    --case express \
    --root "express=$repo" \
    --token-budget 1600 >/dev/null

  local stub_script no_args_snapshot no_args_summary no_args_log
  stub_script="$TEMP_DIR/public-task-routing-matrix-stub.sh"
  no_args_snapshot="$TEMP_DIR/no-args-public-task-routing-matrix.md"
  no_args_summary="$TEMP_DIR/no-args-public-task-routing-matrix-summary.json"
  no_args_log="$TEMP_DIR/no-args-public-task-routing-matrix.log"
  cat >"$stub_script" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

output_dir=""
output=""
summary_json=""
log="${CODEINSIGHT_PUBLIC_TASK_ROUTING_MATRIX_STUB_LOG:?}"
printf '%s\n' "$*" >>"$log"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-dir)
      output_dir="$2"
      shift 2
      ;;
    --output)
      output="$2"
      shift 2
      ;;
    --summary-json)
      summary_json="$2"
      shift 2
      ;;
    *)
      echo "unexpected pass-through argument: $1" >&2
      exit 1
      ;;
  esac
done

mkdir -p "$output_dir" "$(dirname "$output")" "$(dirname "$summary_json")"
cat >"$output" <<MARKDOWN
# CodeInsight Public Task Routing Matrix

- Summary JSON: $summary_json

## Evidence Summary

- cases: 0
- tasks: 0
- expectations: 0/0
MARKDOWN
cat >"$summary_json" <<JSON
{
  "status": "pass",
  "output": "$output",
  "output_dir": "$output_dir",
  "case_count": 0,
  "cases": [],
  "aggregate": {
    "task_count": 0,
    "expectation_count": 0,
    "total_task_source_lines": 0,
    "total_selected_lines": 0,
    "line_reduction": 0
  }
}
JSON
EOF
  chmod +x "$stub_script"

  CODEINSIGHT_PUBLIC_TASK_ROUTING_MATRIX_STUB_LOG="$no_args_log" \
    CODEINSIGHT_PUBLIC_TASK_ROUTING_MATRIX_SCRIPT="$stub_script" \
    "$ROOT_DIR/scripts/update-public-task-routing-matrix.sh" \
      --output "$no_args_snapshot" \
      --summary-output "$no_args_summary" >/dev/null

  CODEINSIGHT_PUBLIC_TASK_ROUTING_MATRIX_STUB_LOG="$no_args_log" \
    CODEINSIGHT_PUBLIC_TASK_ROUTING_MATRIX_SCRIPT="$stub_script" \
    "$ROOT_DIR/scripts/update-public-task-routing-matrix.sh" \
      --check \
      --output "$no_args_snapshot" \
      --summary-output "$no_args_summary" >/dev/null

  if grep -Fq -- '--case' "$no_args_log"; then
    fail "no-argument check should not pass public matrix case arguments"
  fi

  echo "update public task routing matrix smoke passed"
}

main "$@"
