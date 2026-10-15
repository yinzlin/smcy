# 食材采购验收系统

一个面向食材配送业务的**采购 / 验收 / 对账一体化管理系统**。

服务端采用 **Rust（edition 2024）+ Axum + SQLite**，按**严格六边形架构**组织为 cargo workspace；前端为 **React 18 + AntD 5** 单页应用，构建产物经 rust-embed 编译期内嵌进后端二进制。最终交付物为**单 exe**（兼容 Windows 7/10/11，目标机器零依赖），内置系统托盘，双击即用，**完全离线可用**。

## 架构总览

```
crates/
├── common/        通用工具库（六边形之外，无业务依赖）
├── domain/        领域层：出站端口 trait + 输入/行结构体 + UserCtx + DomainError（仅依赖 serde）
├── application/   应用层：入站端口（UseCase trait）+ service 用例 + dto（HTTP 契约 *Req）
├── database/      出站适配器：domain 端口的 Sqlite 实现，持有唯一连接池
├── backend/       入站适配器 + 组合根：handlers、middleware、di.rs（唯一可触 pool 处）、托盘、静态资源
└── frontend/      React 前端（独立 npm 工程，exclude 出 workspace），产物内嵌至 /app
```

依赖方向：`backend → application → domain ← database`，`common` 被各层引用。5 条分层硬约束由 `scripts/check_layers.ps1` 自动站岗，详见 [.trae/skills/hexagonal-layer-guard](.trae/skills/hexagonal-layer-guard/SKILL.md) 与 [.trae/documents/六边形分层规则.md](.trae/documents/六边形分层规则.md)。

## 功能特性

### 基础资料管理
- 供应商、采购方、商品（含别名、规格、单位、进价/售价）、仓库管理
- 商品进价/售价自动维护策略：进价随采购单自动更新，售价按加成率自动计算
- 政采商品三态管理（非政采 / 有指导价 / 无指导价），验收单导出自动加 ★/☆ 标记
- 价格策略时段化：按商品 + 价格类型维护分时段价格，生效/失效日自动归并

### 订单管理
- **采购订单**：创建、编辑、删除、Excel 导入/导出、打印单据、采购单据图片上传
- **销售订单**：创建、编辑、删除、Excel 导入/导出、客户订单/签字验收单图片
- 订单状态机流转：待配单 → 分拣中 → 已分拣 → 配送中 → 已送达 → 已验收 → 已结算
- 由销售订单一键生成各供应商的采购订单（防重复生成保护，大单位采购价按单位换算规则计算）
- 配单分拣：支持按采购方、品类、供应商、综合排序等多种分拣视图，并可批量修正数量（含移动端全屏分拣页）

### 验收与报销
- 验收单 / 报销单 Excel 导出，支持真实口径与报销口径（合并分摊增项）
- 三联验收单导出（存根联 / 客户联 / 财务联，右侧竖排联次标注）
- 明细含零金额条目时禁止导出（强制拦截，可按需绕过）
- 导出文件使用订单日期，支持重复标题行与页脚页码

### 查询与分析
- 采购/销售订单查询、采购/销售汇总、商品排行、报销汇总、单据汇总
- **商品进价/售价同期折线走势图**（价格趋势）
- 供应商/采购方往来对账（应收应付）
- 库存查询：库存余额、流水、低库存预警、呆滞库存、进销存汇总
- 收支、毛利明细、品类进销存统计
- 今日进价采集（分供应商比价，采集价实时参与采购订单生成）

### 权限与安全（RBAC）
- 细粒度权限点体系：`purchase_order.*`、`sales_order.*`、`query.view`、`manage.*`
- 角色划分：超级管理员 / 管理员 / 供应商 / 采购方 / 普通用户
- **行级数据权限**：供应商账号只能查看/操作自己绑定的供应商单据；采购方账号同理
- **订单状态机约束**：仅待配单（pending）状态的订单允许修改/删除
- **操作审计日志**：所有关键写操作自动记录操作人、时间与详情
- 密码 bcrypt 加密存储，会话 Cookie 鉴权

