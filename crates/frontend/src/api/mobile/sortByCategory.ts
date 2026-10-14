// 对应老页函数 page_mobile_sort_by_category，原路由 /mobile/sort_by_category
import { apiFetch } from '../client'

/** 商品明细（字段名以老页 JS 实际读取为准，后端另返回 product_id/unit_price/amount/order_no 等，本页未使用） */
export interface SortCategoryItem {
  item_id: number
  product_name: string
  unit: string
  quantity: number
  remark?: string
}

export interface SortCategoryPurchaser {
  purchaser_name: string
  items: SortCategoryItem[]
  total_quantity: number
}

export interface SortCategory {
  category_name: string
  purchasers?: SortCategoryPurchaser[]
}

export interface SortCorrectionEntry {
  id: number
  quantity: number
}

/** 按分类分拣数据；date 为空时后端返回当前待分拣（pending/sorting） */
export function fetchSortItemsByCategory(date: string): Promise<SortCategory[]> {
  const qs = date ? `?date=${encodeURIComponent(date)}` : ''
  return apiFetch<SortCategory[]>(`/api/sales_order/sort_items_by_category${qs}`)
}

/** 导出 XLSX 下载地址（后端直接返回文件流） */
export function exportSortByCategoryUrl(date: string): string {
  const base = '/api/sales_order/sort_items_by_category_excel'
  return date ? `${base}?date=${encodeURIComponent(date)}` : base
}

/** 保存数量修正；后端返回纯文本结果，由页面 alert 展示 */
export async function saveSortCorrections(corrections: SortCorrectionEntry[]): Promise<string> {
  const resp = await fetch('/api/sales_order/correction', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ corrections }),
  })
  return resp.text()
}
