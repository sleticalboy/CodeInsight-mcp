#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "framework entrypoint demo failed: $*" >&2
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

require_jq() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query" "$file" >/dev/null; then
    echo "query: $query" >&2
    fail "$description"
  fi
}

json_value() {
  local file="$1"
  local query="$2"

  jq -r "$query" "$file"
}

create_fixture() {
  local repo="$1"

  write_file "$repo/app/page.tsx" 'export default function Page() { return <main>Dashboard</main>; }'
  write_file "$repo/pages/_app.tsx" 'export default function App({ Component, pageProps }) { return <Component {...pageProps} />; }'
  write_file "$repo/config/routes.rb" 'Rails.application.routes.draw do
  root "dashboard#index"
end'
  write_file "$repo/src/BillingApplication.java" 'package fixture;

public class BillingApplication {
}'
  write_file "$repo/manage.py" 'from django.core.management import execute_from_command_line

if __name__ == "__main__":
    execute_from_command_line()'
  write_file "$repo/project/asgi.py" 'from django.core.asgi import get_asgi_application

application = get_asgi_application()'
  write_file "$repo/project/wsgi.py" 'from django.core.wsgi import get_wsgi_application

application = get_wsgi_application()'
  write_file "$repo/project/urls.py" 'from django.urls import path

urlpatterns = [
    path("", lambda request: None),
]'
  write_file "$repo/src/Program.cs" 'var builder = WebApplication.CreateBuilder(args);
var app = builder.Build();
app.Run();'
  write_file "$repo/src/Startup.cs" 'public class Startup
{
    public void Configure()
    {
    }
}'
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

main() {
  require_command jq
  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local repo overview_json launch_json routes_json urls_json csharp_json
  repo="$TEMP_DIR/framework-repo"
  overview_json="$TEMP_DIR/overview.json"
  launch_json="$TEMP_DIR/launch-context.json"
  routes_json="$TEMP_DIR/routes-context.json"
  urls_json="$TEMP_DIR/urls-context.json"
  csharp_json="$TEMP_DIR/csharp-context.json"

  create_fixture "$repo"

  "$CODEINSIGHT_BIN" index "$repo" --force >/dev/null
  "$CODEINSIGHT_BIN" overview "$repo" >"$overview_json"
  "$CODEINSIGHT_BIN" context-pack "$repo" --task "understand launch sequence" --token-budget 1200 >"$launch_json"
  "$CODEINSIGHT_BIN" context-pack "$repo" --task "understand routes" --token-budget 1200 >"$routes_json"
  "$CODEINSIGHT_BIN" context-pack "$repo" --task "understand django urls" --token-budget 1200 >"$urls_json"
  "$CODEINSIGHT_BIN" context-pack "$repo" --task "understand csharp startup" --token-budget 1200 >"$csharp_json"

  require_jq "$overview_json" '.entrypoints[] | select(.file == "app/page.tsx" and .reason == "Next.js app router entrypoint")' "Next.js app router entrypoint should be detected"
  require_jq "$overview_json" '.entrypoints[] | select(.file == "config/routes.rb" and .reason == "Rails route entrypoint")' "Rails route entrypoint should be detected"
  require_jq "$overview_json" '.entrypoints[] | select(.file == "project/urls.py" and .reason == "Python web framework entrypoint")' "Python urls entrypoint should be detected"
  require_jq "$overview_json" '.entrypoints[] | select(.file == "src/Program.cs" and .reason == "C# web application entrypoint")' "C# Program entrypoint should be detected"
  require_jq "$overview_json" '.entrypoints[] | select(.file == "src/Startup.cs" and .reason == "C# web application entrypoint")' "C# Startup entrypoint should be detected"

  require_jq "$launch_json" '.seed_strategy == "auto_entrypoint" and .files[0].file == "app/page.tsx"' "launch task should start at the strongest framework entrypoint"
  require_jq "$routes_json" '.files[0].file == "config/routes.rb" and (.selected_seeds[0].matched_keywords | index("routes"))' "routes task should start at Rails routes"
  require_jq "$urls_json" '.files[0].file == "project/urls.py" and (.selected_seeds[0].matched_keywords | index("urls"))' "Django urls task should start at urls.py"
  require_jq "$csharp_json" '.files[0].file == "src/Startup.cs" and (.selected_seeds[0].matched_keywords | index("startup"))' "C# startup task should start at Startup.cs"

  echo "framework entrypoint demo passed"
  echo "fixture_root: $repo"
  echo "overview_entrypoints: $(json_value "$overview_json" '.entrypoints | length')"
  echo "first_entrypoint: $(json_value "$overview_json" '.entrypoints[0].file')"
  echo "launch_first_context: $(json_value "$launch_json" '.files[0].file')"
  echo "routes_first_context: $(json_value "$routes_json" '.files[0].file')"
  echo "urls_first_context: $(json_value "$urls_json" '.files[0].file')"
  echo "csharp_first_context: $(json_value "$csharp_json" '.files[0].file')"
}

main "$@"