### 系统管理
- 用户管理（可绑定供应商/采购方实现行级权限）
- 系统参数配置、数据库备份 / 恢复 / 完整性检查
- 异常数据自查与清理工具

## 技术栈

| 分类 | 技术 |
|------|------|
| 后端 | Rust edition 2024（nightly 工具链）、Axum 0.8、Tokio |
| 数据库 | SQLite（sqlx 0.9 异步驱动），单文件 `food_accept_v3.db` |
| Excel | rust_xlsxwriter（导出）、calamine（导入） |
| 前端 | React 18 + AntD 5 + TypeScript + Vite 5 + react-router 6 + zustand + dayjs，构建目标 chrome87（兼容 Win7 下 Chrome 109） |
| 前端交付 | rust-embed 编译期内嵌，挂载 `/app`，SPA 路由无扩展名回退 index.html |
| 桌面 | tray-icon + tao（系统托盘），工作目录锁定 exe 同级，端口 80 占用自动回退 8080~8099 |
| 兼容性 | 目标 `x86_64-win7-windows-msvc`（Tier 3，build-std 源码编译 std），`+crt-static` 静态链接 C 运行时 |

## 快速开始

### 环境要求
- Rust **nightly** 工具链（`rust-toolchain.toml` 已固定，`rustup override set nightly` 无需手动执行）
- Node.js + npm（仅构建前端时需要）

### 常用任务（make.ps1 / Makefile 一一对应）

```powershell
.\make.ps1 check           # cargo check --workspace
.\make.ps1 check-layers    # 六边形分层护栏（5 条硬约束）
.\make.ps1 build           # cargo build
.\make.ps1 release         # 前端构建 + cargo build --release
.\make.ps1 run             # release 构建并启动 exe
.\make.ps1 fmt             # cargo fmt --all
.\make.ps1 clippy          # cargo clippy --workspace
.\make.ps1 frontend-build  # 仅构建前端（npm install + vite build）
.\make.ps1 frontend-dev    # vite dev server（端口 5173，代理 /api → 本地 80）
```

### 运行

```powershell
.\make.ps1 run
```

启动后监听 80 端口（被占用时自动回退 8080~8099），托盘图标右键可打开页面。前端入口 `http://127.0.0.1/app`；旧版服务端渲染页面保留在 `/legacy` 路径下。

发布产物位于 `target/x86_64-win7-windows-msvc/release/food_accept_single.exe`，单文件即可分发——数据库（`food_accept_v3.db`）、`uploads/`、`backups/` 均在 exe 同级目录自动创建。

### 冒烟测试

```powershell
.\scripts\smoke.ps1
```

232 条行为断言，在隔离目录 `%TEMP%\smcy_smoke` 运行，是接口兼容性的行为基线；任何整改后必须 PASS=232 FAIL=0 才能提交。

## 种子凭据

全新部署（空目录首次启动）时自动初始化 4 个内置账号，首次登录后请及时修改密码：

| 账号 | 密码 | 角色 | 说明 |
|------|------|------|------|
| `super_admin` | `admin123` | 超级管理员 | 全量业务权限 + 用户/系统管理 |
| `admin` | `admin123` | 管理员 | 全量业务权限 |
| `supplier` | `supplier123` | 供应商 | 仅操作自己绑定的供应商单据 |
| `purchaser` | `purchaser123` | 采购方 | 仅操作自己绑定的采购单位单据 |

如需重置种子：关闭程序 → 删除 `food_accept_v3.db` → 重启即可重新播种。

## 目录结构

