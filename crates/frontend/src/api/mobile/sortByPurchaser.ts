// 对应老页函数 page_mobile_sort_by_purchaser（原路由 /mobile/sort_by_purchaser）
import { apiFetch } from '../client'

/** 按单位分拣 - 商品明细（字段以老页 JS 实际读取的为准） */
export interface SortByPurchaserItem {
  id: number
  product_name: string
  unit: string
  remark: string
  quantity: number
}

/** 按单位分拣 - 采购单位分组 */
export interface SortByPurchaserGroup {
  purchaser_name: string
  items: SortByPurchaserItem[]
}

/** 数量修正条目（POST /api/sales_order/correction 请求体元素） */
export interface SortCorrectionEntry {
  id: number
  quantity: number
}

/** 拉取按单位分拣数据（GET 返回 JSON）；date 为空时后端返回当前待分拣 */
export function fetchSortItemsByPurchaser(date: string): Promise<SortByPurchaserGroup[]> {
  const qs = date ? `?date=${encodeURIComponent(date)}` : ''
  return apiFetch<SortByPurchaserGroup[]>(`/api/sales_order/sort_items_by_purchaser${qs}`)
}

/** 保存数量修正到服务器；后端返回纯文本，调用方 alert 展示 */
export function saveSortCorrections(corrections: SortCorrectionEntry[]): Promise<string> {
  return fetch('/api/sales_order/correction', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ corrections }),
  }).then((resp) => resp.text())
}

/** 导出 XLSX 下载地址（后端直接返回文件流，赋值给 window.location.href） */
export function exportSortByPurchaserUrl(date: string): string {
  const base = '/api/sales_order/sort_items_by_purchaser_excel'
  return date ? `${base}?date=${encodeURIComponent(date)}` : base
}
