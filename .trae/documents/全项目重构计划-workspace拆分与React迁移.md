# 全项目重构计划：六边形 Workspace 拆分 + React 前端迁移

## Context

项目当前是单 crate 巨石结构：src/main.rs（路由 257 条 + Excel 导出 + 托盘）、api/mod.rs（约 17k 行）、pages/mod.rs（约 14k 行内嵌原生前端）、db.rs、models.rs、auth.rs、utils.rs。生产运行于 Win7 单 exe（x86_64-win7-windows-msvc + build-std + nightly + crt-static）。

目标：按 `docs/Rust 全栈项目架构整合方案.md`（将迁移至 `.trae/documents/`）重构为 Cargo workspace 六边形分层，frontend 由 Dioxus 改为 React+AntD，**目录结构与方案文档严格同构**（暂不用的目录留空占位，可按需增加必要文件），完成后优化清理使项目与方案同构。

用户已确认的决策：
1. **分阶段**：阶段一后端拆 workspace（UI/行为完全不变，随时可上线）；阶段二 React 骨架 + 核心页迁移，老页面共存可回退
2. database 仅 SQLite 实现，但 postgres/ redis/ 子目录按同构要求留空占位
3. React 先骨架 + 核心页（登录/商品/采购订单/销售订单/今日进价采集/价格策略）
4. 托盘（tray-icon + tao）保留在 backend 入口
5. docs/ 下文档迁移到 .trae/documents/

---

## 阶段一：后端 Workspace 拆分（无行为变更）

### 目标结构（与方案文档严格同构，※ = 留空占位，＋ = 同构基础上的必要新增）

```
d:\projects\smcy\
├── Cargo.toml                          # [workspace] + workspace.dependencies
├── .cargo/config.toml                  # win7 target 配置不动
├── rust-toolchain.toml                 # ＋ 锁定 nightly 日期，防工具链漂移
├── .env.example                        # DATABASE_URL、BIND_ADDR（80 端口硬编码顺手提取，默认值不变）
├── .rustfmt.toml                       # max_width=120
├── clippy.toml                         # cognitive-complexity 放宽（存量）
├── Makefile                            # dev/build-win7/test/lint/fmt/package
├── make.ps1                            # ＋ Windows 主用脚本（与 Makefile 同命令集）
├── docker/
│   ├── Dockerfile                      # ※ 占位（单机 exe 部署不用，留空注释说明）
│   ├── docker-compose.yml              # ※ 占位
│   └── docker-compose.prod.yml         # ※ 占位
├── migrations/
│   └── .keep                           # ※ 占位（迁移逻辑仍在 database crate 内）
├── .github/workflows/
│   ├── ci.yml                          # fmt → clippy → test → host build
│   └── cd.yml                          # ※ 占位（手动发版，后续补）
├── .trae/documents/                    # ← docs/ 下两份文档迁入此目录
└── crates/
    ├── domain/
    │   ├── Cargo.toml                  # serde、chrono
    │   ├── src/
    │   │   ├── lib.rs                  # ← src/models.rs 全部结构体原样迁入
    │   │   ├── aggregate/.keep         # ※
    │   │   ├── value_object/.keep      # ※
    │   │   ├── service/.keep           # ※
    │   │   ├── event/.keep             # ※
    │   │   ├── port/.keep              # ※
    │   │   └── error/.keep             # ※
    │   └── tests/.keep                 # ※
    ├── application/
    │   ├── Cargo.toml                  # sqlx、rust_xlsxwriter、calamine、chrono、serde、serde_json + domain/database/common
    │   ├── src/
    │   │   ├── lib.rs
    │   │   ├── service/                # 业务服务函数（见下表）
    │   │   ├── dto/.keep               # ※
    │   │   ├── port/.keep              # ※
    │   │   └── error/.keep             # ※
    │   └── tests/.keep                 # ※
    ├── backend/                        # bin crate（唯一可执行）
    │   ├── Cargo.toml                  # axum、tokio、bytes、bcrypt、rand、mime、tray-icon、tao + 四 crate
    │   └── src/
    │       ├── main.rs                 # fn main() + 托盘（EventLoop + TrayIcon 保留此处）
    │       ├── di.rs                   # 启动组装：init_pool → 迁移补调 → 构建 Router
    │       ├── api/
    │       │   ├── routes.rs           # ← main.rs 的 257 条路由表整体迁入
    │       │   ├── handlers.rs         # ← api/mod.rs 整体迁入（改名）
    │       │   └── mod.rs
    │       ├── middleware/
    │       │   └── auth.rs             # ← src/auth.rs（路由权限映射 + session 检查）
    │       ├── pages/mod.rs            # ＋ ← pages/mod.rs 老原生前端原样保留
    │       └── static_assets.rs        # ＋ ← main.rs 的 include_str! 静态资源 handler
    ├── database/
    │   ├── Cargo.toml                  # sqlx(sqlite)、anyhow、tokio
    │   └── src/
    │       ├── lib.rs                  # DB_POOL、init_pool()、pool()、init_tables()、repair_db_corruption()
    │       ├── sqlite/mod.rs           # ← db.rs 的建表/迁移 SQL
    │       ├── postgres/.keep          # ※ 同构占位，仅 SQLite 实现
    │       └── redis/.keep             # ※ 同构占位
    └── common/
        ├── Cargo.toml                  # axum(IntoResponse)、rust_xlsxwriter、chrono、serde
        └── src/
            ├── lib.rs
            ├── types/.keep             # ※
            ├── utils/mod.rs            # ← src/utils.rs（xlsx_response/layout_html/parse_csv 等）
            └── macros/.keep            # ※

crates/frontend/                        # 阶段二建设：React 项目（不进 Cargo workspace，见阶段二）
```

