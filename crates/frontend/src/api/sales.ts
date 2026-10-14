import { apiFetch } from './client'

export interface Purchaser {
  id: number
  name: string
  contact: string | null
  phone: string | null
  address: string | null
  audit_status: string | null
  category_name: string | null
}

export interface SalesOrderItem {
  id: number
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
  pre_sale_quantity: number | null
  supplier_id: number | null
  supplier_name: string | null
  production_date: string | null
  batch_no: string | null
  remark: string | null
}

export interface SalesOrderDetail {
  id: number
  purchaser_id: number
  purchaser_name: string
  order_no: string
  order_date: string
  total_amount: number
  discount_rate: number
  amount_reduction: number
  final_amount: number
  warehouse_id: number | null
  warehouse_name: string | null
  status: string
  version: number
  remark: string | null
  customer_order_image: string | null
  signed_order_image: string | null
  supplier_company: string | null
  truck_plate: string | null
  is_settled: number
  items: SalesOrderItem[]
}

export interface SalesOrderListItem {
  id: number
  order_no: string
  order_date: string
  total_amount: number
  discount_rate: number
  amount_reduction: number
  final_amount: number
  status: string
  is_settled: number
  is_reimburse: number
  purchaser_name: string
}

export interface SalesOrderListResp {
  data: SalesOrderListItem[]
  page: number
  page_size: number
  total: number
  total_pages: number
}

export interface SalesOrderItemPayload {
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
  pre_sale_quantity: number | null
  supplier_id: number
  supplier_name: string
  remark: string | null
}

export interface SalesOrderPayload {
  id: number | null
  purchaser_id: number
  order_no: string
  order_date: string
  total_amount: number
  discount_rate: number
  amount_reduction: number
  final_amount: number
  warehouse_id: number
  warehouse_name: string
  items: SalesOrderItemPayload[]
  remark: string | null
  supplier_company: string | null
  truck_plate: string | null
  version: number | null
}

export interface SalesOrderListQuery {
  page: number
  page_size: number
  keyword: string
  purchaser_id: number | null
  is_settled: number | null
  sort_field: string
  sort_order: 'asc' | 'desc'
}

export interface SchedulePriceEntry {
  gov_procurement: number
  supermarket_1: number
  supermarket_2: number
  supermarket_3: number
  ai_realtime: number
}

export interface LookupBatchResp {
  success: boolean
  date: string
  prices: Record<string, SchedulePriceEntry>
}

export interface UpdatePricesItem {
  item_id: number
  product_id: number
  unit_price: number
  amount: number
}

export interface UpdatePricesResp {
  items: UpdatePricesItem[]
  total_amount: number
  final_amount: number
  errors: string[]
}

export function listPurchasers(): Promise<Purchaser[]> {
  return apiFetch<Purchaser[]>('/api/purchaser/list')
}

export function getUserContactPhone(): Promise<{ success: boolean; contact_phone: string }> {
  return apiFetch<{ success: boolean; contact_phone: string }>('/api/user/contact_phone')
}

export function setUserContactPhone(phone: string): Promise<{ success: boolean }> {
  return apiFetch<{ success: boolean }>('/api/user/contact_phone', {
    method: 'POST',
    body: JSON.stringify({ contact_phone: phone }),
  })
}

export function listSalesOrders(q: SalesOrderListQuery): Promise<SalesOrderListResp> {
  const params = new URLSearchParams()
  params.set('page', String(q.page))
  params.set('page_size', String(q.page_size))
  if (q.keyword) params.set('keyword', q.keyword)
  if (q.purchaser_id) params.set('purchaser_id', String(q.purchaser_id))
  if (q.is_settled !== null) params.set('is_settled', String(q.is_settled))
  if (q.sort_field) {
    params.set('sort_field', q.sort_field)
    params.set('sort_order', q.sort_order)
  }
  return apiFetch<SalesOrderListResp>(`/api/sales_order/list?${params.toString()}`)
}

export function getSalesOrderDetail(id: number): Promise<SalesOrderDetail> {
  return apiFetch<SalesOrderDetail>(`/api/sales_order/detail/${id}`)
}

export function generateSalesOrderNo(date: string): Promise<{ order_no: string }> {
  return apiFetch<{ order_no: string }>(`/api/order/generate_no?type=sales&date=${encodeURIComponent(date)}`)
}

