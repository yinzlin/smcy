// 统筹分拣移动页 API 层（其余分拣页各有独立 API 文件：sortByPurchaser/sortByCategory/sortBySupplier/sortComprehensive）
import { apiFetch } from '../client'

export interface SortItem {
  product_id: number
  product_name: string
  unit: string
  total_quantity: number
  purchaser_names: string
  remarks: string | null
}

export function fetchSortItems(date: string): Promise<SortItem[]> {
  const q = date ? `?date=${encodeURIComponent(date)}` : ''
  return apiFetch<SortItem[]>(`/api/sales_order/sort_items${q}`)
}

export function sortItemsExcelUrl(date: string): string {
  return `/api/sales_order/sort_items_excel${date ? `?date=${encodeURIComponent(date)}` : ''}`
}

/** 数量修正保存（后端返回纯文本，由 useSortSelection 统一提示；供统筹分拣页使用） */
export async function saveCorrections(corrections: { id: number; quantity: number }[]): Promise<string> {
  const res = await fetch('/api/sales_order/correction', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ corrections }),
  })
  return res.text()
}