```
smcy/
├── Cargo.toml                  # workspace 根（resolver=3，5 个 crate，frontend 被 exclude）
├── rust-toolchain.toml         # 固定 nightly（build-std 需要）
├── .cargo/config.toml          # win7 目标 + build-std + crt-static
├── make.ps1 / Makefile         # 工程化命令入口
├── scripts/
│   ├── check_layers.ps1        # 六边形分层护栏（5 条硬约束）
│   └── smoke.ps1               # 冒烟测试（232 断言）
├── crates/
│   ├── common/src/utils/       # 通用工具
│   ├── domain/src/
│   │   ├── port/               # 出站端口 trait + 输入/行结构体（25 个业务域）
│   │   ├── error.rs            # DomainError（4 变体，无 HTTP 语义）
│   │   └── lib.rs              # UserCtx（会话上下文领域概念）
│   ├── application/src/
│   │   ├── port/               # 入站端口（UseCase trait）
│   │   ├── service/            # 用例实现（泛型单态化注入 repo）
│   │   ├── dto/                # HTTP 契约 *Req + From 映射
│   │   └── error.rs            # UseCaseError（7 变体，携带 HTTP 语义）
│   ├── database/src/sqlite/
│   │   ├── mod.rs              # 连接池、建表、数据修复
│   │   └── repo/               # 端口 trait 的 Sqlite 实现
│   ├── backend/src/
│   │   ├── main.rs             # 入口：路由、托盘、静态资源
│   │   ├── di.rs               # 组合根：唯一可触 pool() 处，装配 App
│   │   ├── api/                # handlers（HTTP 适配层）
│   │   ├── middleware/         # 鉴权、权限点、审计（经端口取数）
│   │   ├── pages/              # /legacy 旧版服务端渲染页面
│   │   └── app_assets.rs       # rust-embed 前端资源（gzip 压缩 + 缓存）
│   └── frontend/               # React 工程（npm，不参与 workspace）
│       └── src/{api,components,hooks,layouts,pages,stores,styles,utils}
├── .trae/
│   ├── documents/              # 架构方案与 ADR
│   ├── rules/                  # 提交规范等工作区规则
│   └── skills/                 # 项目固化的工程 Skill（含分层护栏）
├── food_accept_v3.db           # SQLite 主数据库（结构随版本入库）
├── uploads/                    # 上传图片（运行时生成，不入库）
└── backups/                    # 数据库备份目录
```

## 开发规范

- **新增业务模块**遵循标准 7 步路径：domain 端口 → database 实现 → application dto + From 映射 → service 入口 shadow → 入站端口 → handler 适配 → di 注入，详见 [.trae/skills/hexagonal-layer-guard](.trae/skills/hexagonal-layer-guard/SKILL.md)。
- **提交信息**遵循 Conventional Commits 混合模式（type 英文 + scope/描述中文），见 [.trae/rules/git-commit-message.md](.trae/rules/git-commit-message.md)。
- **接口兼容红线**：响应/请求 JSON 字段名与顺序不得改变；金额一律 `f64`，日期一律 `String('YYYY-MM-DD')`。
- 任何 Rust 代码变更后执行 `.\make.ps1 check-layers`，违规须立即修复。

## 相关文档

- [Rust 全栈项目架构整合方案.md](.trae/documents/Rust%20全栈项目架构整合方案.md)：本项目的完整脚手架方案
- [迁移指南-旧结构平滑过渡.md](.trae/documents/迁移指南-旧结构平滑过渡.md)：从单 crate 巨石 + SSR 迁移到本脚手架的分阶段路径
- [六边形分层规则.md](.trae/documents/六边形分层规则.md)：分层 ADR（长期有效）
- [导出验收单.md](.trae/documents/导出验收单.md)：验收单 / 报销单导出口径说明
- [耗材分摊方案.md](.trae/documents/耗材分摊方案.md)：耗材分摊增项处理方案
- [销售订单生成采购订单.md](.trae/documents/销售订单生成采购订单.md)：自动生成采购单的业务规则

## 许可证

内部使用项目，未开源。
