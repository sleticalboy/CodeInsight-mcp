#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "public task routing matrix smoke failed: $*" >&2
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

  local repo output_dir summary_json output_log
  repo="$TEMP_DIR/repo"
  output_dir="$TEMP_DIR/output"
  summary_json="$output_dir/summary.json"
  output_log="$TEMP_DIR/output.log"
  create_express_like_fixture "$repo"

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/public-task-routing-matrix.sh" \
    --case express \
    --root "express=$repo" \
    --output-dir "$output_dir" \
    --token-budget 1600 | tee "$output_log"

  require_jq "$summary_json" '.status == "pass" and .case_count == 1' "aggregate summary should pass"
  require_jq "$summary_json" '.aggregate.task_count == 17 and .aggregate.expectation_count == 17' "express expectation count should be aggregated"
  require_jq "$summary_json" '.aggregate.total_task_source_lines > .aggregate.total_selected_lines' "aggregate should include source line baseline"
  require_jq "$summary_json" '.aggregate.line_reduction > 0' "aggregate should include line reduction"
  require_jq "$summary_json" '.cases[] | select(.case == "express" and .task_count == 17)' "express case should be present"
  require_jq "$summary_json" 'all(.cases[].routes[]; (.first_seed_value | type == "string" and length > 0))' "routes should expose first seed values"
  require_jq "$summary_json" 'all(.cases[].routes[]; (.route_quality_level | type == "string" and length > 0)
    and (.route_quality_score | type == "number")
    and (.route_quality_evidence_count | type == "number")
    and (.route_quality_recommended_action | type == "string" and length > 0)
    and (.route_quality_decision_summary | type == "string" and length > 0)
    and (.route_quality_confidence_factors | type == "array")
    and (.route_quality_confidence_factors | length > 0)
    and (.route_quality_verification_steps | type == "array")
    and (.route_quality_verification_steps | length > 0)
    and (.route_quality_warnings | type == "array"))' "public routes should expose route quality evidence"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express application routing behavior" and .first_file == "lib/express.js")' "routing task should choose express entry"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand middleware behavior" and .first_file == "lib/application.js")' "middleware task should choose application"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand startup flow" and .first_file == "index.js")' "startup task should choose index"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express response rendering behavior" and .first_file == "lib/response.js")' "response rendering task should choose response"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express error handling behavior" and .first_file == "lib/application.js")' "error handling task should choose application"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express static file serving behavior" and .first_file == "lib/express.js")' "static file serving task should choose express"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express request body parsing behavior" and .first_file == "lib/express.js")' "request body parsing task should choose express"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express redirect response behavior" and .first_file == "lib/response.js")' "redirect response task should choose response"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express query parameter parsing behavior" and .first_file == "lib/request.js")' "query parameter parsing task should choose request"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express response header behavior" and .first_file == "lib/response.js")' "response header task should choose response"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express route parameter behavior" and .first_file == "lib/application.js")' "route parameter task should choose application"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express response cookie behavior" and .first_file == "lib/response.js")' "response cookie task should choose response"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express HTTP method routing behavior" and .first_file == "lib/application.js")' "HTTP method routing task should choose application"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express mounted app router behavior" and .first_file == "lib/application.js")' "mounted router task should choose application"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express request dispatch lifecycle behavior" and .first_file == "lib/application.js")' "request dispatch lifecycle task should choose application"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express 404 not found final handler behavior" and .first_file == "lib/application.js")' "route miss task should choose application"
  grep -Fq "evidence summary" "$output_log" ||
    fail "terminal output should include evidence summary"
  grep -Fq "expectations: 17/17" "$output_log" ||
    fail "terminal output should include expectation pass count"
  grep -Fq "line_reduction:" "$output_log" ||
    fail "terminal output should include aggregate line reduction"
  grep -Fq "express: 17 tasks, first files index.js, lib/application.js, lib/express.js, lib/request.js, lib/response.js" "$output_log" ||
    fail "terminal output should include first file summary"
  grep -Fq "## Evidence Summary" "$output_dir/public-task-routing-matrix.md" ||
    fail "markdown output should include evidence summary"
  grep -Fq '| Task | First file | Focus | Question | Suggested tool | Quality | Seed strategy | First seed | Reduction | Tokens | Impact |' "$output_dir/public-task-routing-matrix.md" ||
    fail "markdown output should include quality and first seed columns"
  grep -Fq "## Route Quality Evidence" "$output_dir/public-task-routing-matrix.md" ||
    fail "markdown output should include route quality evidence section"

  echo "public task routing matrix smoke passed"
}

main "$@"