### 代码搬迁映射（除下表外均为原样移动）

| 现有位置 | 目标 | 备注 |
|---|---|---|
| src/models.rs | domain/src/lib.rs | 纯结构体 |
| src/utils.rs | common/src/utils/mod.rs | tray_icon 相关函数移到 backend（common 不带托盘依赖） |
| src/db.rs | database/src/lib.rs + sqlite/mod.rs | |
| src/auth.rs | backend/src/middleware/auth.rs | |
| src/api/mod.rs（17k 行） | backend/src/api/handlers.rs | 仅改引用路径 |
| src/pages/mod.rs（14k 行） | backend/src/pages/mod.rs | 老前端不动 |
| src/main.rs 路由表（约 L2106-L2366） | backend/src/api/routes.rs | |
| src/main.rs `fn main()` + 托盘 | backend/src/main.rs | |
| src/main.rs `build_accept_excel`（约 L1489 起） | application/src/service/excel.rs | |
| main.rs 中被 api 反调的业务函数：`rebuild_price_schedule_end_dates`、`lookup_effective_price`、`update_product_purchase_prices`、`log_price_change`、`recalc_base_price_by_markup`、`compute_stock_summary(_reimburse)`、`get_purchase_order_with_items`、`get_user_by_id`、`check_sales_order_access`、`build_purchase_order_export_workbook` | application/src/service/*.rs | 消除 main↔api 反向调用 |
| api/mod.rs 内三处复用服务：`auto_mark_gov_status`(L3671)、`sync_product_base_price`(L3701)、`write_stock_movements_for_audit`(L5016) | application/src/service/*.rs | 其余 handler 不动 |

### 引用批量替换

- `crate::db::pool()` → `database::pool()`（数百处全局替换）
- `crate::models::` → `domain::`；`crate::utils::` → `common::utils::`
- api 对 main.rs 函数的反调 `crate::xxx` → `application::service::xxx`

### 循环依赖处理

db.rs 迁移期回调 `crate::rebuild_price_schedule_end_dates` 会形成 database→application 反向依赖。解法：database 迁移函数改为返回"待修复 product_id 列表"，由 backend 的 di.rs 启动时补调 application 函数。

### 阶段一验证

1. `cargo check` workspace 全绿后才删除旧 src/
2. `cargo +nightly build --release`（win7 target + build-std）产出 exe
3. 冒烟：/api/login/check、/api/query/overview、采购/销售页面、导出验收单 Excel、进价采集保存
4. 路由 257 条不变、cookie session 不变、托盘行为不变

---

## 阶段二：React 前端（crates/frontend/，不进 workspace）

- **栈**：Vite 5 + React 18 + TS + AntD 5（zhCN + dayjs 中文）+ React Router 6 + Zustand + TanStack Query
- **目录**：`crates/frontend/`（对齐方案文档的 frontend 位置，以 package.json 替代 Cargo.toml；workspace members 显式排除）
- **Vite**：`build.target: 'chrome87'`（兼容 Win7 Chrome 109）；dev proxy `/api`、`/static` → 本地后端
- **嵌入共存**：backend 加 `rust-embed` 嵌入 `crates/frontend/dist`，挂 `/app/*path`（SPA fallback 回 index.html）；老页面路由原样保留，每迁完一页老路由 302 → `/app/xxx`，旧页留 `/legacy/xxx` 回退；单 exe 部署形态不变
- **鉴权**：复用 HttpOnly cookie session，React 同源 fetch 自动带 cookie；`POST /api/login`、`GET /api/login/check` 存 Zustand；权限点镜像 `has_permission_point`
- **核心页顺序**（每页独立 PR）：登录 → 商品 → 采购订单 → 销售订单 → 今日进价采集 → 价格策略
- **验证**：每页与老页面并排核对数据/金额/单位换算一致；Chrome 109 实测

---

## 清理收尾（阶段一完成后）

- 删除旧 src/、确认根目录只留方案同构文件 + 必要新增（food_accept_v3.db 等运行数据除外）
- docs/ 下 `Rust 全栈项目架构整合方案.md`、`前端技术栈对比与React迁移评估.md` 迁入 `.trae/documents/` 后删除 docs/

## 风险与回退

| 风险 | 对策 |
|---|---|
| 拆分编译错（crate:: 残留） | cargo check 全绿前不删旧 src/ |
| nightly 工具链漂移 | rust-toolchain.toml 锁版本 |
| React 迁移回归 | 每页独立 PR + /legacy/* 回退 + DB schema 全程不变 |
| 生产连续性 | 阶段一纯搬迁可整体 git revert；保留旧 exe 备份 |

分支策略：`main` 保持可发布；`refactor/workspace-split` 一次拆完合并（机械搬迁一次到位便于 diff 审查）；React 每页一支 `feature/react-page-<name>`。

## 执行顺序总览

```
阶段一
 1. 建 workspace 骨架 + 各 crate Cargo.toml + 同构空目录占位（.keep）
 2. 机械搬迁代码 + 批量引用替换 + 循环依赖处理
 3. cargo check 修复直至全绿
 4. win7 release 构建 + 冒烟验证
 5. 工程化文件（rustfmt/clippy/Makefile+ps1/CI/env.example/rust-toolchain）
 6. 清理：删旧 src/，docs/ → .trae/documents/

阶段二
 7. crates/frontend/ Vite+React+AntD 脚手架
 8. rust-embed 嵌入 + /app/* 挂载 + SPA fallback + /legacy/* 回退
 9. 登录页 + 主布局（菜单/权限）跑通全链路
 10. 核心页迁移（商品/采购/销售/采集/价格策略，逐页进行）
```
