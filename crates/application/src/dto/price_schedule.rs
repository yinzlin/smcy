//! 价格策略时段入站契约 + 到 domain 端口输入类型的映射

use serde::{Deserialize, Serialize};

use domain::port::price_schedule::PriceScheduleInput;

/// 价格策略时段：政采价/超市比价按生效日期段录入与使用
/// 同一 (product_id, price_type) 下，按 effective_date 升序，
/// 任意查询日期落在 [effective_date, next_effective_date) 区间内即匹配该价格。
#[derive(Deserialize, Serialize)]
pub struct PriceScheduleReq {
    pub id: Option<i64>,
    pub product_id: i64,
    /// gov_procurement / supermarket_1 / supermarket_2 / supermarket_3 / ai_realtime
    /// / supplier_contract（供应商约定采购价）/ purchaser_contract（采购单位约定售价）
    pub price_type: String,
    pub price: f64,
    /// 生效日期 'YYYY-MM-DD'，含；该价格自此日期开始生效
    pub effective_date: String,
    /// 失效日期 'YYYY-MM-DD'，含；NULL 表示至今有效
    /// 创建时一般传 NULL，由系统在新增/删除时自动维护该字段（见 api_price_schedule_*）
    pub end_date: Option<String>,
    pub source: Option<String>,
    pub remark: Option<String>,
    /// supplier_contract 时必填：约定采购价的供应商
    #[serde(default)]
    pub supplier_id: Option<i64>,
    /// purchaser_contract 时必填：约定售价的采购单位
    #[serde(default)]
    pub purchaser_id: Option<i64>,
}

/// 批量导入价格策略：用于 Excel 批量录入政采价/超市比价
#[derive(Deserialize, Serialize)]
pub struct PriceScheduleBatchReq {
    pub items: Vec<PriceScheduleReq>,
    /// 冲突策略：upsert=按 (product, price_type, effective_date) 覆盖；ignore=跳过已存在
    pub conflict_policy: Option<String>,
}

impl From<PriceScheduleReq> for PriceScheduleInput {
    fn from(r: PriceScheduleReq) -> Self {
        Self {
            id: r.id,
            product_id: r.product_id,
            price_type: r.price_type,
            price: r.price,
            effective_date: r.effective_date,
            end_date: r.end_date,
            source: r.source,
            remark: r.remark,
            supplier_id: r.supplier_id,
            purchaser_id: r.purchaser_id,
        }
    }
}
