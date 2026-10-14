import { apiFetch } from './client'
import type { ProductSearchItem } from './purchase'

export type PriceType =
  | 'gov_procurement'
  | 'supermarket_1'
  | 'supermarket_2'
  | 'supermarket_3'
  | 'ai_realtime'
  | 'supplier_contract'
  | 'purchaser_contract'

export const PRICE_TYPE_LABEL: Record<PriceType, string> = {
  gov_procurement: '政采价',
  supermarket_1: '超市1价',
  supermarket_2: '超市2价',
  supermarket_3: '超市3价',
  ai_realtime: 'AI实时价',
  supplier_contract: '供应商约定采购价',
  purchaser_contract: '采购单位约定售价',
}

export const PRICE_TYPE_OPTIONS = (Object.keys(PRICE_TYPE_LABEL) as PriceType[]).map((k) => ({
  value: k,
  label: PRICE_TYPE_LABEL[k],
}))

export const PRICE_TYPE_COLOR: Record<PriceType, string> = {
  gov_procurement: 'blue',
  supermarket_1: 'gold',
  supermarket_2: 'gold',
  supermarket_3: 'gold',
  ai_realtime: 'purple',
  supplier_contract: 'green',
  purchaser_contract: 'magenta',
}

export interface PriceScheduleRow {
  id: number
  product_id: number
  product_name: string
  spec: string | null
  unit: string | null
  alias1: string | null
  alias2: string | null
  price_type: PriceType
  price: number
  /** 生效日期（含） */
  effective_date: string
  /** SQL 语义上的开区间上界（不含）；NULL=至今 */
  end_date: string | null
  source: string | null
  remark: string | null
  create_at: string | null
  supplier_id: number | null
  purchaser_id: number | null
  supplier_name: string
  purchaser_name: string
}

export interface PriceScheduleListResp {
  data: PriceScheduleRow[]
  page: number
  page_size: number
  total: number
  total_pages: number
}

export interface PriceScheduleListQuery {
  page: number
  page_size: number
  price_type: string
  effective_from: string
  effective_to: string
  keyword: string
  product_id: number | null
}

export interface PriceSchedulePayload {
  id: number | null
  product_id: number
  price_type: PriceType
  price: number
  effective_date: string
  end_date: null
  source: string
  remark: string
  supplier_id: number | null
  purchaser_id: number | null
}

export interface DiagnosePeriod {
  id: number
  price: number
  effective_date: string
  end_date: string | null
  source: string | null
  remark: string | null
  supplier_id: number | null
  purchaser_id: number | null
}

export interface DiagnoseResp {
  success: boolean
  product_id: number
  product_name: string
  types: Partial<Record<PriceType, DiagnosePeriod[]>>
}

export interface BackfillResp {
  success: boolean
  order_date: string
  price_type: string
  updated: number
  kept: number
  missing: string[]
  message?: string
}

export interface BatchImportExcelResp {
  inserted: number
  skipped: number
  errors: string[]
}

export function listPriceSchedules(q: PriceScheduleListQuery): Promise<PriceScheduleListResp> {
  const params = new URLSearchParams()
  params.set('page', String(q.page))
  params.set('page_size', String(q.page_size))
  if (q.price_type) params.set('price_type', q.price_type)
  if (q.effective_from) params.set('effective_from', q.effective_from)
  if (q.effective_to) params.set('effective_to', q.effective_to)
  if (q.keyword) params.set('keyword', q.keyword)
  if (q.product_id) params.set('product_id', String(q.product_id))
  return apiFetch<PriceScheduleListResp>(`/api/price_schedule/list?${params.toString()}`)
}

export function getPriceSchedule(id: number): Promise<{ success: boolean; data: PriceScheduleRow }> {
  return apiFetch<{ success: boolean; data: PriceScheduleRow }>(`/api/price_schedule/get?id=${id}`)
}

export function diagnosePriceSchedule(productId: number): Promise<DiagnoseResp> {
  return apiFetch<DiagnoseResp>(`/api/price_schedule/diagnose?product_id=${productId}`)
}

/** create/update/delete 均为 POST JSON；成功 2xx，失败 body 为 JSON {message} */
export async function postPriceSchedule(url: string, body: unknown): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch(url, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  return { ok: resp.ok, message: await resp.text() }
}

export function createPriceSchedule(payload: PriceSchedulePayload) {
  return postPriceSchedule('/api/price_schedule/create', payload)
}

export function updatePriceSchedule(payload: PriceSchedulePayload) {
  return postPriceSchedule('/api/price_schedule/update', payload)
}

export function deletePriceSchedule(id: number) {
  return postPriceSchedule('/api/price_schedule/delete', { id })
}

export function backfillSalesOrder(orderId: number, priceType: string, force: boolean): Promise<BackfillResp> {
  return apiFetch<BackfillResp>(
    `/api/price_schedule/backfill_sales_order?order_id=${orderId}&price_type=${encodeURIComponent(priceType)}&force=${force ? 1 : 0}`,
  )
}

export function batchImportExcel(file: File): Promise<BatchImportExcelResp> {
  const form = new FormData()
  form.append('file', file)
  return fetch('/api/price_schedule/batch_import_excel', {
    method: 'POST',
    credentials: 'same-origin',
    body: form,
  }).then(async (resp) => {
    if (!resp.ok) {
      const text = await resp.text()
      let msg = text
      try {
        msg = (JSON.parse(text) as { message?: string }).message || text
      } catch {
        // 纯文本
      }
      throw new Error(msg)
    }
    return (await resp.json()) as BatchImportExcelResp
  })
}

export function exportListUrl(q: Omit<PriceScheduleListQuery, 'page' | 'page_size'>): string {
  const params = new URLSearchParams()
  if (q.price_type) params.set('price_type', q.price_type)
  if (q.effective_from) params.set('effective_from', q.effective_from)
  if (q.effective_to) params.set('effective_to', q.effective_to)
  if (q.keyword) params.set('keyword', q.keyword)
  if (q.product_id) params.set('product_id', String(q.product_id))
  const qs = params.toString()
  return `/api/price_schedule/export${qs ? `?${qs}` : ''}`
}

export const EXPORT_TEMPLATE_URL = '/api/price_schedule/export_template'

export type { ProductSearchItem }