/** 价格策略时段批量查询：政采价/商超1/2/3/AI 实时价 */
export function lookupPriceScheduleBatch(date: string, productIds: number[]): Promise<LookupBatchResp> {
  return apiFetch<LookupBatchResp>('/api/price_schedule/lookup_batch', {
    method: 'POST',
    body: JSON.stringify({ date, product_ids: productIds }),
  })
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

export function createSalesOrder(payload: SalesOrderPayload) {
  return postAction('/api/sales_order/create', payload)
}

export function updateSalesOrder(payload: SalesOrderPayload) {
  return postAction('/api/sales_order/update', payload)
}

export function settleSalesOrder(id: number, isSettled: number) {
  return postAction(`/api/sales_order/settle/${id}`, { is_settled: isSettled })
}

export function unapproveSalesOrder(id: number, reason: string) {
  return postAction(`/api/sales_order/unapprove/${id}`, { reason })
}

export function updateSalesOrderStatus(id: number, status: string) {
  return postAction('/api/sales_order/update_status', { id: String(id), status })
}

/** 一键更新售价：后端按商品基础售价返回明细新价（基础单位口径） */
export function updateSalesOrderPrices(id: number): Promise<UpdatePricesResp> {
  return postAction(`/api/sales_order/update_prices/${id}`).then(async (r) => {
    if (!r.ok) throw new Error(r.message || '获取失败')
    return JSON.parse(r.message) as UpdatePricesResp
  })
}

export async function deleteSalesOrder(id: number): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch(`/api/sales_order/delete/${id}`, {
    method: 'DELETE',
    credentials: 'same-origin',
  })
  return { ok: resp.ok, message: await resp.text() }
}

export async function importSalesOrders(text: string): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch('/api/sales_order/import', {
    method: 'POST',
    credentials: 'same-origin',
    body: text,
  })
  return { ok: resp.ok, message: await resp.text() }
}

export function exportSalesOrdersUrl(q: Pick<SalesOrderListQuery, 'keyword' | 'purchaser_id' | 'sort_field' | 'sort_order'>): string {
  const params = new URLSearchParams()
  if (q.keyword) params.set('keyword', q.keyword)
  if (q.purchaser_id) params.set('purchaser_id', String(q.purchaser_id))
  if (q.sort_field) {
    params.set('sort_field', q.sort_field)
    params.set('sort_order', q.sort_order)
  }
  const qs = params.toString()
  return `/api/sales_order/export${qs ? `?${qs}` : ''}`
}

/** 生成采购订单：error=已处理不允许重新生成；warning=pending 重复需确认后 force 重建 */
export function generatePurchaseOrders(id: number, force = false): Promise<{ json: Record<string, unknown>; ok: boolean }> {
  return fetch(`/api/sales_order/generate_purchase/${id}?force=${force ? 1 : 0}`, {
    method: 'POST',
    credentials: 'same-origin',
  }).then(async (resp) => ({ json: (await resp.json()) as Record<string, unknown>, ok: resp.ok }))
}

export function uploadSalesOrderImage(orderId: number, type: 'customer' | 'signed', file: File): Promise<{ url: string }> {
  const form = new FormData()
  form.append('file', file)
  return fetch(`/api/sales_order/upload_image?order_id=${orderId}&type=${type}`, {
    method: 'POST',
    credentials: 'same-origin',
    body: form,
  }).then(async (resp) => {
    if (!resp.ok) throw new Error(await resp.text())
    return (await resp.json()) as { url: string }
  })
}

export function deleteSalesOrderImage(orderId: number, type: 'customer' | 'signed') {
  return postAction(`/api/sales_order/delete_image?order_id=${orderId}&type=${type}`)
}

/** 下载 Excel（验收单/报销单）；后端可能返回 JSON 警告（需 force 确认）或错误信息 */
export async function downloadSalesExcel(
  url: string,
  confirmForce: (message: string) => Promise<boolean>,
  alertMessage: (message: string) => void,
): Promise<void> {
  const resp = await fetch(url, { credentials: 'same-origin' })
  const contentType = resp.headers.get('content-type') || ''
  if (contentType.includes('application/json')) {
    const data = (await resp.json()) as { warning?: boolean; error?: boolean; message?: string }
    if (data.warning && data.message) {
      const ok = await confirmForce(data.message)
      if (ok) await downloadSalesExcel(`${url}${url.includes('?') ? '&' : '?'}force=1`, confirmForce, alertMessage)
    } else if (data.message) {
      alertMessage(data.message)
    }
    return
  }
  const blob = await resp.blob()
  const disposition = resp.headers.get('content-disposition') || ''
  const match = disposition.match(/filename\*?=(?:UTF-8''|"?)([^;"]+)/)
  const filename = match ? decodeURIComponent(match[1]) : 'export.xlsx'
  const link = document.createElement('a')
  link.href = URL.createObjectURL(blob)
  link.download = filename
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(link.href)
}

export function acceptExcelUrl(id: number): string {
  return `/api/sales_order/accept_excel/${id}`
}

export function realExcelUrl(id: number): string {
  return `/api/sales_order/real_excel/${id}`
}
