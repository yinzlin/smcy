//! 库存查询用例：库存余额/收发存、库存流水、出入库统计、库存预警、呆滞库存（含导出）
//!
//! 行为保持：全部查询失败 unwrap_or_default()/unwrap_or(0) 吞错；workbook 构建 .unwrap() 原样保留；
//! 日期非法（400）由 HTTP 薄适配层前置拦截，本层收到的参数已通过校验。
//! 出入库统计复用 application::service::stock 的 compute_stock_summary / compute_stock_summary_reimburse。

use std::collections::HashMap;

use domain::port::query_stock::{QueryStockRepo, StockFlowRow};
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};

use crate::error::UseCaseError;
use crate::port::query_stock::QueryStockUseCase;
use crate::service::stock::{compute_stock_summary, compute_stock_summary_reimburse};

#[derive(Clone)]
pub struct QueryStockService<R: QueryStockRepo> {
    repo: R,
}

impl<R: QueryStockRepo> QueryStockService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

/// 台账余额累计：rows 必须已按时间线（create_time, src, sid）排序。
/// 基准日期取 start_date（为空则取首行日期），先查各商品在基准日期之前的流水净额作期初，
/// 再逐行按 product_id 分别累计结存（同一行序下各商品独立余额，与手工分户账一致）。
/// 返回与 rows 等长、按行序对应的余额数组。
async fn compute_running_balances<R: QueryStockRepo>(rows: &[StockFlowRow], start_date: &str, repo: &R) -> Vec<f64> {
    let Some(first) = rows.first() else { return Vec::new(); };
    let first_date = first.create_time.clone();
    let base_date = if start_date.is_empty() { first_date.as_str() } else { start_date };

    let mut pids: Vec<i64> = rows.iter().map(|r| r.product_id).collect();
    pids.sort_unstable();
    pids.dedup();
    let pid_list = pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
    // 期初库存必须与 stock_flow_base_sql 同一聚合口径（按订单分组抵消反审核），
    // 否则反审核冲销记录会被重复计入，导致期初余额错误。
    // 行为保持：期初查询失败 unwrap_or_default 返回空
    let opening_rows = repo.list_stock_flow_opening(&pid_list, base_date).await.unwrap_or_default();
    let mut acc: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
    for r in &opening_rows {
        acc.insert(r.product_id, r.net);
    }
    rows.iter().map(|row| {
        let pid = row.product_id;
        let net = row.in_quantity - row.out_quantity;
        let b = acc.get(&pid).copied().unwrap_or(0.0) + net;
        acc.insert(pid, b);
        b
    }).collect()
}

impl<R: QueryStockRepo> QueryStockUseCase for QueryStockService<R> {
    async fn stock_balance(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let product_name = params.get("product_name").map(|s| s.as_str()).unwrap_or("");
        let category_id = params.get("category_id").map(|s| s.as_str()).unwrap_or("");
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1).max(1);
        let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * page_size;

        let history = !start_date.is_empty() || !end_date.is_empty();
        let pattern = format!("%{}%", product_name);
        // 时间归属按单据日期（order_date 快照）：纯日期比较。
        // 缺省下界 1970（期初=0），缺省上界 2999（期末=当前实时余额）
        let start_bound = if start_date.is_empty() { "1970-01-01".to_string() } else { start_date.to_string() };
        let end_bound = if end_date.is_empty() { "2999-12-31".to_string() } else { end_date.to_string() };

        // 行为保持：count 查询失败 unwrap_or(0)，数据查询失败 unwrap_or_default 返回空
        let total = if history {
            self.repo.count_stock_balance_history(&start_bound, &end_bound, &pattern, category_id).await.unwrap_or(0)
        } else {
            self.repo.count_stock_balance_realtime(&pattern, category_id).await.unwrap_or(0)
        };

        let items: Vec<serde_json::Value> = if history {
            let rows = self.repo.list_stock_balance_history(&start_bound, &end_bound, &pattern, category_id, page_size, offset).await.unwrap_or_default();
            rows.iter()
                .map(|row| {
                    serde_json::json!({
                        "product_id": row.product_id,
                        "product_name": row.product_name,
                        "spec": row.spec,
                        "unit": row.unit,
                        "opening_qty": row.opening_qty,
                        "period_in": row.period_in,
                        "period_out": row.period_out,
                        "closing_qty": row.closing_qty,
                        "amount": row.closing_qty * row.base_price,
                    })
                })
                .collect()
        } else {
            let rows = self.repo.list_stock_balance_realtime(&pattern, category_id, page_size, offset).await.unwrap_or_default();
            rows.iter()
                .map(|row| {
                    serde_json::json!({
                        "product_id": row.product_id,
                        "product_name": row.product_name,
                        "spec": row.spec,
                        "unit": row.unit,
                        "quantity": row.quantity,
                        "amount": row.amount,
                        "min_stock": row.min_stock,
                        "max_stock": row.max_stock,
                    })
                })
                .collect()
        };

