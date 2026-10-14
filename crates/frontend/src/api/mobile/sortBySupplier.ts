// 对应老页函数 page_mobile_sort_by_supplier，原路由 /mobile/sort_by_supplier
import { apiFetch } from '../client'

/** 分拣商品行（字段名以老页 JS 实际读取及后端 JSON 为准） */
export interface SortBySupplierItem {
  item_id: number
  order_id: number
  product_id: number
  product_name: string
  unit: string
  unit_price: number
  purchase_price: number
  quantity: number
  amount: number
  remark: string
  order_no: string
}

/** 供应商下的采购单位分组 */
export interface SortBySupplierPurchaser {
  purchaser_name: string
  purchaser_id: number
  items: SortBySupplierItem[]
  total_quantity: number
}

/** 供应商分组（接口顶层数组元素） */
export interface SortBySupplierGroup {
  supplier_name: string
  supplier_id: number
  purchasers?: SortBySupplierPurchaser[]
  total_quantity: number
}

/** 数量修正条目（POST /api/sales_order/correction 请求体元素） */
export interface SortCorrectionEntry {
  id: number
  quantity: number
}

/** GET 按供应商分拣数据；date 为空时后端返回当前待分拣 */
export function fetchSortItemsBySupplier(date: string): Promise<SortBySupplierGroup[]> {
  const qs = date ? `?date=${encodeURIComponent(date)}` : ''
  return apiFetch<SortBySupplierGroup[]>(`/api/sales_order/sort_items_by_supplier${qs}`)
}

/** POST 保存数量修正；后端返回纯文本，调用方 alert 展示 */
export async function saveSortCorrections(corrections: SortCorrectionEntry[]): Promise<string> {
  const resp = await fetch('/api/sales_order/correction', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ corrections }),
  })
  return resp.text()
}

/** 导出 XLSX 下载地址（window.location.href 跳转）；printValues 对应 print_values=1 */
export function exportSortBySupplierExcelUrl(date: string, printValues: boolean): string {
  const base = '/api/sales_order/sort_items_by_supplier_excel'
  const params: string[] = []
  if (date) params.push(`date=${encodeURIComponent(date)}`)
  if (printValues) params.push('print_values=1')
  return params.length ? `${base}?${params.join('&')}` : base
}
