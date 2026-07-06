# CodeInsight MCP Server 落地计划

## 1. 总体策略

CodeInsight 不应从“跨语言智能分析全家桶”开始做，而应从一个可验证的最小闭环开始：

> 本地索引代码库，提供符号搜索、引用查找、调用关系和 Agent 上下文压缩。

第一阶段的关键不是语言数量，而是分析结果是否稳定、是否能被 AI Agent 直接使用、是否真的减少 token 消耗。

## 2. 技术路线

### 2.1 推荐技术栈

| 模块 | MVP 选择 | 长期选择 |
|---|---|---|
| 开发语言 | Rust | Rust |
| MCP SDK | Rust MCP SDK 或轻量协议实现 | 官方稳定 SDK |
| 解析引擎 | Tree-sitter | Tree-sitter + LSP/语言服务增强 |
| 本地存储 | SQLite | SQLite / RocksDB |
| 全文检索 | SQLite FTS5 或 Tantivy | Tantivy |
| 向量检索 | 暂不内置 | 可选本地 embedding + Qdrant/pgvector |
| 图关系 | SQLite 表建模 | 可选图数据库适配 |
| 传输协议 | stdio | stdio + Streamable HTTP |
| 分发 | cargo install / Homebrew | cargo、Homebrew、Docker、npm wrapper |

### 2.2 为什么 MVP 不引入外部数据库

pgvector、Qdrant、Neo4j、Apache AGE 都会带来部署成本，与“零配置、本地轻量化”冲突。MVP 应该使用单进程、单二进制、本地缓存文件完成闭环。

长期可以通过插件方式接入外部服务，但不能让它们成为默认启动条件。

## 3. 架构规划

### 3.1 MVP 架构

```text
MCP Client
  |
  | stdio
  v
CodeInsight MCP Server
  |
  +-- Tool Router
  +-- Project Indexer
  +-- Tree-sitter Parser
  +-- Symbol Extractor
  +-- Reference Resolver
  +-- Dependency Analyzer
  +-- Context Pack Builder
  +-- Local Index Store
```

### 3.2 核心模块职责

#### Tool Router

负责注册 MCP 工具、参数校验、调用内部服务、格式化响应。

#### Project Indexer

负责遍历项目文件、识别语言、跳过无效目录、增量更新索引。

需要默认跳过：

- `.git`
- `node_modules`
- `target`
- `dist`
- `build`
- `.venv`
- `vendor`
- `.next`
- `.turbo`

#### Tree-sitter Parser

负责将源代码解析成 AST，并暴露统一节点访问接口。

#### Symbol Extractor

负责从 AST 中提取函数、类、接口、方法、变量、常量等符号。

#### Reference Resolver

负责用静态规则解析引用关系。MVP 接受近似结果，但必须输出 `confidence`。

#### Dependency Analyzer

负责解析 import、require、use、mod、package 等模块依赖。

#### Context Pack Builder

负责根据任务、种子符号和 token 预算选择最重要的文件片段。

这是核心产品壁垒，应作为 MVP 的重点投入模块。

## 4. 最小 MVP

### 4.1 MVP 目标

在 4 到 6 周内做出一个可以真实接入 Codex、Claude Code 或 Cursor 的本地 MCP server，支持对真实仓库进行基础代码理解。

### 4.2 MVP 支持语言

当前 MVP 支持少量核心语言：

- TypeScript / JavaScript
- Python
- Go
- Rust

原因：

- 覆盖大量开源和业务项目。
- Tree-sitter grammar 成熟。
- 依赖关系和符号提取复杂度适中。
- 可以快速验证 Agent 场景。

Java 放到 MVP+，不要阻塞第一版。Rust 已作为自索引和开发验证语言纳入当前支持范围。

### 4.3 MVP 必做功能

| 功能 | 优先级 | 说明 |
|---|---|---|
| `serve --transport stdio` | P0 | MCP 接入基础 |
| `index_project` | P0 | 建立本地索引 |
| `project_overview` | P0 | 项目概览 |
| `symbol_search` | P0 | 按名称查找符号 |
| `file_outline` | P0 | 文件结构化大纲 |
| `find_references` | P1 | 静态引用查找 |
| `dependency_graph` | P1 | 文件/目录依赖 |
| `context_pack` | P0 | Agent 上下文压缩 |
| `callers` / `callees` | P1 | 基础调用链 |

### 4.4 MVP 不做功能

