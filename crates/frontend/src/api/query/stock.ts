import { apiFetch } from '../client'

// ===== 实时库存余额（旧路由 /query/stock_balance）=====

export interface StockBalanceItem {
  product_id: number
  product_name: string
  spec: string | null
  unit: string | null
  /** 实时模式：库存数量 */
  quantity?: number
  /** 实时=库存金额；历史=期末金额 */
  amount: number
  /** 历史模式：期初数量/本期入库/本期出库/期末数量 */
  opening_qty?: number
  period_in?: number
  period_out?: number
  closing_qty?: number
}

export interface StockBalanceResp {
  total: number
  /** 'history'=按单据日期区间查收发存；'realtime'=当前实时余额 */
  mode: string
  items: StockBalanceItem[]
}

export interface StockBalanceQuery {
  productName: string
  categoryId: string
  startDate: string
  endDate: string
  page: number
  pageSize: number
}

/** URL 与参数名从老页面 JS 照抄：起止日期仅在填了开始日期时才携带 */
export function stockBalanceQueryUrl(q: StockBalanceQuery): string {
  let url =
    '/api/query/stock_balance?product_name=' +
    encodeURIComponent(q.productName) +
    '&category_id=' +
    q.categoryId
  if (q.startDate) url += '&start_date=' + q.startDate + '&end_date=' + q.endDate
  return url + '&page=' + q.page + '&page_size=' + q.pageSize
}

/** 导出当前查询条件命中的全部结果（不受分页限制） */
export function stockBalanceExportUrl(q: Omit<StockBalanceQuery, 'page' | 'pageSize'>): string {
  let url =
    '/api/query/stock_balance/export?product_name=' +
    encodeURIComponent(q.productName) +
    '&category_id=' +
    q.categoryId
  if (q.startDate) url += '&start_date=' + q.startDate + '&end_date=' + q.endDate
  return url
}

export function queryStockBalance(q: StockBalanceQuery): Promise<StockBalanceResp> {
  return apiFetch<StockBalanceResp>(stockBalanceQueryUrl(q))
}

// ===== 出入库记录（旧路由 /query/stock_flow）=====

export type StockFlowView = 'timeline' | 'group'
export type StockFlowExportFormat = 'ledger' | 'timeline'

export interface StockFlowItem {
  product_id: number
  product_name: string
  spec: string | null
  unit: string | null
  in_quantity: number
  out_quantity: number
  balance: number
  create_time: string
  movement_type: string
  summary: string
  ref_no: string | null
  party_name: string | null
  production_date: string | null
  batch_no: string | null
  /** 大单位换算备注（与基础单位不同时展示"原X件"） */
  orig_unit: string | null
  orig_quantity: number
}

export interface StockFlowResp {
  total: number
  items: StockFlowItem[]
}

export interface StockFlowQuery {
  productName: string
  startDate: string
  endDate: string
  view: StockFlowView
  page: number
  pageSize: number
}

export function stockFlowQueryUrl(q: StockFlowQuery): string {
  return (
    '/api/query/stock_flow?product_name=' +
    encodeURIComponent(q.productName) +
    '&start_date=' +
    q.startDate +
    '&end_date=' +
    q.endDate +
    '&view=' +
    q.view +
    '&page=' +
    q.page +
    '&page_size=' +
    q.pageSize
  )
}

/** ledger=一品一户数量保管账；timeline=时间线连续补打（承接前期结存） */
export function stockFlowExportUrl(q: {
  productName: string
  startDate: string
  endDate: string
  format: StockFlowExportFormat
}): string {
  return (
    '/api/query/stock_flow/export?product_name=' +
    encodeURIComponent(q.productName) +
    '&start_date=' +
    q.startDate +
    '&end_date=' +
    q.endDate +
    '&format=' +
    q.format
  )
}

export function queryStockFlow(q: StockFlowQuery): Promise<StockFlowResp> {
  return apiFetch<StockFlowResp>(stockFlowQueryUrl(q))
}

/** 查看流水：老页面按 page_size=1000 循环翻页拉全量后拼接 */
export async function fetchAllStockFlow(productId: number): Promise<StockFlowItem[]> {
  const flows: StockFlowItem[] = []
  let page = 1
  let total = 0
  do {
    const data = await apiFetch<StockFlowResp>(
      '/api/query/stock_flow?product_id=' + productId + '&page=' + page + '&page_size=1000',
    )
    total = data.total || 0
    flows.push(...(data.items || []))
    page++
  } while (flows.length < total)
  return flows
}

// ===== 出入库统计（旧路由 /query/stock_summary、/query/stock_summary_reimburse）=====

export type StockSummaryKind = 'real' | 'reimburse'

export interface StockSummaryRow {
  day: string
  warehouse_name: string
  in_amount: number
  in_order_count: number
  in_item_count: number
  out_amount: number
  discounted_out_amount: number
  out_order_count: number
  out_item_count: number
  gross_profit: number
  /** 合计行：加粗 + 浅黄底 */
  is_summary?: boolean
}

export interface StockSummaryResp {
  rows: StockSummaryRow[]
  total_in_amount: number
  total_out_amount: number
  total_discounted_out_amount: number
  total_gross_profit: number
}

export interface StockSummaryQuery {
  startDate: string
  endDate: string
}

export function stockSummaryQueryUrl(kind: StockSummaryKind, q: StockSummaryQuery): string {
  const base = kind === 'real' ? '/api/query/stock_summary' : '/api/query/stock_summary_reimburse'
  return base + '?start_date=' + q.startDate + '&end_date=' + q.endDate
}

export function stockSummaryExportUrl(kind: StockSummaryKind, q: StockSummaryQuery): string {
  const base =
    kind === 'real' ? '/api/query/stock_summary/export' : '/api/query/stock_summary_reimburse/export'
  return base + '?start_date=' + q.startDate + '&end_date=' + q.endDate
}

export function queryStockSummary(kind: StockSummaryKind, q: StockSummaryQuery): Promise<StockSummaryResp> {
  return apiFetch<StockSummaryResp>(stockSummaryQueryUrl(kind, q))
}

// ===== 库存上下限预警（旧路由 /query/stock_warning）=====

export interface StockWarningLowItem {
  product_name: string
  spec: string | null
  unit: string | null
  current_stock: number
  min_stock: number
}

export interface StockWarningHighItem {
  product_name: string
  spec: string | null
  unit: string | null
  current_stock: number
  max_stock: number
}

export interface StockWarningResp {
  low_stock: StockWarningLowItem[]
  high_stock: StockWarningHighItem[]
}

export function queryStockWarning(): Promise<StockWarningResp> {
  return apiFetch<StockWarningResp>('/api/query/stock_warning')
}

export const STOCK_WARNING_EXPORT_URL = '/api/query/stock_warning/export'

// ===== 呆滞库存查询（旧路由 /query/slow_stock，默认 days=30）=====

export interface SlowStockItem {
  product_name: string
  spec: string | null
  unit: string | null
  stock_quantity: number
  stock_amount: number
  /** 空值展示为"从未出库" */
  last_out_date: string | null
  days: number
}

export function slowStockQueryUrl(days: number | ''): string {
  return '/api/query/slow_stock?days=' + days
}

export function querySlowStock(days: number | ''): Promise<SlowStockItem[]> {
  return apiFetch<SlowStockItem[]>(slowStockQueryUrl(days))
}

export function slowStockExportUrl(days: number | ''): string {
  return '/api/query/slow_stock/export?days=' + days
}
