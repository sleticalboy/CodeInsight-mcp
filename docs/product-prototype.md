# CodeInsight MCP Server 产品原型文档

## 1. 产品定位

CodeInsight MCP Server 是一个本地优先的 AI Agent 代码上下文路由器。它通过轻量级代码索引、符号关系分析和面向 AI Agent 的上下文压缩，让 Cursor、Claude Code、Codex 等 AI 编程助手先读对文件、按预算读取上下文，并在改代码前预览影响范围。

一句话定位：

> 面向 AI Agent 的本地代码上下文路由层，让 AI 少读文件、少猜上下文、少浪费 token。

产品边界：

- 不替代 IDE、LSP、编译器、测试框架或 Sourcegraph。
- 不把静态启发式结果包装成确定性语义结论。
- 不默认依赖外部向量数据库、图数据库或托管索引服务。
- 核心价值是给 Agent 一条可追溯的 first-read 路线：`agent_route -> selected context -> suggested_tool -> impact check`。

## 2. 核心用户

### 2.1 个人开发者

典型场景：

- 接手陌生开源项目，快速理解模块结构。
- 想知道某个函数、类、接口在哪里定义和被谁使用。
- 修改代码前，希望知道影响范围。
- 让 AI 助手精准读取相关文件，而不是反复全局搜索。

### 2.2 AI 编程重度用户

典型场景：

- 在 Codex、Claude Code、Cursor 中分析大型仓库。
- 需要给 Agent 提供结构化、可追溯的代码证据。
- 希望降低上下文窗口占用和模型调用成本。
- 希望 AI 在代码审查、重构、Bug 定位时少遗漏关键调用关系。

### 2.3 团队研发与技术负责人

典型场景：

- 评估一个变更会影响哪些模块。
- 辅助代码审查，确认 PR 影响半径。
- 发现技术债、孤立代码、复杂依赖。
- 为内部 AI 开发工具提供统一代码理解能力。

## 3. 产品原则

### 3.1 本地优先

默认所有代码解析、索引和查询都在本机完成。代码不上传到第三方服务。

### 3.2 零配置启动

默认只需要指定项目路径即可运行。MVP 不依赖外部数据库、图数据库或远程向量服务。

### 3.3 Agent 友好

输出不是简单搜索结果，而是可被 AI 直接消费的结构化上下文，包括文件路径、行号、符号、关系、阅读问题、选择理由、后续工具建议和置信度。

### 3.4 渐进增强

基础能力离线可用，高级能力如语义搜索、团队共享索引、远程部署可以作为可选能力加入，但不能成为默认使用路径。

### 3.5 可追溯

任何分析结论都必须能回到具体文件、行号和符号，避免只返回不可验证的自然语言总结。

## 4. MVP 产品范围

MVP 不追求“支持最多语言”，而追求“核心场景闭环可用”。当前 MVP 支持这些语法级基础索引语言：

- JavaScript / TypeScript
- Python
- Go
- Rust
- Java
- C / C++
- C#
- PHP
- Ruby
- Bash / Shell scripts

这些语言用于验证跨语言仓库中的 first-read 路由、文件大纲、依赖关系、引用查找、调用关系和上下文压缩。精度目标是 Agent 导航可用，不承诺 compiler-grade 或 LSP-grade 的完整语义分析。

### 4.1 MVP 核心工作流

#### 工作流 1：理解陌生仓库

用户问题：

> 这个项目的入口在哪里？核心模块有哪些？

推荐入口：

1. `agent_route` 一次性执行 first-read 路线。
2. 返回的 `route[]` 记录 `index_project -> project_overview -> context_pack -> impact_analysis`。
3. Agent 先按 `context_pack.reading_plan[]` 阅读选中文件。
4. 如果选中上下文不足，再执行 `reading_plan[].suggested_tool`。

#### 工作流 2：查找符号和引用

用户问题：

> `PaymentService` 在哪里定义？哪些地方调用了它？

工具链路：

1. `symbol_search` 查找定义。
2. `find_references` 查找引用点。
3. `callers` 返回调用者。
4. `context_pack` 汇总定义、调用链和关键片段。

#### 工作流 3：修改前影响分析

用户问题：

> 如果我改这个函数，会影响哪些地方？

工具链路：

1. `symbol_search` 定位目标符号。
2. `callers` 和 `callees` 获取直接调用关系。
3. `dependency_graph` 找到模块级依赖。
4. `impact_analysis` 输出影响范围、风险文件和建议验证点。

