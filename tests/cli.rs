use std::io::Write;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn cli_indexes_and_queries_fixture_project() {
    let fixture = fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 9);
    assert_eq!(index["changed_files"], 9);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let second_index = run_json(["index", fixture.path().to_str().unwrap()]);
    assert_eq!(second_index["changed_files"], 0);
    assert_eq!(second_index["unchanged_files"], 9);

    let symbols = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "AuthService",
        "--limit",
        "5",
    ]);
    assert_eq!(symbols[0]["name"], "AuthService");

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    let targets = deps["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|dependency| dependency["target"].as_str())
        .collect::<Vec<_>>();
    assert!(targets.contains(&"os"));
    assert!(targets.contains(&"./ui"));
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "./ui" && dependency["resolved_file"] == "src/ui.ts"
            })
    );

    let refs = run_json([
        "find-references",
        fixture.path().to_str().unwrap(),
        "AuthService",
        "--include-definitions",
    ]);
    assert!(refs.as_array().unwrap().iter().any(|reference| {
        reference["file"] == "src/auth.py" && reference["reference_kind"] == "definition"
    }));

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand auth flow",
        "--symbol",
        "AuthService",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(context["symbols"][0]["name"], "AuthService");
    assert!(
        context["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| { file["file"] == "src/auth.py" })
    );
    let auth_context_files = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(auth_context_files.first(), Some(&"src/auth.py"));
    assert!(auth_context_files.contains(&"src/consumer.py"));

    let billing_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand billing payment behavior",
        "--symbol",
        "Service",
        "--token-budget",
        "1600",
    ]);
    let billing_context_files = billing_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(billing_context_files.first(), Some(&"src/billing.py"));

    let main_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand ui flow",
        "--symbol",
        "main",
        "--token-budget",
        "1600",
    ]);
    assert!(
        main_context["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| { file["file"] == "src/ui.ts" })
    );
    let file_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand ui entry file",
        "--file",
        "src/main.ts",
        "--token-budget",
        "1600",
    ]);
    let context_files = file_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(context_files.first(), Some(&"src/main.ts"));
    assert!(context_files.contains(&"src/ui.ts"));

    let long_file_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand late entrypoint",
        "--file",
        "src/long.ts",
        "--token-budget",
        "1600",
    ]);
    let long_file = &long_file_context["files"].as_array().unwrap()[0];
    assert_eq!(long_file["file"], "src/long.ts");
    let long_excerpt = long_file["ranges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|range| range["excerpt"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(long_excerpt.contains("lateEntry"));
    assert!(!long_excerpt.contains("filler_60"));

    let small_budget_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand late entrypoint",
        "--file",
        "src/long.ts",
        "--token-budget",
        "500",
    ]);
    let small_budget_file = &small_budget_context["files"].as_array().unwrap()[0];
    assert_eq!(small_budget_file["file"], "src/long.ts");
    let small_budget_excerpt = small_budget_file["ranges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|range| range["excerpt"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(small_budget_excerpt.contains("import { render }"));
    assert!(!small_budget_excerpt.contains("filler_60"));

    let callers = run_json([
        "callers",
        fixture.path().to_str().unwrap(),
        "helper",
        "--limit",
        "5",
    ]);
    assert_eq!(callers[0]["caller"], "AuthService.login");
    assert_eq!(callers[0]["callee"], "helper");

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "AuthService.login",
        "--limit",
        "5",
    ]);
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| call["callee"] == "helper")
    );

    let imported_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "main",
        "--limit",
        "5",
    ]);
    assert!(
        imported_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "render" && call["callee_file"] == "src/ui.ts" })
    );

    let aliased_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "aliasMain",
        "--limit",
        "5",
    ]);
    assert!(
        aliased_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "draw" && call["callee_file"] == "src/ui.ts" })
    );

    let namespace_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "namespaceMain",
        "--limit",
        "5",
    ]);
    assert!(
        namespace_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "ui.render" && call["callee_file"] == "src/ui.ts" })
    );

    let module_alias_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "moduleAliasMain",
        "--limit",
        "5",
    ]);
    assert!(
        module_alias_callees.as_array().unwrap().iter().any(|call| {
            call["callee"] == "uiModule.render" && call["callee_file"] == "src/ui.ts"
        })
    );

    let default_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "defaultMain",
        "--limit",
        "5",
    ]);
    assert!(
        default_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "drawDefault" && call["callee_file"] == "src/ui.ts" })
    );

    let reexport_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "reexportMain",
        "--limit",
        "5",
    ]);
    assert!(
        reexport_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "relayRender" && call["callee_file"] == "src/ui.ts" })
    );

    let reexport_default_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "reexportDefaultMain",
        "--limit",
        "5",
    ]);
    assert!(
        reexport_default_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "relayDefault" && call["callee_file"] == "src/ui.ts" })
    );

    let export_star_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "exportStarMain",
        "--limit",
        "5",
    ]);
    assert!(
        export_star_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "starRender" && call["callee_file"] == "src/ui.ts" })
    );

    let namespace_reexport_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "namespaceReexportMain",
        "--limit",
        "5",
    ]);
    assert!(
        namespace_reexport_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "uiApi.render" && call["callee_file"] == "src/ui.ts" })
    );

    let two_hop_reexport_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "twoHopReexportMain",
        "--limit",
        "5",
    ]);
    assert!(
        two_hop_reexport_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "finalRender" && call["callee_file"] == "src/ui.ts" })
    );

    let two_hop_default_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "twoHopDefaultMain",
        "--limit",
        "5",
    ]);
    assert!(
        two_hop_default_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "finalDefault" && call["callee_file"] == "src/ui.ts" })
    );

    let two_hop_namespace_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "twoHopNamespaceMain",
        "--limit",
        "5",
    ]);
    assert!(
        two_hop_namespace_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "finalApi.render" && call["callee_file"] == "src/ui.ts"
            })
    );
}