以下能力不进入最小 MVP：

- 语义搜索
- 向量数据库
- 图数据库
- 安全污点分析
- 死代码检测
- PR 审查系统
- Web UI
- 团队共享索引
- Windows 完整兼容
- 16 种语言支持

这些能力都可以做，但不应阻塞第一版真实使用。

### 4.5 MVP 验收标准

功能验收：

- 可以通过 MCP client 调用所有 P0 工具。
- 可以索引至少 3 个真实开源项目。
- 索引失败文件不会导致全局失败。
- `symbol_search` 能定位主要函数、类、方法。
- `context_pack` 可以按 token 预算返回相关文件片段。

性能验收：

- 5 万行项目冷启动索引小于 20 秒。
- 10 万行项目冷启动索引小于 45 秒。
- 二次索引可根据文件 hash 跳过未变化文件。
- 符号查询 P95 小于 200 ms。

质量验收：

- 至少有 20 个 fixture 项目或代码片段测试。
- 每种 MVP 语言至少覆盖函数、类/结构体、导入、导出/包符号。
- 工具输出包含文件路径和行号。
- 关键工具有集成测试。

## 5. 4-6 周执行计划

### Week 1：项目骨架与 MCP 通路

交付物：

- Rust workspace 初始化。
- CLI 入口：`codeinsight`。
- MCP stdio server 能启动。
- 工具注册框架完成。
- `health` 或 `version` 测试工具可调用。

技术任务：

- 选择 Rust MCP SDK 或轻量 JSON-RPC 实现。
- 定义统一错误格式。
- 定义工具响应 schema。
- 建立基础测试和 CI。

### Week 2：索引器与文件扫描

交付物：

- `index_project` 可扫描真实仓库。
- 支持 ignore 规则。
- 支持文件 hash。
- SQLite schema 初版。

技术任务：

- 项目 root 识别。
- 语言识别。
- 文件内容读取和缓存。
- 基础 project、file 表。

### Week 3：Tree-sitter 解析与符号提取

交付物：

- TypeScript/JavaScript、Python、Go、Rust 的 AST 解析。
- `file_outline` 可返回文件大纲。
- `symbol_search` 可按名称查询。

技术任务：

- 接入 tree-sitter grammars。
- 为每种语言实现 symbol extractor。
- 设计统一 Symbol 模型。
- 写 fixture 测试。

### Week 4：引用、依赖与上下文包

交付物：

- `find_references` 初版。
- `dependency_graph` 初版。
- `context_pack` 初版。

技术任务：

- 建立 reference 表。
- 解析 import/require/from/use/package。
- 根据符号、引用、依赖关系计算上下文重要性。
- 实现 token 预算估算。

### Week 5：调用关系与真实仓库验证

交付物：

- `callers` / `callees` 初版。
- 在 3 到 5 个真实仓库中验证。
- 修复解析失败、性能瓶颈和输出可用性问题。

技术任务：

- 基础 callers/callees 查询。
- JavaScript/TypeScript imported target hints。
- 增加性能采样日志。
- 建立真实仓库 smoke test。

### Week 6：打包、文档和首个预览版

交付物：

- `v0.1.0` 预览版本。
- README。
- MCP client 配置示例。
- Homebrew 或 cargo install 分发说明。
- 示例演示仓库。

技术任务：

- 发布二进制。
- 添加基础 GitHub Actions。
- 增加用户反馈模板。
- 编写限制说明和路线图。

## 6. MVP 后路线图

### v0.2：语言扩展与准确率提升

时间：MVP 后 1-2 个月。

目标：

- 完善 Rust 支持并评估 Java 支持。
- 提升引用解析准确率。
- 增加跨文件符号解析。
- 支持 monorepo 多 package 识别。

重点功能：

- 更准确的 import resolution。
- package/module aware symbol lookup。
- 测试文件关联。
- 入口文件识别。

### v0.3：Agent 上下文优化

时间：MVP 后 2-3 个月。

目标：

- 让 `context_pack` 成为产品核心能力。
- 为常见任务提供上下文策略。

重点功能：

- bugfix context mode。
- refactor context mode。
- review context mode。
- test-generation context mode。
- 输出引用证据链。
- token 节省指标报告。

### v0.4：语义搜索可选增强

时间：MVP 后 3-5 个月。

目标：

- 引入可选本地 embedding。
- 支持概念搜索，但不破坏零配置体验。

重点功能：

