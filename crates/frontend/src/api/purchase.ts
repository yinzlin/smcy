import { apiFetch } from './client'

export interface Supplier {
  id: number
  name: string
  contact: string | null
  phone: string | null
  address: string | null
  audit_status: string | null
  is_designated: number
  designated_no: string | null
  license_no: string | null
  license_expire: string | null
}

export interface Warehouse {
  id: number
  name: string
  status: number
}

export interface HandlerUser {
  id: number
  username: string
  nickname: string
  phone: string
  role: string
}

export interface ProductSearchItem {
  id: number
  name: string
  alias1: string | null
  alias2: string | null
  spec: string | null
  unit: string
  base_unit: string
  base_price: number
  purchase_price: number
  selling_price: number
  category_name: string | null
}

export interface UnitOption {
  name: string
  ratio: number
  unit_price: number
  purchase_price: number
}

export interface ProductBrief {
  id?: number
  name?: string
  base_unit?: string
  base_price?: number
  purchase_price?: number
}

export interface LastPurchasePrice {
  purchase_price: number
  base_unit: string
  last_unit_price: number
  last_unit: string
}

export interface CompareSupplier {
  supplier_id: number
  supplier_name: string
  last_date: string
  last_unit_price: number
  last_unit: string
  last_base_unit_price: number
  avg_price: number
  min_price: number
  max_price: number
  order_count: number
  today_price: number
  today_updated: string
}

export interface PriceCompareResp {
  product_name?: string
  base_unit?: string
  suppliers?: CompareSupplier[]
}

export interface ContractPriceResp {
  matched: boolean
  price: number
  effective_date: string | null
}

export interface PurchaseOrderItem {
  id: number | null
  product_id: number
  product_name: string
  alias1: string | null
  alias2: string | null
  spec: string | null
  unit: string | null
  unit_price: number
  quantity: number
  base_quantity: number | null
  amount: number
  ordered_quantity: number | null
  remark: string | null
  warehouse_id: number | null
  warehouse_name: string | null
  production_date: string | null
  batch_no: string | null
}

export interface PurchaseOrderDetail {
  id: number
  supplier_id: number
  supplier_name: string
  order_no: string
  order_date: string
  total_amount: number
  discount_rate: number
  amount_reduction: number
  final_amount: number
  warehouse_id: number
  warehouse_name: string | null
  status: string
  version: number
  is_settled: number
  remark: string | null
  user_id?: number
  items: PurchaseOrderItem[]
}

export interface PurchaseOrderListItem {
  id: number
  order_no: string
  order_date: string
  total_amount: number
  discount_rate: number
  amount_reduction: number
  final_amount: number
  warehouse_id: number
  warehouse_name: string
  status: string
  remark: string | null
  supplier_name: string
  is_settled: number
}

export interface PurchaseOrderListResp {
  data: PurchaseOrderListItem[]
  page: number
  page_size: number
  total: number
  total_pages: number
}

export interface OrderItemPayload {
  id: number | null
  product_id: number
  product_name: string
  alias1: string | null
  alias2: string | null
  spec: string | null
  unit: string | null
  unit_price: number
  quantity: number
  base_quantity: number | null
  amount: number
  ordered_quantity: number | null
  remark: string | null
  warehouse_id: number | null
  warehouse_name: string | null
  production_date: string | null
  batch_no: string | null
}

export interface OrderPayload {
  id: number | null
  supplier_id: number
  order_no: string
  order_date: string
  total_amount: number
  discount_rate: number
  amount_reduction: number
  final_amount: number
  warehouse_id: number
  warehouse_name: string
  user_id: number | null
  items: OrderItemPayload[]
  remark: string | null
  version: number | null
  is_settled?: number
}

export interface OrderListQuery {
  page: number
  page_size: number
  keyword: string
  supplier_id: number | null
  is_settled: number | null
  sort_field: string
  sort_order: 'asc' | 'desc'
}

export function listPurchaseOrders(q: OrderListQuery): Promise<PurchaseOrderListResp> {
  const params = new URLSearchParams()
  params.set('page', String(q.page))
  params.set('page_size', String(q.page_size))
  if (q.keyword) params.set('keyword', q.keyword)
  if (q.supplier_id) params.set('supplier_id', String(q.supplier_id))
  if (q.is_settled !== null) params.set('is_settled', String(q.is_settled))
  if (q.sort_field) {
    params.set('sort_field', q.sort_field)
    params.set('sort_order', q.sort_order)
  }
  return apiFetch<PurchaseOrderListResp>(`/api/purchase_order/list?${params.toString()}`)
}

