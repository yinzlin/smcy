---
name: hexagonal-layer-guard
description: Enforces strict hexagonal architecture layer boundaries for this repo. Use when modifying Rust source under crates or reviewing new code to verify it obeys the 5 rules. Run the guard script after any change and fix violations before proceeding. Do not use for frontend-only changes or non-code tasks.
---

# Hexagonal Layer Guard

本仓库采用**严格六边形架构**（Phase 0-4 已全部完成）。任何 Rust 代码变更后必须执行护栏验证，违规立即修复。

## 护栏脚本

```powershell
.\scripts\check_layers.ps1          # 全量 5 条规则
.\scripts\check_layers.ps1 -Rule 3  # 单条规则
.\make.ps1 check-layers             # 标准调用入口
```

退出码 0 = 全绿，非 0 = 有违规。

## 5 条硬约束（详见 `.trae/documents/六边形分层规则.md`）

| 规则 | 断言 | 验证方式 |
|---|---|---|
| R1 domain 仅 serde | `domain/Cargo.toml` 无额外依赖 | grep `^\s*\w+` 排除 serde |
| R2 application 零持久化 | `application/src/**/*.rs` 无 `pool()` / `sqlx::` | grep 正则 |
| R3 domain 零 HTTP 契约 | `domain/src/**/*.rs` 无 `pub struct XxxReq {` | grep `Req\s*\{` |
| R4 backend 仅 di.rs 持 pool | `backend/src/**/*.rs`（除 di.rs）无 `pool()` | grep 排除 di.rs |
| R5 application Cargo.toml 零 db/sqlx | `application/Cargo.toml` 无 `database=` / `sqlx=` | grep |

## 新增业务模块的标准路径

严格按此流程，任何逆向都会触发护栏违规：

```
1. domain/src/port/<mod>.rs        定义 trait + 自有输入/行结构体（不含 serde）
2. database/src/sqlite/repo/<mod>.rs  实现 trait
3. application/src/dto/<mod>.rs     写 *Req（HTTP 契约，serde default）+ From 映射
4. application/src/service/<mod>.rs  用例，入口 shadow 转换 Req → Input
5. application/src/port/<mod>.rs    入站端口 trait（可引用 application::dto）
6. backend/src/api/handlers.rs      适配层，State<App> 取用例
7. backend/src/di.rs                注入 service + 必要的 repo 句柄
```

## 常见违规与修复

| 症状 | 根因 | 修复 |
|---|---|---|
| `application/src` 出现 `sqlx::query` | 偷懒绕过端口 | 把 SQL 搬到 `database/src/sqlite/repo/`，在 `domain/src/port/` 加 trait，经注入的 repo 句柄调用 |
| `backend/src/middleware` 出现 `pool()` | 中间件直连连接池 | 函数签名加 `&App`，内部改经注入的 repo 调用 |
| `domain/src` 出现 `pub struct XxxReq {` | HTTP 契约落到领域层 | 移到 `application/src/dto/`，在对应 `port/*.rs` 定义 domain 自有输入结构体 |
| `application/Cargo.toml` 出现 `database =` | application 穿透依赖 | 移除依赖，补齐 domain trait + database 实现 |

## 持久化访问唯一路径

全局 `pool()` **只允许**在 `backend/src/di.rs` 出现一次。任何其他位置（包括 middleware、handlers）直接调用 pool 都是违规。正确做法：

1. 在 `di.rs` 把 repo 句柄挂到 `App` 结构体
2. handler/middleware 从 `State<App>`（或签名参数）取 repo
3. 所有 SQL 封装在 `database/src/sqlite/repo/*.rs` 内

## 错误映射链

```
DomainError (4 variants, 无 HTTP 语义)
  └─ From<DomainError> for UseCaseError
UseCaseError (7 variants)
  └─ backend 适配器映射为 HTTP 状态码
```

下层禁止引入 `StatusCode`；文案由 backend 唯一决定。

## 接口兼容红线

- 响应/请求 JSON 字段名与顺序**不得改变**（serde 序列化顺序 = 字段声明顺序）
- 金额一律 `f64`、日期一律 `String('YYYY-MM-DD')`
- 任何整改后必须 `scripts/smoke.ps1` PASS=232 FAIL=0 才能提交

## 已完成阶段

| Phase | 提交 | 说明 |
|---|---|---|
| 0 | `498ebe4` | ADR 定稿 |
| 1 | `f6ee911` | 中间件端口化 |
| 2 | `12f8bbb` | application 去穿透 |
| 3 | `4e634dd` | 21 个 *Req 迁移至 dto |
| 4 | `0c57bb5` | 护栏脚本 + auth 端口化补漏 |

## 提交规范

遵循仓库 `.trae/rules/git-commit-message.md`：Conventional Commits 混合模式（type 英文 + scope/描述中文）。示例：`refactor(六边形): 新增 *Module* 域端口与仓储实现`。
