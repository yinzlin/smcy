import { apiFetch } from '../client'

// ===== 采购汇总统计（旧路由 /query/purchase_summary）=====

export interface PurchaseSummarySupplierItem {
  name: string
  quantity: number
  amount: number
}

export interface PurchaseSummaryProductItem {
  product_name: string
  spec: string | null
  quantity: number
  amount: number
}

export interface PurchaseSummaryResp {
  by_supplier: PurchaseSummarySupplierItem[]
  by_product: PurchaseSummaryProductItem[]
}

/** URL 照老页面 JS 拼接：'/api/query/purchase_summary?start_date=xx&end_date=xx'（日期可为空串） */
export function getPurchaseSummary(start_date: string, end_date: string): Promise<PurchaseSummaryResp> {
  return apiFetch<PurchaseSummaryResp>(`/api/query/purchase_summary?start_date=${start_date}&end_date=${end_date}`)
}

// ===== 销售汇总报表（旧路由 /query/sales_summary）=====

export interface SalesSummaryPurchaserItem {
  name: string
  quantity: number
  sales_amount: number
  cost_amount: number
}

export interface SalesSummaryProductItem {
  product_name: string
  spec: string | null
  quantity: number
  sales_amount: number
  cost_amount: number
}

export interface SalesSummaryResp {
  by_purchaser: SalesSummaryPurchaserItem[]
  by_product: SalesSummaryProductItem[]
}

export function getSalesSummary(start_date: string, end_date: string): Promise<SalesSummaryResp> {
  return apiFetch<SalesSummaryResp>(`/api/query/sales_summary?start_date=${start_date}&end_date=${end_date}`)
}

/** 老页面导出按钮为固定链接（不带日期参数） */
export const SALES_SUMMARY_EXPORT_URL = '/api/query/sales_summary/export'

// ===== 畅销滞销商品查询（旧路由 /query/product_rank）=====

export interface ProductRankTopItem {
  product_name: string
  spec: string | null
  quantity: number
  amount: number
}

export interface ProductRankSlowItem {
  product_name: string
  spec: string | null
  stock_quantity: number
  last_sale_date: string | null
}

export interface ProductRankResp {
  top_selling: ProductRankTopItem[]
  slow_moving: ProductRankSlowItem[]
}

export function getProductRank(start_date: string, end_date: string): Promise<ProductRankResp> {
  return apiFetch<ProductRankResp>(`/api/query/product_rank?start_date=${start_date}&end_date=${end_date}`)
}

// ===== 报销口径汇总（旧路由 /query/reimburse_summary）=====

export interface ReimburseSummaryPurchaserItem {
  name: string
  real_amount: number
  supplement_amount: number
  reimburse_amount: number
}

export interface ReimburseSummaryProductItem {
  product_name: string
  spec: string | null
  quantity: number
  reimburse_amount: number
}

export interface ReimburseSummaryResp {
  by_purchaser: ReimburseSummaryPurchaserItem[]
  by_product: ReimburseSummaryProductItem[]
}

export function getReimburseSummary(start_date: string, end_date: string): Promise<ReimburseSummaryResp> {
  return apiFetch<ReimburseSummaryResp>(`/api/query/reimburse_summary?start_date=${start_date}&end_date=${end_date}`)
}

/** 老页面导出按钮为固定链接（不带日期参数） */
export const REIMBURSE_SUMMARY_EXPORT_URL = '/api/query/reimburse_summary/export'

// ===== 分摊来源统计（旧路由 /query/allocation_source）=====

export interface AllocationSourceTarget {
  order_no: string
  amount: number
}

export interface AllocationSourceItem {
  order_no: string
  order_date: string
  total_amount: number
  allocated_amount: number
  remaining_balance: number
  status: number
  /** 老页面 JS 按可空处理：(item.targets || []) */
  targets: AllocationSourceTarget[] | null
}

export function getAllocationSource(start_date: string, end_date: string): Promise<AllocationSourceItem[]> {
  return apiFetch<AllocationSourceItem[]>(`/api/query/allocation_source?start_date=${start_date}&end_date=${end_date}`)
}
