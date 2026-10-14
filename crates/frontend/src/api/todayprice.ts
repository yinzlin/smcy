import { apiFetch } from './client'

export interface TodayPriceItem {
  product_id: number
  product_name: string
  spec: string
  base_unit: string
  /** 基础单位换算合计（不同单位仅简单相加，不做换算精化） */
  total_qty: number
  /** 多单位明细，如 "2箱 + 5斤"；全部为基础单位时缺省 */
  qty_detail?: string
  /** 最近价（全局） */
  purchase_price: number
  max_purchase_price: number
  min_purchase_price: number
  /** 分供应商最近价 */
  supplier_price: number
}

export interface TodayPriceSupplier {
  supplier_id: number
  supplier_name: string
  items: TodayPriceItem[]
  total_quantity: number
}

export interface TodayPriceSaveItem {
  supplier_id: number
  product_id: number
  price: number
}

export interface TodayPriceSaveResp {
  success: boolean
  message: string
}

/** 按供应商分拣数据（仅用于收集订单 id 批量生成采购订单） */
export interface SortSupplierNode {
  purchasers?: { items?: { order_id?: number }[] }[]
}

export function fetchTodayPriceItems(date: string): Promise<TodayPriceSupplier[]> {
  const qs = date ? `?date=${encodeURIComponent(date)}` : ''
  return apiFetch<TodayPriceSupplier[]>(`/api/product/today_price_items${qs}`)
}

export function saveTodayPrices(items: TodayPriceSaveItem[]): Promise<TodayPriceSaveResp> {
  return apiFetch<TodayPriceSaveResp>('/api/product/today_price_save', {
    method: 'POST',
    body: JSON.stringify({ items }),
  })
}

export function fetchSortItemsBySupplier(date: string): Promise<SortSupplierNode[]> {
  const qs = date ? `?date=${encodeURIComponent(date)}` : ''
  return apiFetch<SortSupplierNode[]>(`/api/sales_order/sort_items_by_supplier${qs}`)
}

/** 批量生成/补全采购订单（单次调用，返回 JSON 含 count/merged/excluded_count/warning/error） */
export function generatePurchaseForOrder(orderId: number, force: boolean): Promise<Record<string, unknown>> {
  return fetch(`/api/sales_order/generate_purchase/${orderId}?force=${force ? 1 : 0}`, {
    method: 'POST',
    credentials: 'same-origin',
  }).then(async (resp) => {
    const ct = resp.headers.get('content-type') || ''
    if (ct.includes('application/json')) {
      return (await resp.json()) as Record<string, unknown>
    }
    return { ok: resp.ok } as Record<string, unknown>
  })
}

/** 导出下载（后端直接返回 xlsx 流，无 force 确认流程） */
export function exportTodayPriceUrl(kind: 'excel' | 'a4' | 'category', date: string, printValues: boolean): string {
  const base = kind === 'a4' ? '/api/product/today_price_a4' : kind === 'category' ? '/api/product/today_price_excel_by_category' : '/api/product/today_price_excel'
  const params: string[] = []
  if (date) params.push(`date=${encodeURIComponent(date)}`)
  if (printValues) params.push('print_values=1')
  return params.length ? `${base}?${params.join('&')}` : base
}