export function getPurchaseOrderDetail(id: number): Promise<PurchaseOrderDetail> {
  return apiFetch<PurchaseOrderDetail>(`/api/purchase_order/detail/${id}`)
}

export function listSuppliers(): Promise<Supplier[]> {
  return apiFetch<Supplier[]>('/api/supplier/list')
}

export function listWarehouses(): Promise<Warehouse[]> {
  return apiFetch<Warehouse[]>('/api/warehouse/list')
}

export function listUsers(): Promise<HandlerUser[]> {
  return apiFetch<HandlerUser[]>('/api/user/list')
}

export function searchProducts(keyword: string): Promise<ProductSearchItem[]> {
  return apiFetch<ProductSearchItem[]>(`/api/product/search?audited=1&keyword=${encodeURIComponent(keyword)}`)
}

export function getProductById(id: number): Promise<ProductBrief> {
  return apiFetch<ProductBrief>(`/api/product/by_id?id=${id}`)
}

export function getProductUnits(productId: number): Promise<UnitOption[]> {
  return apiFetch<UnitOption[]>(`/api/product/unit/list?product_id=${productId}`)
}

export function getLastPurchasePrice(productId: number): Promise<LastPurchasePrice> {
  return apiFetch<LastPurchasePrice>(`/api/product/last_purchase_price?product_id=${productId}`)
}

export function getPriceCompare(productId: number): Promise<PriceCompareResp> {
  return apiFetch<PriceCompareResp>(`/api/query/purchase_price_compare?product_id=${productId}`)
}

export function lookupContractPrice(productId: number, supplierId: number, queryDate: string): Promise<ContractPriceResp> {
  const params = new URLSearchParams({
    product_id: String(productId),
    price_type: 'supplier_contract',
    supplier_id: String(supplierId),
    query_date: queryDate,
  })
  return apiFetch<ContractPriceResp>(`/api/price_schedule/lookup?${params.toString()}`)
}

export function generateOrderNo(date: string): Promise<{ order_no: string }> {
  return apiFetch<{ order_no: string }>(`/api/order/generate_no?type=purchase&date=${encodeURIComponent(date)}`)
}

/** POST JSON；成功返回 2xx，失败时 body 为文本原因 */
export async function postAction(url: string, body?: unknown): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch(url, {
    method: 'POST',
    credentials: 'same-origin',
    headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  return { ok: resp.ok, message: await resp.text() }
}

export function createPurchaseOrder(payload: OrderPayload) {
  return postAction('/api/purchase_order/create', payload)
}

export function updatePurchaseOrder(payload: OrderPayload) {
  return postAction('/api/purchase_order/update', payload)
}

export function approvePurchaseOrder(id: number) {
  return postAction(`/api/purchase_order/approve/${id}`, { reason: '' })
}

export function unapprovePurchaseOrder(id: number, reason: string) {
  return postAction(`/api/purchase_order/unapprove/${id}`, { reason })
}

export function settlePurchaseOrder(id: number, isSettled: number) {
  return postAction(`/api/purchase_order/settle/${id}`, { is_settled: isSettled })
}

export async function deletePurchaseOrder(id: number): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch(`/api/purchase_order/delete/${id}`, {
    method: 'DELETE',
    credentials: 'same-origin',
  })
  return { ok: resp.ok, message: await resp.text() }
}

export async function importPurchaseOrders(text: string): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch('/api/purchase_order/import', {
    method: 'POST',
    credentials: 'same-origin',
    body: text,
  })
  return { ok: resp.ok, message: await resp.text() }
}

export function exportPurchaseOrdersUrl(q: Pick<OrderListQuery, 'keyword' | 'supplier_id' | 'sort_field' | 'sort_order'>): string {
  const params = new URLSearchParams()
  if (q.keyword) params.set('keyword', q.keyword)
  if (q.supplier_id) params.set('supplier_id', String(q.supplier_id))
  if (q.sort_field) {
    params.set('sort_field', q.sort_field)
    params.set('sort_order', q.sort_order)
  }
  const qs = params.toString()
  return `/api/purchase_order/export${qs ? `?${qs}` : ''}`
}

export function exportPrintUrl(orderId: number, userId: number): string {
  return `/api/purchase_order/export_print/${orderId}${userId > 0 ? `?user_id=${userId}` : ''}`
}
