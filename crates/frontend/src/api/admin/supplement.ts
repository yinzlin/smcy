import { apiFetch } from '../client'
import { postAction } from '../purchase'

/** 耗材分摊：操作类型（老页面 operation_type 原样透传） */
export type SupplementOpType = 'new_item' | 'increase_quantity' | 'replace_item' | 'replace_remove' | 'replace_add'

/** /api/sales_order/by_purchaser/{purchaser_id} 返回的订单行 */
export interface SupplementOrder {
  id: number
  order_no: string
  order_date: string
  total_amount: number
  discount_rate: number
  discount_total: number
  status: string
  /** -1 未初始化 / 0 未分摊 / 1 分摊中 / 2 已完成 / 3 已终止 */
  allocation_status: number
  purchaser_name: string
  purchaser_id: number
}

/** /api/sales_order/detail/{id} 中老页面实际消费的明细字段 */
export interface SupplementDetailItem {
  id: number
  product_id: number
  product_name: string
  alias1: string | null
  alias2: string | null
  spec: string | null
  unit: string | null
  unit_price: number
  quantity: number
  amount: number
  category_name?: string | null
}

export interface SupplementOrderDetailResp {
  items?: SupplementDetailItem[]
}

/** /api/allocation/summary/{source_order_id} */
export interface AllocationSummary {
  exists: boolean
  id?: number
  source_order_id?: number
  total_amount: number
  allocated_amount: number
  remaining_balance: number
  /** 0 未分摊 / 1 分摊中 / 2 已完成 / 3 已终止 */
  status: number
  remark?: string
  created_at?: string
  completed_at?: string
  source_item_ids?: number[]
}

/** /api/supplement/list_by_source、list_by_target、create 的记录结构 */
export interface SupplementRecord {
  id: number | null
  target_order_id: number
  source_order_id: number
  source_order_no?: string
  target_order_no?: string
  source_remark?: string
  product_id: number
  product_name: string
  alias1: string | null
  alias2: string | null
  spec: string | null
  unit: string | null
  unit_price: number
  quantity: number
  amount: number
  allocate_date: string
  operation_type: SupplementOpType
  target_order_item_id?: number | null
}

/** /api/allocation/allocated_orders 行 */
export interface AllocatedOrderRow {
  alloc_id: number
  source_order_id: number
  order_no: string
  order_date: string
  purchaser_id: number
  purchaser_name: string
  total_amount: number
  allocated_amount: number
  remaining_balance: number
  status: number
  remark: string
  created_at: string
  completed_at: string
}

/** /api/product/list?page_size=50 行（老页面增项/替换商品搜索用） */
export interface SupplementProduct {
  id: number
  name: string
  alias1: string | null
  alias2: string | null
  spec: string | null
  unit: string
  category_name: string | null
  selling_price: number
  base_price: number
}

/** /api/supplement/compare/{order_id} 的商品行 */
export interface CompareItem {
  id: number
  product_id: number
  product_name: string
  display_name?: string
  alias1: string | null
  alias2: string | null
  spec: string | null
  unit: string | null
  unit_price: number
  quantity: number
  amount: number
  is_increase: boolean
  is_new: boolean
  is_replaced: boolean
  supplement_quantity: number
  supplement_amount: number
  total_quantity: number
  total_amount: number
}

/** /api/supplement/compare/{order_id} */
export interface CompareResp {
  real_total: number
  supplement_total: number
  allocation_total: number
  items: CompareItem[]
}

/** 替换明细「加行」的暂存行 */
export interface PendingReplaceLine {
  product_id: number
  product_name: string
  alias1: string
  alias2: string
  spec: string
  unit: string
  unit_price: number
  quantity: number
  amount: number
}

/** 按采购单位加载其全部销售订单（源/目标列表共用同一份数据，与老页面一致） */
export function listOrdersByPurchaser(purchaserId: number): Promise<SupplementOrder[]> {
  return apiFetch<SupplementOrder[]>(`/api/sales_order/by_purchaser/${purchaserId}`)
}

/** 订单明细（老页面只消费 items） */
export function getSupplementOrderDetail(orderId: number): Promise<SupplementOrderDetailResp> {
  return apiFetch<SupplementOrderDetailResp>(`/api/sales_order/detail/${orderId}`)
}

/** 源订单的分摊方案汇总 */
export function getAllocationSummary(sourceOrderId: number): Promise<AllocationSummary> {
  return apiFetch<AllocationSummary>(`/api/allocation/summary/${sourceOrderId}`)
}

/** 以某订单为来源的全部分摊记录（可回滚列表） */
export function listSupplementsBySource(orderId: number): Promise<SupplementRecord[]> {
  return apiFetch<SupplementRecord[]>(`/api/supplement/list_by_source/${orderId}`)
}

/** 以某订单为目标的全部分摊记录（增项列表） */
export function listSupplementsByTarget(orderId: number): Promise<SupplementRecord[]> {
  return apiFetch<SupplementRecord[]>(`/api/supplement/list_by_target/${orderId}`)
}

/** 所有已进入分摊的源订单 */
export function listAllocatedOrders(): Promise<AllocatedOrderRow[]> {
  return apiFetch<AllocatedOrderRow[]>('/api/allocation/allocated_orders')
}

/** 老页面商品搜索：/api/product/list?keyword=&page_size=50（兼容 items/data 两种包裹） */
export async function listSupplementProducts(keyword: string): Promise<SupplementProduct[]> {
  const resp = await apiFetch<{ items?: SupplementProduct[]; data?: SupplementProduct[] }>(
    `/api/product/list?keyword=${encodeURIComponent(keyword || '')}&page_size=50`,
  )
  return resp.items ?? resp.data ?? []
}

/** 真实账套 vs 分摊账套对比 */
export function compareSupplement(orderId: number): Promise<CompareResp> {
  return apiFetch<CompareResp>(`/api/supplement/compare/${orderId}`)
}

/** 初始化分摊方案 */
export function createAllocation(payload: { source_order_id: number; source_item_ids: number[]; remark: string }) {
  return postAction('/api/allocation/create', payload)
}

/** 终止分摊 */
export function terminateAllocation(payload: { source_order_id: number; remark: string }) {
  return postAction('/api/allocation/terminate', payload)
}

/** 取消分摊方案（仅在未产生分摊记录时可用） */
export function cancelAllocation(payload: { source_order_id: number }) {
  return postAction('/api/allocation/cancel', payload)
}

/** 确认完成分摊（可自动创建尾差冲销项）；message 为 JSON，含 auto_tail */
export function completeAllocation(payload: { source_order_id: number; target_order_id: number | null; auto_tail: boolean }) {
  return postAction('/api/allocation/complete', payload)
}

/** 保存单条增项 */
export function createSupplement(item: SupplementRecord) {
  return postAction('/api/supplement/create', item)
}

/** 回滚单条增项 */
export async function deleteSupplement(id: number): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch(`/api/supplement/delete/${id}`, {
    method: 'DELETE',
    credentials: 'same-origin',
  })
  return { ok: resp.ok, message: await resp.text() }
}
