// 对应老页函数 page_mobile_sort_comprehensive，原路由 /mobile/sort_comprehensive
import { apiFetch } from '../client'

/** 综合分拣明细项（字段名与老页 JS 实际读取的一致） */
export interface SortComprehensiveItem {
  id: number
  product_id: number
  product_name: string
  unit: string
  unit_price: number
  quantity: number
  amount: number
  order_nos: string[]
  remarks: string[]
}

export interface SortComprehensiveCategory {
  category_id: number
  category_name: string
  items: SortComprehensiveItem[]
}

export interface SortComprehensivePurchaser {
  purchaser_id: number
  purchaser_name: string
  total_amount: number
  categories: SortComprehensiveCategory[]
}

/** 提交给 /api/sales_order/correction 的修正条目 */
export interface CorrectionEntry {
  id: number
  quantity: number
}

/** 综合分拣清单：date 为空返回当前待分拣，否则返回指定日期历史分拣 */
export function fetchSortComprehensive(date: string): Promise<SortComprehensivePurchaser[]> {
  const qs = date ? `?date=${encodeURIComponent(date)}` : ''
  return apiFetch<SortComprehensivePurchaser[]>(`/api/sales_order/sort_comprehensive${qs}`)
}

/** 导出 XLSX 下载地址（后端直接返回文件流，前端 window.location.href 跳转） */
export function sortComprehensiveExcelUrl(date: string): string {
  const base = '/api/sales_order/sort_comprehensive_excel'
  return date ? `${base}?date=${encodeURIComponent(date)}` : base
}

/** 保存数量修正：后端返回纯文本，由调用方 alert 展示 */
export async function saveCorrections(corrections: CorrectionEntry[]): Promise<string> {
  const resp = await fetch('/api/sales_order/correction', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ corrections }),
  })
  return resp.text()
}