#[test]
fn mcp_stdio_executes_symbol_search() {
    let fixture = fixture_project();
    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "symbol_search",
            "arguments": {
                "root": fixture.path(),
                "query": "AuthService",
                "limit": 3
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(response["id"], 1);
    assert_eq!(
        response["result"]["structuredContent"][0]["name"],
        "AuthService"
    );
}

#[test]
fn mcp_stdio_rejects_invalid_tool_arguments() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "symbol_search",
            "arguments": {
                "root": ".",
                "query": "AuthService",
                "limit": 0
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(response["id"], 2);
    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("limit")
    );
}

fn run_json<const N: usize>(args: [&str; N]) -> Value {
    let output = Command::cargo_bin("codeinsight")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();

    write_file(
        &dir,
        "src/auth.py",
        r#"
import os

class AuthService:
    def login(self):
        return helper()

def helper():
    return os.getenv("USER")
"#,
    );
    write_file(
        &dir,
        "src/billing.py",
        r#"
class BillingService:
    def charge(self):
        return "paid"
"#,
    );
    write_file(
        &dir,
        "src/consumer.py",
        r#"
from auth import AuthService

def build_service():
    return AuthService()
"#,
    );
    write_file(
        &dir,
        "src/main.ts",
        r#"
import { render } from "./ui";
import drawDefault from "./ui";
import { relayRender, relayDefault, render as starRender, uiApi } from "./barrel";
import { finalApi, finalDefault, finalRender } from "./barrel2";
import * as ui from "./ui";
const { render: draw } = require("./ui");
const uiModule = require("./ui");

export function main() {
  render();
}

export function aliasMain() {
  draw();
}

export function namespaceMain() {
  ui.render();
}

export function moduleAliasMain() {
  uiModule.render();
}

export function defaultMain() {
  drawDefault();
}

export function reexportMain() {
  relayRender();
}

export function reexportDefaultMain() {
  relayDefault();
}

export function exportStarMain() {
  starRender();
}

export function namespaceReexportMain() {
  uiApi.render();
}

export function twoHopReexportMain() {
  finalRender();
}

export function twoHopDefaultMain() {
  finalDefault();
}

export function twoHopNamespaceMain() {
  finalApi.render();
}
"#,
    );
    write_file(
        &dir,
        "src/barrel.ts",
        r#"
export { render as relayRender, default as relayDefault } from "./ui";
export * from "./ui";
export * as uiApi from "./ui";
"#,
    );
    write_file(
        &dir,
        "src/barrel2.ts",
        r#"
export { relayRender as finalRender, relayDefault as finalDefault, uiApi as finalApi } from "./barrel";
"#,
    );
    write_file(
        &dir,
        "src/ui.ts",
        r#"
export function render() {
  return "ok";
}

export default function defaultRender() {
  return "default";
}
"#,
    );
    write_file(&dir, "src/long.ts", &long_typescript_file());
    write_file(
        &dir,
        "src/service.go",
        r#"
package service

import "fmt"

func Login() {
  fmt.Println("login")
}
"#,
    );

    dir
}

fn long_typescript_file() -> String {
    let mut source = String::from("\nimport { render } from \"./ui\";\n\n");
    for index in 1..=85 {
        source.push_str(&format!("const filler_{index} = {index};\n"));
    }
    source.push_str("\nexport function lateEntry() {\n  render();\n}\n");
    source
}

fn write_file(dir: &TempDir, path: &str, contents: &str) {
    let mut file = std::fs::File::create(dir.path().join(path)).unwrap();
    file.write_all(contents.as_bytes()).unwrap();
}
