---
name: domain-rule-sink
description: Sinks pure business rules from application free fns into domain Input impl blocks. Use when the user asks to sink pure rules, refactor domain association functions, or clean application layer fns. Do not use for fns that touch IO or HTTP contracts.
---

# Domain Rule Sink

把 application service 层的**纯业务规则 free fn**下沉到 domain 层对应 `*Input` 结构体的 `impl` 块里，让领域模型自己携带不变量。

## 触发条件

用户提到"纯业务规则下沉"、"domain 关联函数"、"规则下沉"、"把 application 里的函数搬到 domain"、"纯函数下沉"时触发。

## 前置依赖

- 已完成 Phase 0-4（严格六边形架构 + 护栏脚本就位）
- `domain-rule-sink` 是 hexagonal-layer-guard 的**补充行动**：护栏管"不能违规"，本 skill 管"怎么合规重构"

## 识别标准：哪些能下沉

**同时满足全部四条**才能下沉：

1. **零 IO** — 无 `async`/`await`/`.await`/`sqlx::`/`pool()`/repo 调用
2. **操作 domain 自有类型** — 入参/出参是 `*Input` / `*Row` 等 domain port 内定义的结构体，不是 `application::dto::*Req` 或 `serde_json::Value`
3. **纯业务规则** — 语义是"领域本身应该是什么样"（如"仓库汇总"、"数量兜底"、"字段校验"），不是用例编排或响应组装
4. **可被多用例复用** — 同一规则可能被 create/update/batch_import 等多条路径使用

## 识别标准：哪些不能下沉

| 函数类型 | 留在 application 的理由 |
|---|---|
| `can_access_*` 权限判定 | 绑定 `UserCtx` + 角色/租户隔离，是用例编排的一部分 |
| `list_row_json` / 响应组装 | 生成 `serde_json::Value`，是 HTTP 适配层职责 |
| `calc_selling_price` 展示逻辑 | 服务前端渲染卡片，不是领域不变量 |
| 带 `common::utils::is_valid_date_str` 的校验 | **可以下沉但要内联日期校验**——domain 不能引 common crate，要在 domain 内写一份逐字一致的 YYYY-MM-DD 校验 |
| Excel 导出/工作簿构建 | 绑定 `rust_xlsxwriter`，不是纯业务规则 |

## 下沉方式

**唯一正确姿势**：作为 `crates/domain/src/port/<mod>.rs` 内对应 `*Input` 结构体的 `impl` 块里的**关联函数**（associated function），不是新建 `domain/src/service/` 目录。理由：

1. 这些规则**紧绑特定 Input 结构体**（比如 `normalize` 只能操作 `SalesOrderItemInput`），关联函数内聚性最高
2. Rust 孤儿规则——本 crate 内类型，`impl XxxInput { ... }` 完全合法，不需要 `From` trait 映射
3. 不破坏 domain 零外部依赖约束（需要 common 的校验就 domain 内置一份）

### 示例模板

```rust
// crates/domain/src/port/purchase_order.rs

/// 采购单明细域内纯业务规则（零 IO，紧绑 PurchaseOrderItemInput）
impl PurchaseOrderItemInput {
    /// 按明细列表汇总主表仓库：各行全部同一仓库则记该仓库；否则仓库名去重后以"、"连接
    pub fn derive_main_warehouse(items: &[Self]) -> (i64, String) {
        // ... 纯计算逻辑
    }

    /// base_quantity 重算入参组装（product_unit.ratio 权威），不信任前端值
    pub fn base_inputs_of(items: &[Self]) -> Vec<(i64, String, f64, f64)> {
        // ...
    }
}
```

### application 侧调用点同步

```rust
// Before
let (main_wh_id, main_wh_name) = derive_main_warehouse(&req.items);
let base_qtys = self.repo.compute_base_quantities(&base_inputs_of(&req.items)).await;

// After
let (main_wh_id, main_wh_name) = PurchaseOrderItemInput::derive_main_warehouse(&req.items);
let base_qtys = self.repo.compute_base_quantities(&PurchaseOrderItemInput::base_inputs_of(&req.items)).await;
```

**同名替换，零字段访问改动，零行为变化。**

### 需要跨模块引用的情况

domain 端口文件之间可以互相引用：

```rust
// purchase_order.rs 里引用 price_schedule::PurchasePriceInput
impl PurchaseOrderItemInput {
    pub fn price_inputs_of(items: &[Self]) -> Vec<crate::port::price_schedule::PurchasePriceInput> {
        use crate::port::price_schedule::PurchasePriceInput;
        items.iter()
            .map(|it| PurchasePriceInput { ... })
            .collect()
    }
}
```

## 日期校验内置

`common::utils::is_valid_date_str` 逻辑三行，domain 内置一份避免依赖 common crate：

```rust
fn is_valid_date(s: &str) -> bool {
    if s.len() != 10 { return false; }
    let b = s.as_bytes();
    b[4] == b'-' && b[7] == b'-'
        && b[..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
}
```

## 执行步骤

1. **侦察**：grep `crates/application/src/service/*.rs` 里所有 `fn xxx(items: &[XxxInput])` 形式的函数，确认零 IO + 操作 domain 自有类型
2. **domain 加 impl**：在对应 `port/<mod>.rs` 的 Input 结构体定义后加 `impl` 块，函数体**逐字复制**，签名改关联函数
3. **application 删 free fn**：删掉 service 文件里的本地 free fn 与多余 import
4. **调用点替换**：全局替换 `fn_name(&xxx)` → `XxxInput::fn_name(&xxx)`
5. **四连验证**：
   - `cargo check --workspace` 零错误零警告
   - `scripts/check_layers.ps1` PASS=5 FAIL=0
   - `cargo build --release` → 复制 exe 到仓库根
   - 隔离目录 `scripts/smoke.ps1` PASS=232 FAIL=0
6. **提交**：Conventional Commits 混合模式，type=refactor，scope=六边形

## 已完成的下沉

| 提交 | 下沉内容 | 原位置 → 新位置 |
|---|---|---|
| `eb03389` | `derive_main_warehouse` / `base_inputs_of` / `price_inputs_of` / `normalize_quantities` / `validate` / `is_valid_price_type` | application/service/{purchase_order,sales_order,price_schedule}.rs → domain/port/{purchase_order,sales_order,price_schedule}.rs 的 impl 块 |

## 常见陷阱

| 陷阱 | 解法 |
|---|---|
| orphan rule 以为要 `From` 映射 | 不需要。domain port 文件内为 domain port 类型写 impl 是本 crate 内的自由操作 |
| 误引 `common::utils::*` | domain 不能依赖 common——日期校验这类简单逻辑在 domain 内联写一份 |
| 跨模块引用报循环依赖 | `purchase_order::PurchaseOrderItemInput::price_inputs_of` 要引用 `price_schedule::PurchasePriceInput`——这是 domain port 之间的单向引用，合法，且 domain 无 crate 循环（port 模块互不 mod include，只是类型引用） |
| `can_access_*` 函数也想下沉 | **不要**——它绑定 UserCtx（domain 有）但做的是权限编排，属于 application 层 |
