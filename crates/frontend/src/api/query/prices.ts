import { apiFetch } from '../client'

/** 分页响应：与老页面 JS 的 result.data/page/total_pages/total 契约一致 */
export interface PagedResp<T> {
  data: T[]
  page: number
  page_size: number
  total: number
  total_pages: number
}

// ===== 采购台账查询（旧路由 /query/purchase_price）=====

export interface PurchasePriceRow {
  order_date: string
  product_name: string
  unit: string | null
  unit_price: number
  quantity: number
  amount: number
  production_date: string | null
  shelf_life: string | null
  supplier_name: string
  remark: string | null
}

export interface PurchasePriceQuery {
  product_name: string
  supplier_id: number | null
  page: number
  page_size: number
}

export function fetchPurchasePrice(q: PurchasePriceQuery): Promise<PagedResp<PurchasePriceRow>> {
  const params = new URLSearchParams()
  params.set('product_name', q.product_name)
  params.set('supplier_id', q.supplier_id ? String(q.supplier_id) : '')
  params.set('page', String(q.page))
  params.set('page_size', String(q.page_size))
  return apiFetch<PagedResp<PurchasePriceRow>>(`/api/query/purchase_price?${params.toString()}`)
}

export function purchasePriceExportUrl(q: Pick<PurchasePriceQuery, 'product_name' | 'supplier_id'>): string {
  const params = new URLSearchParams()
  params.set('product_name', q.product_name)
  params.set('supplier_id', q.supplier_id ? String(q.supplier_id) : '')
  return `/api/query/purchase_price/export?${params.toString()}`
}

// ===== 销售价格查询（旧路由 /query/sales_price）=====

export interface SalesPriceRow {
  product_name: string
  spec: string | null
  purchaser_name: string
  unit_price: number
  order_date: string
  quantity: number
}

export interface SalesPriceQuery {
  product_name: string
  product_id: number | null
  purchaser_id: number | null
  page: number
  page_size: number
}

export function fetchSalesPrice(q: SalesPriceQuery): Promise<PagedResp<SalesPriceRow>> {
  const params = new URLSearchParams()
  params.set('product_name', q.product_name)
  params.set('product_id', q.product_id ? String(q.product_id) : '')
  params.set('purchaser_id', q.purchaser_id ? String(q.purchaser_id) : '')
  params.set('page', String(q.page))
  params.set('page_size', String(q.page_size))
  return apiFetch<PagedResp<SalesPriceRow>>(`/api/query/sales_price?${params.toString()}`)
}

// ===== 价格趋势（销售价格查询页的图表数据）=====

export interface PriceTrendDetail {
  /** 采购明细为供应商名，销售明细为采购单位名 */
  supplier_name?: string
  purchaser_name?: string
  unit: string
  unit_price: number
  quantity: number
  base_unit_price: number
}

export interface PriceTrendPoint {
  date: string
  avg_price: number
  min_price: number
  max_price: number
  count: number
  details: PriceTrendDetail[]
}

export interface PriceTrendResp {
  product_id: number
  product_name: string
  base_unit: string
  purchase_points: PriceTrendPoint[]
  selling_points: PriceTrendPoint[]
}

export function fetchPriceTrend(productId: number, startDate: string, endDate: string): Promise<PriceTrendResp> {
  let url = `/api/query/product_price_trend?product_id=${productId}`
  if (startDate) url += `&start_date=${encodeURIComponent(startDate)}`
  if (endDate) url += `&end_date=${encodeURIComponent(endDate)}`
  return apiFetch<PriceTrendResp>(url)
}

// ===== 供应商往来对账（旧路由 /query/supplier_balance）=====

export interface SupplierBalanceRow {
  name: string
  purchase_total: number
  paid_total: number
  unpaid: number
  prepay_balance: number
}

export function fetchSupplierBalance(): Promise<SupplierBalanceRow[]> {
  return apiFetch<SupplierBalanceRow[]>('/api/query/supplier_balance')
}

export const SUPPLIER_BALANCE_EXPORT_URL = '/api/query/supplier_balance/export'

// ===== 采购方应收对账（旧路由 /query/purchaser_balance）=====

export interface PurchaserBalanceRow {
  name: string
  sales_total: number
  received_total: number
  unreceived: number
  prepay_balance: number
}

export function fetchPurchaserBalance(): Promise<PurchaserBalanceRow[]> {
  return apiFetch<PurchaserBalanceRow[]>('/api/query/purchaser_balance')
}

export const PURCHASER_BALANCE_EXPORT_URL = '/api/query/purchaser_balance/export'