- 本地 embedding provider。
- 外部 embedding provider 适配。
- 向量索引可选开启。
- 语义结果与符号结果融合排序。

### v1.0：稳定版本

时间：6-9 个月。

目标：

- 支持 8 种主流语言。
- 工具 schema 稳定。
- 索引稳定、性能可预测。
- 能作为日常 AI 编程工作流基础设施使用。

建议支持语言：

- TypeScript / JavaScript
- Python
- Go
- Rust
- Java
- C / C++
- C#
- PHP 或 Ruby

## 7. 长期规划

### Phase 1：本地代码理解层

时间：0-6 个月。

核心能力：

- 本地索引。
- 符号搜索。
- 文件大纲。
- 引用查找。
- 调用关系。
- 依赖图。
- Agent 上下文压缩。

产品目标：

- 成为 AI 编程工具的本地代码理解插件。

### Phase 2：智能分析层

时间：6-12 个月。

核心能力：

- 语义搜索。
- 影响分析。
- 死代码候选检测。
- 重构风险评估。
- PR 影响半径分析。
- 测试推荐。

产品目标：

- 从“查代码”升级为“分析代码变更风险”。

### Phase 3：团队协作层

时间：12-18 个月。

核心能力：

- 团队共享索引。
- 私有部署。
- 权限和审计。
- CI 集成。
- PR bot。
- 历史代码审查知识沉淀。

产品目标：

- 从个人工具升级为团队研发效率基础设施。

### Phase 4：企业代码知识图谱

时间：18 个月以上。

核心能力：

- 跨仓库知识图谱。
- 架构依赖分析。
- 服务边界识别。
- 安全与合规分析。
- 自然语言代码库问答。

产品目标：

- 成为企业内部 AI 研发平台的代码知识底座。

## 8. 商业化规划

### 8.1 开源核心

建议开源内容：

- 本地 MCP server。
- 基础索引。
- 基础符号搜索。
- 文件大纲。
- 基础上下文包。
- 主要语言 parser。

许可证建议：

- Apache 2.0 更利于企业采用。
- 如担心云厂商直接托管变现，可后续评估 Apache 2.0 + 商业附加组件。

### 8.2 Pro 版本

适合个人高级用户。

可收费能力：

- 高级语义搜索。
- 更强上下文压缩策略。
- 大仓库性能优化。
- 多仓库索引。
- 高级影响分析。
- IDE 集成。

### 8.3 Team 版本

适合研发团队。

可收费能力：

- 团队共享索引。
- PR 审查辅助。
- 统一规则配置。
- CI 集成。
- 权限控制。
- 使用统计。

### 8.4 Enterprise 版本

适合大型组织。

可收费能力：

- 私有化部署。
- SSO。
- 审计日志。
- 定制语言支持。
- 内部平台集成。
- SLA 和技术支持。

## 9. 风险与应对

### 9.1 多语言准确率风险

风险：

Tree-sitter 只提供语法树，不等同于完整语义理解。跨文件引用、动态语言调用、类型推断都存在准确率边界。

应对：

- MVP 明确输出 `confidence`。
- 不承诺 100% 静态分析准确率。
- 对核心语言逐步接入语言服务或编译器元数据。

### 9.2 范围膨胀风险

风险：

如果同时做 16 种语言、语义搜索、安全分析、知识图谱，MVP 很容易失控。

应对：

- MVP 限定少量核心语言，避免一次性扩展到 16 种语言。
- 每个版本只围绕一个主目标。
- 把 `context_pack` 定义为第一壁垒。

### 9.3 竞品跟进风险

风险：

Sourcegraph、IDE 厂商、AI 编程工具可能快速补齐代码理解能力。

应对：

- 聚焦本地轻量化和开源生态。
- 深化 MCP 工具体验。
- 建立真实 token 节省 benchmark。

### 9.4 协议演进风险

风险：

MCP 协议、SDK 和传输方式仍在演进。

应对：

- 工具内部逻辑与 MCP 协议层解耦。
- 优先支持 stdio。
- HTTP transport 作为独立 adapter。

## 10. 决策建议

建议立即启动最小 MVP，而不是继续扩大规划范围。

第一阶段只验证三个问题：

1. 能否稳定索引真实仓库？
2. 能否提供比普通 grep 更有用的结构化代码理解？
3. 能否让 AI Agent 少读文件并更快定位关键代码？

如果这三个问题成立，再扩展语义搜索、安全分析、团队协作和商业化能力。