#### 工作流 4：给 Agent 精准上下文

用户问题：

> 修复这个登录 Bug，需要读哪些代码？

工具链路：

1. `symbol_search` 根据关键词查找相关符号。
2. `find_references` 扩展上下游调用点。
3. `context_pack` 按 token 预算输出最相关片段。

## 5. MCP 工具原型

### 5.1 `index_project`

用途：为指定仓库建立或刷新本地索引。

输入：

```json
{
  "root": "/path/to/repo",
  "languages": ["python", "typescript", "go"],
  "force": false
}
```

输出：

```json
{
  "project_id": "local-hash",
  "indexed_files": 1240,
  "symbols": 9821,
  "references": 44210,
  "duration_ms": 1834,
  "cache_hit": false
}
```

### 5.2 `project_overview`

用途：返回项目结构概览。

输出重点：

- 语言分布
- 文件数量和代码行数
- 主要目录
- 入口文件候选
- 测试目录候选
- 配置文件候选

### 5.3 `symbol_search`

用途：按名称搜索符号。

输入：

```json
{
  "root": "/absolute/path/to/repo",
  "query": "PaymentService",
  "limit": 20
}
```

输出：

```json
[
  {
    "name": "PaymentService",
    "qualified_name": "PaymentService",
    "kind": "class",
    "language": "typescript",
    "file": "src/payment/PaymentService.ts",
    "start_line": 12,
    "end_line": 96
  }
]
```

### 5.4 `file_outline`

用途：返回单个文件的结构化大纲。

输出重点：

- imports
- exports
- classes
- functions
- methods
- constants
- symbols range
- 文件摘要

### 5.5 `find_references`

用途：查找某个符号的引用点。

输入：

```json
{
  "root": "/absolute/path/to/repo",
  "symbol": "PaymentService",
  "include_definitions": false,
  "limit": 100
}
```

输出重点：

- 引用文件
- 引用行号
- 引用类型：调用、导入、继承、实现、赋值、读取
- 上下文片段
- 置信度

### 5.6 `callers`

用途：查找调用当前函数或方法的上游符号。

输入：

```json
{
  "root": "/absolute/path/to/repo",
  "symbol": "ValidateToken",
  "limit": 50
}
```

输出重点：

- 静态调用点
- 文件和行号
- 可解析时的 imported target file hint
- 置信度

### 5.7 `callees`

用途：查找当前函数或方法调用了哪些下游符号。

输出结构与 `callers` 类似。

### 5.8 `dependency_graph`

用途：返回模块、包、目录之间的依赖关系。

输入：

```json
{
  "root": "/absolute/path/to/repo",
  "files": ["src/service.cpp"],
  "languages": ["cpp"],
  "kinds": ["base_type"],
  "limit": 500
}
```

输出重点：

- root
- dependencies
- nodes
- edges
- summary
- top_sources
- top_targets
- kind 过滤：例如 `base_type` 仅查看继承、接口实现、trait impl 等类型关系
- 已解析的本地文件路径提示

### 5.9 `impact_analysis`

用途：根据符号或文件返回基础影响范围。

MVP 只做静态近似分析，不承诺完整类型系统精度。

输出重点：

- 直接影响符号
- 间接影响符号
- 相关测试文件候选
- 风险等级
- 建议验证命令候选

### 5.10 `context_pack`

用途：按 token 预算生成 Agent 可消费的代码上下文包。

输入：

```json
{
  "root": "/absolute/path/to/repo",
  "task": "fix login token refresh bug",
  "symbols": ["RefreshToken", "AuthService"],
  "files": ["src/auth/AuthService.ts"],
  "token_budget": 6000
}
```

输出：

```json
{
  "summary": "AuthService validates access tokens and refreshes expired sessions through RefreshTokenStore.",
  "files": [
    {
      "file": "src/auth/AuthService.ts",
      "reason": "Defines primary token validation flow",
      "ranges": [
        {
          "start_line": 18,
          "end_line": 94,
          "importance": "high",
          "reason": "Defines symbol AuthService"
        }
      ]
    }
  ],
  "symbols": [],
  "references": [],
  "estimated_tokens": 4210,
  "truncated": false
}
```

这是 MVP 的核心差异化工具。它不是搜索工具，而是帮助 Agent 选择、压缩和组织代码上下文。

### 5.11 `agent_route`

