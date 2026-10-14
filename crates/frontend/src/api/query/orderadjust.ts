/**
 * 旧版查询页 API 层（/query/order_adjust 订单调整与同屏比对）
 * URL 与参数照抄老页面 JS 拼接逻辑（含空参数原样拼接）
 */
import { apiFetch } from '../client'
import type { SalesOrderDetail } from '../sales'

// ===== 订单号搜索（/api/sales_order/list，老页面读取 items || data）=====

export interface OrderSearchItem {
  id: number
  order_no: string
  order_date: string
  purchaser_name: string | null
  final_amount: number
  total_amount: number
}

export function fetchOrderSearch(keyword: string): Promise<{ items?: OrderSearchItem[]; data?: OrderSearchItem[] }> {
  return apiFetch<{ items?: OrderSearchItem[]; data?: OrderSearchItem[] }>(
    '/api/sales_order/list?keyword=' + encodeURIComponent(keyword) + '&page=1&page_size=20',
  )
}

export function fetchOrderDetail(id: number): Promise<SalesOrderDetail> {
  return apiFetch<SalesOrderDetail>('/api/sales_order/detail/' + id)
}

// ===== 商品搜索（/api/product/list，老页面读取 items || data）=====

export interface ProductPickItem {
  id: number
  name: string
  alias1: string | null
  alias2: string | null
  spec: string | null
  unit: string | null
  selling_price: number
  base_price: number
}

export function fetchProductPickList(keyword: string): Promise<{ items?: ProductPickItem[]; data?: ProductPickItem[] }> {
  return apiFetch<{ items?: ProductPickItem[]; data?: ProductPickItem[] }>(
    '/api/product/list?keyword=' + encodeURIComponent(keyword || '') + '&page_size=50',
  )
}

// ===== 有变更的订单列表（/api/supplement/adjusted_orders）=====

export interface AdjustedOrderRow {
  id: number
  order_no: string
  order_date: string
  purchaser_name: string
  purchaser_id: number
  total_amount: number
  adjust_amount: number
  adjusted_total: number
  adjust_count: number
  last_adjust_date: string
}

export interface AdjustedOrdersResp {
  total: number
  page: number
  page_size: number
  total_real_amount: number
  total_adjust_amount: number
  total_adjusted_amount: number
  items: AdjustedOrderRow[]
}

export interface AdjustedOrdersParams {
  page: number
  page_size: number
  keyword: string
  purchaser_id: string
  sort_order: string
}

/** 老页面无条件拼接全部参数（含空值） */
export function fetchAdjustedOrders(p: AdjustedOrdersParams): Promise<AdjustedOrdersResp> {
  return apiFetch<AdjustedOrdersResp>(
    '/api/supplement/adjusted_orders?page=' + p.page + '&page_size=' + p.page_size +
    '&keyword=' + encodeURIComponent(p.keyword) + '&purchaser_id=' + encodeURIComponent(p.purchaser_id) +
    '&sort_order=' + p.sort_order,
  )
}

// ===== 真实/调整后订单同屏比对（/api/supplement/compare/{order_id}）=====

export interface CompareItem {
  id: number
  product_name: string
  display_name: string
  quantity: number
  amount: number
  is_new: boolean
  is_increase: boolean
  is_replaced: boolean
  supplement_quantity: number
  total_quantity: number
  total_amount: number
}

export interface CompareResp {
  real_total: number
  supplement_total: number
  allocation_total: number
  items: CompareItem[]
}

export function fetchSupplementCompare(orderId: number): Promise<CompareResp> {
  return apiFetch<CompareResp>('/api/supplement/compare/' + orderId)
}

// ===== 本单调整记录（/api/supplement/list_by_target/{order_id}）=====

export interface SupplementRecord {
  id: number
  target_order_id: number
  source_order_id: number
  source_order_no: string
  source_remark: string
  product_id: number
  product_name: string
  alias1: string
  alias2: string
  spec: string
  unit: string
  unit_price: number
  quantity: number
  amount: number
  allocate_date: string
  operation_type: string
  target_order_item_id: number | null
}

export function fetchSupplementRecords(orderId: number): Promise<SupplementRecord[]> {
  return apiFetch<SupplementRecord[]>('/api/supplement/list_by_target/' + orderId)
}

// ===== 创建 / 回滚 =====

/** 选单来源统一结构（订单搜索项 / 变更订单列表行均可） */
export interface SelectedOrder {
  id: number
  order_no: string
  purchaser_name: string | null
}

export interface SupplementCreateBody {
  target_order_id: number
  source_order_id: number
  source_remark: string
  product_id: number
  product_name: string
  alias1: string
  alias2: string
  spec: string
  unit: string
  unit_price: number
  quantity: number
  amount: number
  allocate_date: string
  operation_type: 'new_item' | 'increase_quantity' | 'replace_remove' | 'replace_add'
  target_order_item_id: number | null
}

/** POST /api/supplement/create；成功 2xx，失败 body 为文本原因 */
export async function createSupplement(body: SupplementCreateBody): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch('/api/supplement/create', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  return { ok: resp.ok, message: await resp.text() }
}

/** DELETE /api/supplement/delete/{id}；成功 2xx，失败 body 为文本原因 */
export async function deleteSupplement(id: number): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch('/api/supplement/delete/' + id, {
    method: 'DELETE',
    credentials: 'same-origin',
  })
  return { ok: resp.ok, message: await resp.text() }
}
