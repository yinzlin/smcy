/**
 * 旧版查询页 API 层（/query/purchase_order、/query/purchase_document、/query/sales_order）
 * URL 与参数名照抄老页面 JS 拼接逻辑（含空参数原样拼接）
 */
import { apiFetch } from '../client'
import type { PurchaseOrderDetail } from '../purchase'
import type { SalesOrderDetail } from '../sales'

/** 分页响应：老页面 JS 读取 data/page/page_size/total/total_pages */
export interface PagedResp<T> {
  data: T[]
  page: number
  page_size: number
  total: number
  total_pages: number
}

// ===== 采购订单查询（旧路由 /query/purchase_order）=====

export interface PurchaseOrderQueryRow {
  id: number
  order_no: string
  order_date: string
  total_amount: number
  final_amount: number
  status: string
  remark: string | null
  supplier_name: string
}

export interface PurchaseOrderQueryParams {
  supplier_id: string
  start_date: string
  end_date: string
  status: string
  page: number
}

export function purchaseOrderQueryUrl(p: PurchaseOrderQueryParams): string {
  return (
    '/api/query/purchase_order?supplier_id=' + p.supplier_id +
    '&start_date=' + p.start_date +
    '&end_date=' + p.end_date +
    '&status=' + p.status +
    '&page=' + p.page + '&page_size=20'
  )
}

export function fetchPurchaseOrderQuery(p: PurchaseOrderQueryParams): Promise<PagedResp<PurchaseOrderQueryRow>> {
  return apiFetch<PagedResp<PurchaseOrderQueryRow>>(purchaseOrderQueryUrl(p))
}

/** 老页面导出为固定链接（不带筛选参数） */
export function purchaseOrderQueryExportUrl(): string {
  return '/api/query/purchase_order/export'
}

export function fetchPurchaseOrderDetail(id: number): Promise<PurchaseOrderDetail> {
  return apiFetch<PurchaseOrderDetail>('/api/purchase_order/detail/' + id)
}

// ===== 采购单据列表（旧路由 /query/purchase_document）=====

export interface PurchaseDocumentItem {
  id: number
  supplier_id: number
  supplier_name: string
  document_date: string
  image_url: string
  remark: string | null
  create_at: string
}

/** 老页面拼接逻辑：supplier_id 有值追加 'supplier_id=x&'，document_date 有值追加 'document_date=x' */
export function purchaseDocumentListUrl(supplierId: string, documentDate: string): string {
  let url = '/api/purchase_document/list?'
  if (supplierId) url += 'supplier_id=' + supplierId + '&'
  if (documentDate) url += 'document_date=' + documentDate
  return url
}

export function fetchPurchaseDocuments(supplierId: string, documentDate: string): Promise<PurchaseDocumentItem[]> {
  return apiFetch<PurchaseDocumentItem[]>(purchaseDocumentListUrl(supplierId, documentDate))
}

/** 逐张上传单据图片；成功返回 true（老页面按 res.ok 计数） */
export async function uploadPurchaseDocument(
  file: File,
  supplierId: string,
  supplierName: string,
  documentDate: string,
  remark: string,
): Promise<boolean> {
  const formData = new FormData()
  formData.append('file', file)
  formData.append('supplier_id', supplierId)
  formData.append('supplier_name', supplierName)
  formData.append('document_date', documentDate)
  formData.append('remark', remark)
  const resp = await fetch('/api/purchase_document/upload', {
    method: 'POST',
    credentials: 'same-origin',
    body: formData,
  })
  return resp.ok
}

export async function deletePurchaseDocument(id: number): Promise<boolean> {
  const resp = await fetch('/api/purchase_document/delete/' + id, {
    method: 'DELETE',
    credentials: 'same-origin',
  })
  return resp.ok
}

// ===== 销售订单查询（旧路由 /query/sales_order）=====

export interface SalesOrderQueryRow {
  id: number
  order_no: string
  order_date: string
  total_amount: number
  discount_rate: number
  final_amount: number
  status: string
  remark: string | null
  purchaser_name: string
}

export interface SalesOrderQueryParams {
  purchaser_id: string
  start_date: string
  end_date: string
  status: string
  page: number
  sort_field: string
  sort_order: 'asc' | 'desc'
}

export function salesOrderQueryUrl(p: SalesOrderQueryParams): string {
  let url =
    '/api/query/sales_order?purchaser_id=' + p.purchaser_id +
    '&start_date=' + p.start_date +
    '&end_date=' + p.end_date +
    '&status=' + p.status +
    '&page=' + p.page + '&page_size=20'
  if (p.sort_field) {
    url += '&sort_field=' + p.sort_field + '&sort_order=' + p.sort_order
  }
  return url
}

export function fetchSalesOrderQuery(p: SalesOrderQueryParams): Promise<PagedResp<SalesOrderQueryRow>> {
  return apiFetch<PagedResp<SalesOrderQueryRow>>(salesOrderQueryUrl(p))
}

/** 导出 URL：老页面无条件拼接全部筛选参数（含空值） */
export function salesOrderQueryExportUrl(p: { purchaser_id: string; start_date: string; end_date: string; status: string }): string {
  return (
    '/api/query/sales_order/export?purchaser_id=' + p.purchaser_id +
    '&start_date=' + p.start_date +
    '&end_date=' + p.end_date +
    '&status=' + p.status
  )
}

export function fetchSalesOrderDetail(id: number): Promise<SalesOrderDetail> {
  return apiFetch<SalesOrderDetail>('/api/sales_order/detail/' + id)
}