用途：作为默认 first-read 入口，一次性完成本地索引、项目概览、上下文包选择和修改前影响预览。

输入：

```json
{
  "root": "/absolute/path/to/repo",
  "task": "understand the main application entrypoint",
  "token_budget": 6000
}
```

输出重点：

- `route[]`：说明已经执行的 `index_project`、`project_overview`、`context_pack`、`impact_analysis` 路线。
- `context_pack.files[]`：Agent 应先读的文件和行范围。
- `context_pack.reading_plan[]`：每个文件要回答的问题、选择理由和建议后续工具。
- `execution_plan[]`：客户端或 Agent 下一步应执行的有序动作。
- `impact_analysis`：修改前风险预览。

## 6. 界面与交互原型

MVP 主要以 MCP 工具方式交互，不先做独立 GUI。

### 6.1 CLI

```bash
codeinsight index /path/to/repo
codeinsight overview /path/to/repo
codeinsight agent-route /path/to/repo --task "understand app entrypoint" --token-budget 6000
codeinsight symbols /path/to/repo PaymentService
codeinsight refs /path/to/repo PaymentService
codeinsight serve --transport stdio
```

### 6.2 MCP 配置示例

```json
{
  "mcpServers": {
    "codeinsight": {
      "command": "codeinsight",
      "args": ["serve", "--transport", "stdio"]
    }
  }
}
```

### 6.3 Agent 返回风格

工具输出应避免大段自然语言，优先返回结构化 JSON。

每条结果都应包含：

- `file`
- `start_line`
- `end_line`
- `symbol`
- `kind`
- `reason`
- `confidence`

## 7. 数据模型原型

### 7.1 Project

```text
Project
- id
- root_path
- created_at
- updated_at
- index_version
- language_stats
```

### 7.2 File

```text
File
- id
- project_id
- path
- language
- hash
- line_count
- last_indexed_at
```

### 7.3 Symbol

```text
Symbol
- id
- project_id
- file_id
- name
- qualified_name
- kind
- language
- start_line
- end_line
- signature
- visibility
```

### 7.4 Reference

```text
Reference
- id
- project_id
- source_symbol_id
- target_symbol_id
- file_id
- line
- kind
- confidence
```

### 7.5 Dependency

```text
Dependency
- id
- project_id
- source_file_id
- target_file_id
- source_module
- target_module
- kind
```

## 8. 非功能需求

### 8.1 性能目标

MVP 目标：

- 10 万行以内仓库，冷启动索引小于 30 秒。
- 二次启动增量索引小于 5 秒。
- 符号搜索 P95 小于 200 ms。
- `context_pack` P95 小于 2 秒。

### 8.2 隐私目标

- 默认不联网。
- 默认不上传代码。
- 遥测必须显式开启。
- 索引文件保存在本地用户缓存目录。

### 8.3 稳定性目标

- 单个文件解析失败不能中断整个项目索引。
- 工具输出必须包含错误列表和跳过文件列表。
- 索引版本升级要能自动重建。

### 8.4 兼容性目标

- macOS、Linux 优先。
- Windows 作为 v1.0 后增强目标。
- MCP 传输优先支持 `stdio`，后续支持 Streamable HTTP。

## 9. 竞争差异化

### 9.1 不做 Sourcegraph 替代品

CodeInsight 不以企业级代码搜索平台为第一目标，而是以 AI Agent 本地代码上下文路由为第一目标。

### 9.2 不做纯语义搜索工具

语义搜索只是可选增强。真正核心是 `agent_route`、`context_pack`、阅读计划、后续工具交接和修改前影响预览。

### 9.3 不做 IDE 插件优先

MVP 先把 MCP 工具和 CLI 做稳定，再考虑 IDE 插件。

## 10. 成功指标

### 10.1 MVP 成功指标

- 能在当前支持语言项目中稳定建立索引。
- `agent_route` 能作为默认 first-read 入口，返回阅读顺序、阅读问题、选择理由和可执行后续工具。
- `context_pack` 能明显减少 Agent 需要读取的文件数量。
- `impact_analysis` 能作为修改前风险预览，而不是最终正确性证明。
- 在真实仓库中，常见理解任务能减少 30% 以上上下文读取量。

### 10.2 v1.0 成功指标

- GitHub star 达到 1000。
- 每月活跃安装达到 1000。
- 支持当前主流语言并保持输出 schema 稳定。
- 真实用户反馈中，代码理解和定位问题场景有明确收益。