        let result = serde_json::json!({
            "items": items,
            "total": total,
            "page": page,
            "page_size": page_size,
            "mode": if history { "history" } else { "realtime" },
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_stock_balance(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let product_name = params.get("product_name").map(|s| s.as_str()).unwrap_or("");
        let category_id = params.get("category_id").map(|s| s.as_str()).unwrap_or("");
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let history = !start_date.is_empty() || !end_date.is_empty();
        let pattern = format!("%{}%", product_name);
        let start_bound = if start_date.is_empty() { "1970-01-01".to_string() } else { start_date.to_string() };
        let end_bound = if end_date.is_empty() { "2999-12-31".to_string() } else { end_date.to_string() };

        let mut workbook = Workbook::new();
        let ws = workbook.add_worksheet();
        let hf = common::utils::xlsx_header_format(0x2E75B6);
        let (sheet_name, headers, widths): (&str, &[&str], &[u32]) = if history {
            (
                "收发存",
                &["商品名称", "规格", "单位", "期初数量", "本期入库", "本期出库", "期末数量", "期末金额"],
                &[20, 14, 10, 12, 12, 12, 12, 14],
            )
        } else {
            (
                "库存余额",
                &["商品名称", "规格", "单位", "库存数量", "库存金额", "最低库存", "最高库存"],
                &[20, 14, 10, 14, 14, 14, 14],
            )
        };
        ws.set_name(sheet_name).unwrap();
        for (c, h) in headers.iter().enumerate() {
            ws.write_with_format(0, c as u16, *h, &hf).unwrap();
            ws.set_column_width(c as u16, widths[c]).unwrap();
        }
        // 行为保持：查询失败 unwrap_or_default 返回空
        if history {
            let rows = self.repo.list_stock_balance_history_export(&start_bound, &end_bound, &pattern, category_id).await.unwrap_or_default();
            for (i, row) in rows.iter().enumerate() {
                let r = (i + 1) as u32;
                ws.write(r, 0, row.product_name.as_str()).unwrap();
                ws.write(r, 1, row.spec.clone().unwrap_or_default()).unwrap();
                ws.write(r, 2, row.unit.clone().unwrap_or_default()).unwrap();
                ws.write(r, 3, row.opening_qty).unwrap();
                ws.write(r, 4, row.period_in).unwrap();
                ws.write(r, 5, row.period_out).unwrap();
                ws.write(r, 6, row.closing_qty).unwrap();
                ws.write(r, 7, row.closing_qty * row.base_price).unwrap();
            }
        } else {
            let rows = self.repo.list_stock_balance_realtime_export(&pattern, category_id).await.unwrap_or_default();
            for (i, row) in rows.iter().enumerate() {
                let r = (i + 1) as u32;
                ws.write(r, 0, row.product_name.as_str()).unwrap();
                ws.write(r, 1, row.spec.clone().unwrap_or_default()).unwrap();
                ws.write(r, 2, row.unit.clone().unwrap_or_default()).unwrap();
                ws.write(r, 3, row.quantity).unwrap();
                ws.write(r, 4, row.amount).unwrap();
                ws.write(r, 5, row.min_stock).unwrap();
                ws.write(r, 6, row.max_stock).unwrap();
            }
        }
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn stock_flow(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let product_name = params.get("product_name").map(|s| s.as_str()).unwrap_or("");
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let product_id = params.get("product_id").and_then(|s| s.parse::<i64>().ok());
        let group_view = params.get("view").map(|s| s.as_str()) == Some("group");
        let page: i64 = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1).max(1);
        let page_size: i64 = params.get("page_size").and_then(|s| s.parse().ok()).unwrap_or(200).clamp(1, 1000);
        let offset = (page - 1) * page_size;

        let pattern = format!("%{}%", product_name);

        // 直接从 stock_movement 读取：单一 where 子句即可（不再分别拼采购/销售）
        // 时间归属按单据日期（order_date 为 'YYYY-MM-DD' 纯日期，直接比较）
        let mut where_clause = String::from("WHERE 1=1");
        if let Some(pid) = product_id {
            where_clause.push_str(&format!(" AND sm.product_id = {}", pid));
        } else if !product_name.is_empty() {
            where_clause.push_str(" AND p.name LIKE ?");
        }
        if !start_date.is_empty() {
            where_clause.push_str(&format!(" AND sm.order_date >= '{}'", start_date));
        }
        if !end_date.is_empty() {
            where_clause.push_str(&format!(" AND sm.order_date <= '{}'", end_date));
        }

        let needs_bind = product_id.is_none() && !product_name.is_empty();

        // 行为保持：count 查询失败 unwrap_or(0)
        let total = self.repo.count_stock_flow(&where_clause, needs_bind, &pattern).await.unwrap_or(0);

        // 分页查询：余额需从第 0 行开始累计才正确，故一次取到当前页末尾（LIMIT offset+page_size），
        // 应用层累计后再切出当前页；深度分页成本随页码线性增长，可接受
        let fetch_limit = offset + page_size;
        // 时间线视图：单据日期 → 先入后出 → 流水号；余额按各商品自身逐笔结存。
        // 分户视图：先按商品聚拢，商品内仍按时间线——余额按 product_id 独立累计，行序变化不影响余额正确性。
        let order_by = if group_view {
            "product_id, create_time, src, sid"
        } else {
            "create_time, src, sid"
        };
        // 行为保持：数据查询失败 unwrap_or_default 返回空
        let rows = self.repo.list_stock_flow(&where_clause, order_by, fetch_limit, needs_bind, &pattern).await.unwrap_or_default();

        // 期初净额：涉及商品在基准日期（start_date，无则为首行日期）之前的聚合净额作期初，
        // 再按时间线顺序逐行累计（按 product_id 分别结存），保证余额与显示顺序自洽
        let balances = compute_running_balances(&rows, start_date, &self.repo).await;

        let skip = offset as usize;
        let items: Vec<serde_json::Value> = rows.iter().enumerate().skip(skip).map(|(i, row)| {
            serde_json::json!({
                "create_time": row.create_time,
                "movement_type": row.movement_type,
                "summary": row.summary,
                "direction": row.direction,
                "product_id": row.product_id,
                "product_name": row.product_name,
                "spec": row.spec,
                "unit": row.unit,
                "in_quantity": row.in_quantity,
                "out_quantity": row.out_quantity,
                "ref_no": row.ref_no,
                "party_name": row.party_name,
                "production_date": row.production_date,
                "batch_no": row.batch_no,
                "balance": balances.get(i).copied().unwrap_or(0.0),
                "orig_quantity": row.orig_quantity,
                "orig_unit": row.orig_unit,
            })
        }).collect();

        let result = serde_json::json!({
            "total": total,
            "page": page,
            "page_size": page_size,
            "items": items,
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_stock_flow(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let product_name=params.get("product_name").map(|s|s.as_str()).unwrap_or("");let start_date=params.get("start_date").map(|s|s.as_str()).unwrap_or("");let end_date=params.get("end_date").map(|s|s.as_str()).unwrap_or("");let product_id=params.get("product_id").and_then(|s|s.parse::<i64>().ok());
        let fmt=params.get("format").map(|s|s.as_str()).unwrap_or("ledger");
        let pattern=format!("%{}%",product_name);let mut wc=String::from("WHERE 1=1");
        if let Some(pid)=product_id{wc.push_str(&format!(" AND sm.product_id={}",pid));}else if!product_name.is_empty(){wc.push_str(" AND p.name LIKE ?");}
        if!start_date.is_empty(){wc.push_str(&format!(" AND sm.order_date>='{}'",start_date));}if!end_date.is_empty(){wc.push_str(&format!(" AND sm.order_date<='{}'",end_date));}
        let needs_bind = product_id.is_none() && !product_name.is_empty();

        // 时间线补打：全部商品按单据日期连续紧凑排列，不按户分页，只输出新期间薄薄几页续接旧账本。
        // 每行结存仍是该商品自身余额（期初 = 开始日期前全历史净额），第一行即承接前期账。
        if fmt == "timeline" {
            // 行为保持：查询失败 unwrap_or_default 返回空
            let rows = self.repo.list_stock_flow_export_timeline(&wc, needs_bind, &pattern).await.unwrap_or_default();
            if rows.is_empty(){
                return Ok(Workbook::new().save_to_buffer().unwrap_or_default());
            }
            let balances=compute_running_balances(&rows,start_date,&self.repo).await;
            let mut workbook=Workbook::new();let ws=workbook.add_worksheet();ws.set_name("出入库记录").unwrap();
            let hf = Format::new().set_bold()
                .set_background_color(Color::RGB(0xD9D9D9)).set_font_color(Color::Black)
                .set_align(FormatAlign::Center).set_align(FormatAlign::VerticalCenter)
                .set_text_wrap()
                .set_border(FormatBorder::Thin);
            ws.set_landscape(); ws.set_paper_size(9);
            let margin_cm = 2.0f64/2.54;
            ws.set_margins(margin_cm, margin_cm, 0.0, 0.0, 0.0, 0.0);
            ws.set_print_center_horizontally(true);
            ws.set_footer("&C第&[Page]页");
            ws.set_repeat_rows(0, 2).unwrap();
            let grid_center = Format::new().set_border(FormatBorder::Thin).set_align(FormatAlign::Center);
            let grid_left = Format::new().set_border(FormatBorder::Thin);
            let grid_shrink_left = Format::new().set_border(FormatBorder::Thin).set_shrink();
            let grid_shrink_center = Format::new().set_border(FormatBorder::Thin).set_align(FormatAlign::Center).set_shrink();
            let red_center = Format::new().set_font_color(Color::RGB(0xC00000)).set_border(FormatBorder::Thin).set_align(FormatAlign::Center);
            let title_fmt = Format::new().set_bold().set_font_size(20.0).set_align(FormatAlign::Center);
            // 标题含补打期间，便于与前期账本区分、装订排序
            let period = match (start_date.is_empty(), end_date.is_empty()) {
                (false,false)=>format!("（{} 至 {} 补打）",start_date,end_date),
                (false,true)=>format!("（{} 起补打）",start_date),
                (true,false)=>format!("（截至 {}）",end_date),
                _=>String::new(),
            };
            let title = format!("出 入 库 流 水 账{}",period);
            ws.merge_range(0,0,0,9,title.as_str(),&title_fmt).unwrap();
            // 类型列只标方向（入库/出库），数量合并单列——出库数量红色，装订翻阅时一眼可辨；
            // 品名规格取商品名称本身（名称已含规格，如"元宝大豆油20L"），不再单列规格
            let headers_arr=["日期","类型","单据号","品名规格","单位","数量","结存","来源/去向","生产日期\n/批号","备注"];
            for(c,h)in headers_arr.iter().enumerate(){ws.write_with_format(2,c as u16,*h,&hf).unwrap();}
            // 日期/类型/单据号/生产日期批号 4 列按内容自适应宽度（中文按 2 宽计，初值含列头）；
            // 品名规格/单位/数量/结存/来源去向/备注 6 列固定宽度
            for(c,w)in [(3u16,26u32),(4,5),(5,9),(6,9),(7,16),(9,12)].iter(){ws.set_column_width(*c,*w).unwrap();}
            let disp_w=|s:&str|->f64{s.chars().map(|c|if(c as u32)>=0x2E80{2.0}else{1.0}).sum()};
            let mut aw=[11.0f64,5.0,8.0,8.0];
            let fmt_qty = |q: f64| -> String { if q == 0.0 {String::new()} else {let r=(q*100.0).round()/100.0; format!("{}",r)} };
            for(i,row)in rows.iter().enumerate(){
                let r=(i+3)as u32;
                let inq=row.in_quantity;
                let outq=row.out_quantity;
                let is_out = outq != 0.0;
                let qty_s = fmt_qty(if is_out {outq} else {inq});
                let bal=balances.get(i).copied().unwrap_or(0.0);
                let unit=row.unit.clone().unwrap_or_default();
                let orig_q=row.orig_quantity;
                let orig_u=row.orig_unit.clone().unwrap_or_default();
                let remark=if!orig_u.is_empty()&&orig_u!=unit&&orig_q!=0.0{format!("原{}{}",fmt_qty(orig_q),orig_u)}else{String::new()};
                let batch_s=match(row.production_date.as_deref().unwrap_or(""),row.batch_no.as_deref().unwrap_or("")){
                    ("","")=>String::new(),(p,"")=>p.to_string(),("",b)=>b.to_string(),(p,b)=>format!("{} / {}",p,b)
                };
                let ref_s=row.ref_no.clone().unwrap_or_default();
                aw[0]=aw[0].max(disp_w(row.create_time.as_str()));
                aw[2]=aw[2].max(disp_w(ref_s.as_str()));
                aw[3]=aw[3].max(disp_w(batch_s.as_str()));
                ws.write_with_format(r,0,row.create_time.as_str(),&grid_center).unwrap();
                ws.write_with_format(r,1,if is_out {"出库"} else {"入库"},&grid_center).unwrap();
                ws.write_with_format(r,2,ref_s.as_str(),&grid_shrink_center).unwrap();
                ws.write_with_format(r,3,row.product_name.as_str(),&grid_shrink_left).unwrap();
                ws.write_with_format(r,4,unit.as_str(),&grid_center).unwrap();
                ws.write_with_format(r,5,qty_s.as_str(),if is_out {&red_center} else {&grid_center}).unwrap();
                ws.write_with_format(r,6,bal,if bal<0.0{&red_center}else{&grid_center}).unwrap();
                ws.write_with_format(r,7,row.party_name.clone().unwrap_or_default(),&grid_shrink_left).unwrap();
                ws.write_with_format(r,8,batch_s.as_str(),&grid_shrink_center).unwrap();
                ws.write_with_format(r,9,remark.as_str(),&grid_left).unwrap();
            }
            // 紧凑连续分页：每满 25 条一页，数据不足不补空行（补打页不浪费纸张）
            let total=rows.len();
            let breaks:Vec<u32>=(1..).map(|k|k*25).take_while(|&c|c<total).map(|c|c as u32+3).collect();
            if!breaks.is_empty(){ws.set_page_breaks(&breaks).unwrap();}
            // 4 列自适应宽度应用（+1 边距；列宽是列级元数据，数据写完后设置同样生效）
            ws.set_column_width(0,aw[0]+1.0).unwrap();
            ws.set_column_width(1,aw[1]+1.0).unwrap();
            ws.set_column_width(2,aw[2]+1.0).unwrap();
            ws.set_column_width(8,aw[3]+1.0).unwrap();
            let last_row=(total+2)as u32;
            // 行高：标题/间隔/数据行固定 20（标题 30）；列头行（r=2）不设固定行高，由 Excel 按换行内容自动撑高
            for r in 1..=last_row{ if r!=2 { ws.set_row_height(r,20.0).unwrap(); } }
            ws.set_row_height(0,30.0).unwrap();
            return Ok(workbook.save_to_buffer().unwrap());
        }

        // 一品一户数量保管账：服务端按商品聚拢（户内时间线），余额按 product_id 独立累计
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self.repo.list_stock_flow_export_ledger(&wc, needs_bind, &pattern).await.unwrap_or_default();
        if rows.is_empty(){
            return Ok(Workbook::new().save_to_buffer().unwrap_or_default());
        }

        // 期初：各商品在基准日期之前的聚合净额（与 compute_running_balances 同口径）
        let mut pids: Vec<i64> = rows.iter().map(|r| r.product_id).collect();
        pids.sort_unstable(); pids.dedup();
        let pid_list = pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
        let first_date = rows.iter().map(|r| r.create_time.clone()).min().unwrap_or_default();
        let base_date = if start_date.is_empty() { first_date.as_str() } else { start_date };
        // 行为保持：期初查询失败 unwrap_or_default 返回空
        let opening_rows = self.repo.list_stock_flow_opening(&pid_list, base_date).await.unwrap_or_default();
        let mut opening: std::collections::HashMap<i64,f64> = std::collections::HashMap::new();
        for r in &opening_rows { opening.insert(r.product_id, r.net); }
        // 行为保持：主仓名查询失败兜底"主仓"
        let warehouse_name = self.repo.main_warehouse_name().await.unwrap_or_else(|_| "主仓".to_string());

        let mut workbook=Workbook::new();let ws=workbook.add_worksheet();ws.set_name("出入库记录").unwrap();
        // 打印布局与采购台账一致：A4 横版、浅灰列头黑字、行高 20、25 行/页、页脚页码
        let hf = Format::new().set_bold()
            .set_background_color(Color::RGB(0xD9D9D9)).set_font_color(Color::Black)
            .set_align(FormatAlign::Center).set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin);
        ws.set_landscape(); ws.set_paper_size(9);
        let margin_cm = 2.0f64/2.54;
        ws.set_margins(margin_cm, margin_cm, 0.0, 0.0, 0.0, 0.0);
        ws.set_print_center_horizontally(true);
        ws.set_footer("&C第&[Page]页");
        ws.set_repeat_rows(0, 2).unwrap();
        let grid_center = Format::new().set_border(FormatBorder::Thin).set_align(FormatAlign::Center);
        let grid_left = Format::new().set_border(FormatBorder::Thin);
        let grid_shrink_left = Format::new().set_border(FormatBorder::Thin).set_shrink();
        let grid_shrink_center = Format::new().set_border(FormatBorder::Thin).set_align(FormatAlign::Center).set_shrink();
        let bold_center = Format::new().set_bold().set_border(FormatBorder::Thin).set_align(FormatAlign::Center);
        let bold_left = Format::new().set_bold().set_border(FormatBorder::Thin);
        let head_fmt = Format::new().set_bold().set_background_color(Color::RGB(0xF2F2F2))
            .set_border(FormatBorder::Thin).set_align(FormatAlign::VerticalCenter);
        let title_fmt = Format::new().set_bold().set_font_size(20.0).set_align(FormatAlign::Center);
        ws.merge_range(0,0,0,8,"出 入 库 记 录（数量保管账）",&title_fmt).unwrap();
        let headers_arr = ["日期","单据号","摘要","来源/去向","入库数量","出库数量","结存数量","生产日期/批号","备注"];
        for (c,h) in headers_arr.iter().enumerate() { ws.write_with_format(2,c as u16,*h,&hf).unwrap(); }
        let widths=[12u32,17,10,18,10,10,10,20,16];
        for (c,w) in widths.iter().enumerate() { ws.set_column_width(c as u16,*w).unwrap(); }

        // 数量为 0 留白；结存允许负数（红字提示账实异常）
        let red_bold = Format::new().set_bold().set_font_color(Color::RGB(0xC00000)).set_border(FormatBorder::Thin).set_align(FormatAlign::Center);
        let fmt_qty = |q: f64| -> String { if q == 0.0 { String::new() } else { let r=(q*100.0).round()/100.0; format!("{}",r) } };
        let batch_text = |prod: &Option<String>, batch: &Option<String>| -> String {
            match (prod.as_deref().unwrap_or(""), batch.as_deref().unwrap_or("")) {
                ("","")=>String::new(), (p,"")=>p.to_string(), ("",b)=>b.to_string(), (p,b)=>format!("{} / {}",p,b)
            }
        };

        const PER_PAGE: usize = 25;
        let mut content_k: usize = 0;            // 内容区行计数（0 基，对应工作表行 content_k+3）
        let mut breaks: Vec<u32> = Vec::new();
        let mut idx = 0usize;
        while idx < rows.len() {
            let pid = rows[idx].product_id;
            let name = rows[idx].product_name.clone();
            let spec = rows[idx].spec.clone().unwrap_or_default();
            let unit = rows[idx].unit.clone().unwrap_or_default();
            let mut j = idx;
            while j < rows.len() && rows[j].product_id == pid { j += 1; }
            let block = &rows[idx..j];

            // 补空行对齐到页边界，保证每个商品一户一页起打
            let used = 2 + block.len() + 1;      // 账头 + 期初 + 流水 + 本期合计
            let fill = (PER_PAGE - (used % PER_PAGE)) % PER_PAGE;
            if content_k % PER_PAGE != 0 {
                // 理论上每块结束都补齐，这里防御性处理（首块 k=0 不补）
                let gap = PER_PAGE - content_k % PER_PAGE;
                for _ in 0..gap {
                    let r = (content_k + 3) as u32;
                    for c in 0..9u16 { ws.write_with_format(r,c,"",&grid_left).unwrap(); }
                    content_k += 1;
                }
            }

            // 账头行：品名规格 / 计量单位 / 仓库
            let head_text = format!("品名规格：{} {}　　计量单位：{}　　仓库：{}", name, spec, unit, warehouse_name);
            let r = (content_k + 3) as u32;
            ws.merge_range(r,0,r,8,head_text.as_str(),&head_fmt).unwrap();
            content_k += 1;

            // 期初行
            let mut bal = opening.get(&pid).copied().unwrap_or(0.0);
            let r = (content_k + 3) as u32;
            ws.write_with_format(r,0,"期初",&grid_center).unwrap();
            ws.write_with_format(r,1,"",&grid_center).unwrap();
            ws.write_with_format(r,2,"上期结转",&grid_center).unwrap();
            for c in 3..6u16 { ws.write_with_format(r,c,"",&grid_left).unwrap(); }
            ws.write_with_format(r,6,bal,if bal<0.0 {&red_bold}else{&bold_center}).unwrap();
            ws.write_with_format(r,7,"",&grid_center).unwrap();
            ws.write_with_format(r,8,"",&grid_left).unwrap();
            content_k += 1;

            // 流水行
            let mut sum_in = 0.0; let mut sum_out = 0.0;
            for row in block {
                let inq = row.in_quantity;
                let outq = row.out_quantity;
                sum_in += inq; sum_out += outq; bal += inq - outq;
                let r = (content_k + 3) as u32;
                ws.write_with_format(r,0,row.create_time.as_str(),&grid_center).unwrap();
                ws.write_with_format(r,1,row.ref_no.clone().unwrap_or_default(),&grid_shrink_center).unwrap();
                ws.write_with_format(r,2,row.summary.as_str(),&grid_center).unwrap();
                ws.write_with_format(r,3,row.party_name.clone().unwrap_or_default(),&grid_shrink_left).unwrap();
                ws.write_with_format(r,4,fmt_qty(inq),&grid_left).unwrap();
                ws.write_with_format(r,5,fmt_qty(outq),&grid_left).unwrap();
                ws.write_with_format(r,6,bal,if bal<0.0 {&red_bold}else{&grid_center}).unwrap();
                ws.write_with_format(r,7,batch_text(&row.production_date,&row.batch_no),&grid_shrink_center).unwrap();
                // 备注：原始大单位与基础单位不同时显示「原X大单位」
                let orig_q = row.orig_quantity;
                let orig_u = row.orig_unit.clone().unwrap_or_default();
                let remark = if !orig_u.is_empty() && orig_u != unit && orig_q != 0.0 {
                    format!("原{}{}", fmt_qty(orig_q), orig_u)
                } else { String::new() };
                ws.write_with_format(r,8,remark,&grid_left).unwrap();
                content_k += 1;
            }

            // 本期合计行
            let r = (content_k + 3) as u32;
            ws.write_with_format(r,0,"",&grid_center).unwrap();
            ws.write_with_format(r,1,"",&grid_center).unwrap();
            ws.write_with_format(r,2,"本期合计",&bold_center).unwrap();
            ws.write_with_format(r,3,"",&grid_left).unwrap();
            ws.write_with_format(r,4,fmt_qty(sum_in),&bold_left).unwrap();
            ws.write_with_format(r,5,fmt_qty(sum_out),&bold_left).unwrap();
            ws.write_with_format(r,6,bal,if bal<0.0 {&red_bold}else{&bold_center}).unwrap();
            ws.write_with_format(r,7,"",&grid_center).unwrap();
            ws.write_with_format(r,8,"",&grid_left).unwrap();
            content_k += 1;

            // 补带框空行至本页满 25 行
            for _ in 0..fill {
                let r = (content_k + 3) as u32;
                for c in 0..9u16 { ws.write_with_format(r,c,"",&grid_left).unwrap(); }
                content_k += 1;
            }
            // 下一户另起一页（最后一户不分页）
            if j < rows.len() {
                breaks.push((content_k + 3) as u32);
            }
            idx = j;
        }
        if !breaks.is_empty() { ws.set_page_breaks(&breaks).unwrap(); }
        // 行高统一 20（大标题行 30）
        let last_row = (content_k + 2) as u32;
        for r in 0..=last_row { ws.set_row_height(r,20.0).unwrap(); }
        ws.set_row_height(0,30.0).unwrap();

        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn stock_summary(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let (rows, total_in, total_out, total_discounted_out) = compute_stock_summary(start_date, end_date).await;

        // 收集所有仓库列表（保持原有兼容性，虽然导出不再需要）
        use std::collections::BTreeMap;
        let mut warehouse_names: BTreeMap<i64, String> = BTreeMap::new();
        for r in &rows {
            if r.warehouse_id >= 0 {
                warehouse_names.insert(r.warehouse_id, r.warehouse_name.clone());
            }
        }
        let warehouse_list: Vec<serde_json::Value> = warehouse_names
            .iter()
            .map(|(id, name)| serde_json::json!({"id": id, "name": name}))
            .collect();

        let items: Vec<serde_json::Value> = rows.iter().map(|r| {
            serde_json::json!({
                "day": r.day,
                "warehouse_id": r.warehouse_id,
                "warehouse_name": r.warehouse_name,
                "is_summary": r.is_summary,
                "in_amount": r.in_amount,
                "in_item_count": r.in_item_count,
                "in_order_count": r.in_order_count,
                "out_amount": r.out_amount,
                "out_item_count": r.out_item_count,
                "out_order_count": r.out_order_count,
                "discounted_out_amount": r.discounted_out_amount,
                "gross_profit": r.gross_profit,
            })
        }).collect();

        let result = serde_json::json!({
            "rows": items,
            "warehouses": warehouse_list,
            "total_in_amount": total_in,
            "total_out_amount": total_out,
            "total_discounted_out_amount": total_discounted_out,
            "total_gross_profit": total_discounted_out - total_in,
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_stock_summary(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let (rows, total_in, total_out, total_discounted_out) = compute_stock_summary(start_date, end_date).await;

        let mut workbook = rust_xlsxwriter::Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("出入库统计").unwrap();

        let header_format = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0x2E75B6))
            .set_font_color(rust_xlsxwriter::Color::White)
            .set_align(rust_xlsxwriter::FormatAlign::Center)
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        let summary_format = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0xFFF2CC))
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        let num_format = rust_xlsxwriter::Format::new()
            .set_num_format("#,##0.00")
            .set_align(rust_xlsxwriter::FormatAlign::Right);

        let num_format_sum = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_num_format("#,##0.00")
            .set_background_color(rust_xlsxwriter::Color::RGB(0xFFF2CC))
            .set_align(rust_xlsxwriter::FormatAlign::Right);

        // 列顺序：日期、仓库、入库金额、入库单数、入库条数、出库金额、下浮后出库金额、出库单数、出库条数、毛利
        let headers = ["日期", "仓库", "入库金额", "入库单数", "入库条数", "出库金额", "下浮后出库金额", "出库单数", "出库条数", "毛利"];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_with_format(0, col as u16, *header, &header_format).unwrap();
        }

        // 设置列宽
        worksheet.set_column_width(0, 14).unwrap(); // 日期
        worksheet.set_column_width(1, 16).unwrap(); // 仓库
        worksheet.set_column_width_pixels(2, 90).unwrap();  // 入库金额
        worksheet.set_column_width_pixels(3, 70).unwrap();  // 入库单数
        worksheet.set_column_width_pixels(4, 70).unwrap();  // 入库条数
        worksheet.set_column_width_pixels(5, 90).unwrap();  // 出库金额
        worksheet.set_column_width_pixels(6, 90).unwrap();  // 下浮后出库金额
        worksheet.set_column_width_pixels(7, 70).unwrap();  // 出库单数
        worksheet.set_column_width_pixels(8, 70).unwrap();  // 出库条数
        worksheet.set_column_width_pixels(9, 90).unwrap();  // 毛利

        let mut prev_day = String::new();
        for (row_idx, row) in rows.iter().enumerate() {
            let sheet_row = (row_idx + 1) as u32;
            // 日期只在第一次出现时填写
            let day_display = if row.day != prev_day { row.day.as_str() } else { "" };
            prev_day = row.day.clone();

            if row.is_summary {
                worksheet.write_with_format(sheet_row, 0, day_display, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 1, &row.warehouse_name, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 2, row.in_amount, &num_format_sum).unwrap();
                worksheet.write_with_format(sheet_row, 3, row.in_order_count, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 4, row.in_item_count, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 5, row.out_amount, &num_format_sum).unwrap();
                worksheet.write_with_format(sheet_row, 6, row.discounted_out_amount, &num_format_sum).unwrap();
                worksheet.write_with_format(sheet_row, 7, row.out_order_count, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 8, row.out_item_count, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 9, row.gross_profit, &num_format_sum).unwrap();
            } else {
                worksheet.write(sheet_row, 0, day_display).unwrap();
                worksheet.write(sheet_row, 1, &row.warehouse_name).unwrap();
                worksheet.write_with_format(sheet_row, 2, row.in_amount, &num_format).unwrap();
                worksheet.write(sheet_row, 3, row.in_order_count).unwrap();
                worksheet.write(sheet_row, 4, row.in_item_count).unwrap();
                worksheet.write_with_format(sheet_row, 5, row.out_amount, &num_format).unwrap();
                worksheet.write_with_format(sheet_row, 6, row.discounted_out_amount, &num_format).unwrap();
                worksheet.write(sheet_row, 7, row.out_order_count).unwrap();
                worksheet.write(sheet_row, 8, row.out_item_count).unwrap();
                worksheet.write_with_format(sheet_row, 9, row.gross_profit, &num_format).unwrap();
            }
        }

        // 最后附加一个总计行
        let total_row = (rows.len() + 2) as u32;
        worksheet.write_with_format(total_row, 0, "合计", &summary_format).unwrap();
        worksheet.write_with_format(total_row, 2, total_in, &num_format_sum).unwrap();
        worksheet.write_with_format(total_row, 5, total_out, &num_format_sum).unwrap();
        worksheet.write_with_format(total_row, 6, total_discounted_out, &num_format_sum).unwrap();
        worksheet.write_with_format(total_row, 9, total_discounted_out - total_in, &num_format_sum).unwrap();

        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn stock_summary_reimburse(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let (rows, total_in, total_out, total_discounted_out) = compute_stock_summary_reimburse(start_date, end_date).await;

        let items: Vec<serde_json::Value> = rows.iter().map(|r| {
            serde_json::json!({
                "day": r.day,
                "warehouse_id": r.warehouse_id,
                "warehouse_name": r.warehouse_name,
                "is_summary": r.is_summary,
                "in_amount": r.in_amount,
                "in_item_count": r.in_item_count,
                "in_order_count": r.in_order_count,
                "out_amount": r.out_amount,
                "out_item_count": r.out_item_count,
                "out_order_count": r.out_order_count,
                "discounted_out_amount": r.discounted_out_amount,
                "gross_profit": r.gross_profit,
            })
        }).collect();

        let result = serde_json::json!({
            "rows": items,
            "total_in_amount": total_in,
            "total_out_amount": total_out,
            "total_discounted_out_amount": total_discounted_out,
            "total_gross_profit": total_discounted_out - total_in,
        });

        Ok(serde_json::to_string(&result).unwrap())
    }

    async fn export_stock_summary_reimburse(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let start_date = params.get("start_date").map(|s| s.as_str()).unwrap_or("");
        let end_date = params.get("end_date").map(|s| s.as_str()).unwrap_or("");
        let (rows, total_in, total_out, total_discounted_out) = compute_stock_summary_reimburse(start_date, end_date).await;

        let mut workbook = rust_xlsxwriter::Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("报销出入库统计").unwrap();

        let header_format = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0x2E75B6))
            .set_font_color(rust_xlsxwriter::Color::White)
            .set_align(rust_xlsxwriter::FormatAlign::Center)
            .set_border(rust_xlsxwriter::FormatBorder::Thin);
        let summary_format = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0xFFF2CC))
            .set_border(rust_xlsxwriter::FormatBorder::Thin);
        let num_format = rust_xlsxwriter::Format::new()
            .set_num_format("#,##0.00")
            .set_align(rust_xlsxwriter::FormatAlign::Right);
        let num_format_sum = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_num_format("#,##0.00")
            .set_background_color(rust_xlsxwriter::Color::RGB(0xFFF2CC))
            .set_align(rust_xlsxwriter::FormatAlign::Right);

        // 列顺序：日期、仓库、入库金额、入库单数、入库条数、出库金额、下浮后出库金额、出库单数、出库条数、毛利
        let headers = ["日期", "仓库", "入库金额", "入库单数", "入库条数", "出库金额", "下浮后出库金额", "出库单数", "出库条数", "毛利"];
        for (col, header) in headers.iter().enumerate() {
            worksheet.write_with_format(0, col as u16, *header, &header_format).unwrap();
        }
        worksheet.set_column_width(0, 14).unwrap();
        worksheet.set_column_width(1, 16).unwrap();
        worksheet.set_column_width_pixels(2, 90).unwrap();
        worksheet.set_column_width_pixels(3, 70).unwrap();
        worksheet.set_column_width_pixels(4, 70).unwrap();
        worksheet.set_column_width_pixels(5, 90).unwrap();
        worksheet.set_column_width_pixels(6, 90).unwrap();
        worksheet.set_column_width_pixels(7, 70).unwrap();
        worksheet.set_column_width_pixels(8, 70).unwrap();
        worksheet.set_column_width_pixels(9, 90).unwrap();

        let mut prev_day = String::new();
        for (row_idx, row) in rows.iter().enumerate() {
            let sheet_row = (row_idx + 1) as u32;
            let day_display = if row.day != prev_day { row.day.as_str() } else { "" };
            prev_day = row.day.clone();
            if row.is_summary {
                worksheet.write_with_format(sheet_row, 0, day_display, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 1, &row.warehouse_name, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 2, row.in_amount, &num_format_sum).unwrap();
                worksheet.write_with_format(sheet_row, 3, row.in_order_count, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 4, row.in_item_count, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 5, row.out_amount, &num_format_sum).unwrap();
                worksheet.write_with_format(sheet_row, 6, row.discounted_out_amount, &num_format_sum).unwrap();
                worksheet.write_with_format(sheet_row, 7, row.out_order_count, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 8, row.out_item_count, &summary_format).unwrap();
                worksheet.write_with_format(sheet_row, 9, row.gross_profit, &num_format_sum).unwrap();
            } else {
                worksheet.write(sheet_row, 0, day_display).unwrap();
                worksheet.write(sheet_row, 1, &row.warehouse_name).unwrap();
                worksheet.write_with_format(sheet_row, 2, row.in_amount, &num_format).unwrap();
                worksheet.write(sheet_row, 3, row.in_order_count).unwrap();
                worksheet.write(sheet_row, 4, row.in_item_count).unwrap();
                worksheet.write_with_format(sheet_row, 5, row.out_amount, &num_format).unwrap();
                worksheet.write_with_format(sheet_row, 6, row.discounted_out_amount, &num_format).unwrap();
                worksheet.write(sheet_row, 7, row.out_order_count).unwrap();
                worksheet.write(sheet_row, 8, row.out_item_count).unwrap();
                worksheet.write_with_format(sheet_row, 9, row.gross_profit, &num_format).unwrap();
            }
        }

        let total_row = (rows.len() + 2) as u32;
        worksheet.write_with_format(total_row, 0, "合计", &summary_format).unwrap();
        worksheet.write_with_format(total_row, 2, total_in, &num_format_sum).unwrap();
        worksheet.write_with_format(total_row, 5, total_out, &num_format_sum).unwrap();
        worksheet.write_with_format(total_row, 6, total_discounted_out, &num_format_sum).unwrap();
        worksheet.write_with_format(total_row, 9, total_discounted_out - total_in, &num_format_sum).unwrap();

        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn stock_warning(&self) -> Result<String, UseCaseError> {
        // 行为保持：查询失败 unwrap_or_default 返回空
        let low_rows = self.repo.list_stock_warning_low().await.unwrap_or_default();
        let high_rows = self.repo.list_stock_warning_high().await.unwrap_or_default();

        let low_stock: Vec<serde_json::Value> = low_rows.iter().map(|row| {
            serde_json::json!({
                "product_name": row.product_name,
                "spec": row.spec,
                "unit": row.unit,
                "current_stock": row.current_stock,
                "min_stock": row.limit_stock,
            })
        }).collect();

        let high_stock: Vec<serde_json::Value> = high_rows.iter().map(|row| {
            serde_json::json!({
                "product_name": row.product_name,
                "spec": row.spec,
                "unit": row.unit,
                "current_stock": row.current_stock,
                "max_stock": row.limit_stock,
            })
        }).collect();

        Ok(serde_json::to_string(&serde_json::json!({
            "low_stock": low_stock,
            "high_stock": high_stock,
        })).unwrap())
    }

    async fn export_stock_warning(&self) -> Result<Vec<u8>, UseCaseError> {
        // 行为保持：查询失败 unwrap_or_default 返回空
        let low_rows = self.repo.list_stock_warning_low_export().await.unwrap_or_default();
        let high_rows = self.repo.list_stock_warning_high_export().await.unwrap_or_default();
        let mut workbook=Workbook::new();let hf=common::utils::xlsx_header_format(0x2E75B6);
        let ws1=workbook.add_worksheet();ws1.set_name("低于最低库存").unwrap();
        for(c,h)in["商品名称","规格","单位","当前库存","最低库存","缺货数量"].iter().enumerate(){ws1.write_with_format(0,c as u16,*h,&hf).unwrap();}
        ws1.set_column_width(0,20).unwrap();ws1.set_column_width(1,14).unwrap();ws1.set_column_width(2,10).unwrap();ws1.set_column_width(3,14).unwrap();ws1.set_column_width(4,14).unwrap();ws1.set_column_width(5,14).unwrap();
        for(i,row)in low_rows.iter().enumerate(){let r=(i+1)as u32;let cur=row.current_stock;let min=row.limit_stock;ws1.write(r,0,row.product_name.as_str()).unwrap();ws1.write(r,1,row.spec.clone().unwrap_or_default()).unwrap();ws1.write(r,2,row.unit.clone().unwrap_or_default()).unwrap();ws1.write(r,3,cur).unwrap();ws1.write(r,4,min).unwrap();ws1.write(r,5,(min-cur).max(0.0)).unwrap();}
        let ws2=workbook.add_worksheet();ws2.set_name("高于最高库存").unwrap();
        for(c,h)in["商品名称","规格","单位","当前库存","最高库存","积压数量"].iter().enumerate(){ws2.write_with_format(0,c as u16,*h,&hf).unwrap();}
        ws2.set_column_width(0,20).unwrap();ws2.set_column_width(1,14).unwrap();ws2.set_column_width(2,10).unwrap();ws2.set_column_width(3,14).unwrap();ws2.set_column_width(4,14).unwrap();ws2.set_column_width(5,14).unwrap();
        for(i,row)in high_rows.iter().enumerate(){let r=(i+1)as u32;let cur=row.current_stock;let max=row.limit_stock;ws2.write(r,0,row.product_name.as_str()).unwrap();ws2.write(r,1,row.spec.clone().unwrap_or_default()).unwrap();ws2.write(r,2,row.unit.clone().unwrap_or_default()).unwrap();ws2.write(r,3,cur).unwrap();ws2.write(r,4,max).unwrap();ws2.write(r,5,(cur-max).max(0.0)).unwrap();}
        Ok(workbook.save_to_buffer().unwrap())
    }

    async fn slow_stock(&self, params: &HashMap<String, String>) -> Result<String, UseCaseError> {
        let days: i64 = params.get("days").and_then(|s| s.parse().ok()).unwrap_or(30);

        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self.repo.list_slow_stock(days).await.unwrap_or_default();

        let items: Vec<serde_json::Value> = rows.iter().map(|row| {
            serde_json::json!({
                "product_id": row.id,
                "product_name": row.product_name,
                "spec": row.spec,
                "unit": row.unit,
                "current_stock": row.current_stock,
                "last_sale_date": row.last_sale_date,
            })
        }).collect();

        Ok(serde_json::to_string(&items).unwrap())
    }

    async fn export_slow_stock(&self, params: &HashMap<String, String>) -> Result<Vec<u8>, UseCaseError> {
        let days: i64 = params.get("days").and_then(|s| s.parse().ok()).unwrap_or(30);
        // 行为保持：查询失败 unwrap_or_default 返回空
        let rows = self.repo.list_slow_stock_export(days).await.unwrap_or_default();
        let mut workbook=Workbook::new();let ws=workbook.add_worksheet();ws.set_name("呆滞库存").unwrap();let hf=common::utils::xlsx_header_format(0x2E75B6);
        for(c,h)in["商品名称","规格","单位","当前库存","库存金额","最后出库日期","呆滞天数"].iter().enumerate(){ws.write_with_format(0,c as u16,*h,&hf).unwrap();}
        ws.set_column_width(0,20).unwrap();ws.set_column_width(1,14).unwrap();ws.set_column_width(2,10).unwrap();ws.set_column_width(3,14).unwrap();ws.set_column_width(4,14).unwrap();ws.set_column_width(5,16).unwrap();ws.set_column_width(6,12).unwrap();
        for(i,row)in rows.iter().enumerate(){let r=(i+1)as u32;let cur=row.current_stock;let amt=row.amount;ws.write(r,0,row.product_name.as_str()).unwrap();ws.write(r,1,row.spec.clone().unwrap_or_default()).unwrap();ws.write(r,2,row.unit.clone().unwrap_or_default()).unwrap();ws.write(r,3,cur).unwrap();ws.write(r,4,amt).unwrap();ws.write(r,5,row.last_sale_date.as_str()).unwrap();ws.write(r,6,days).unwrap();}
        Ok(workbook.save_to_buffer().unwrap())
    }
}
