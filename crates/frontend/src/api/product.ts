import { apiFetch } from './client'

export interface ProductUnit {
  id?: number
  unit_name: string
  ratio: number
  unit_price: number
  purchase_price: number
  sort_order: number
}

export interface ProductPrice {
  price_type: string
  price: number
}

export interface Product {
  id: number
  name: string
  spec: string | null
  alias1: string | null
  alias2: string | null
  unit: string
  base_unit: string
  base_price: number
  purchase_price: number
  max_purchase_price: number
  min_purchase_price: number
  markup_rate: number
  auto_update_price: number
  image_url: string | null
  category_id: number | null
  status: number
  audit_status: string | null
  shelf_life: string | null
  gov_status: number
  category_name: string | null
  units: ProductUnit[]
  prices: ProductPrice[]
  selling_price: number
}

export interface ProductListResp {
  data: Product[]
  page: number
  page_size: number
  total: number
  total_pages: number
}

export interface Category {
  id: number
  name: string
  parent_id: number | null
  entity_type: string
  sort_order: number
}

export interface CreateProductInput {
  name: string
  spec: string | null
  unit: string | null
  base_unit: string | null
  base_price: number | null
  purchase_price: number | null
  category_id: number | null
  shelf_life: string | null
  gov_status?: number
}

export interface UpdateProductInput extends CreateProductInput {
  id: number
  alias1: string | null
  alias2: string | null
  image_url: string | null
  markup_rate: number
  auto_update_price: number
  gov_status: number
}

export interface ProductListQuery {
  category_id: number | null
  keyword: string
  gov_status: number | null
  page: number
  page_size: number
}

export async function listProducts(q: ProductListQuery): Promise<ProductListResp> {
  const params = new URLSearchParams()
  if (q.category_id) params.set('category_id', String(q.category_id))
  if (q.keyword) params.set('keyword', q.keyword)
  if (q.gov_status !== null) params.set('gov_status', String(q.gov_status))
  params.set('page', String(q.page))
  params.set('page_size', String(q.page_size))
  params.set('thumbs', '1')
  return apiFetch<ProductListResp>(`/api/product/list?${params.toString()}`)
}

export function listCategories(): Promise<Category[]> {
  return apiFetch<Category[]>('/api/category/list')
}

export interface BatchPricesResp {
  success: boolean
  date: string
  prices: Record<string, Record<string, number>>
}

export function lookupBatchPrices(date: string, productIds: number[]): Promise<BatchPricesResp> {
  return apiFetch<BatchPricesResp>('/api/price_schedule/lookup_batch', {
    method: 'POST',
    body: JSON.stringify({ date, product_ids: productIds }),
  })
}

export function checkProductName(name: string): Promise<Product[]> {
  return apiFetch<Product[]>(`/api/product/check_name?name=${encodeURIComponent(name)}`)
}

/** POST JSON 或无 body；成功返回 2xx，失败时 body 为文本原因 */
export async function postAction(url: string, body?: unknown): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch(url, {
    method: 'POST',
    credentials: 'same-origin',
    headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  return { ok: resp.ok, message: await resp.text() }
}

export async function getAction(url: string): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch(url, { credentials: 'same-origin' })
  return { ok: resp.ok, message: await resp.text() }
}

export function createProduct(input: CreateProductInput) {
  return postAction('/api/product/create', input)
}

export function updateProduct(input: UpdateProductInput) {
  return postAction('/api/product/update', input)
}

export function deleteProduct(id: number) {
  return postAction('/api/product/delete', { id })
}

export function approveProduct(id: number) {
  return postAction('/api/product/approve', { id })
}

export function unapproveProduct(id: number) {
  return postAction('/api/product/unapprove', { id })
}

export function toggleProductStatus(id: number) {
  return postAction(`/api/product/toggle_status/${id}`)
}

export function setAutoUpdatePrice(productId: number, autoUpdatePrice: number) {
  return postAction('/api/product/set_auto_update_price', {
    product_id: productId,
    auto_update_price: autoUpdatePrice,
  })
}

export function batchSetAutoUpdatePrice(autoUpdatePrice: number) {
  return postAction('/api/product/batch_set_auto_update_price', { auto_update_price: autoUpdatePrice })
}

export function deleteProductPrices(productId: number) {
  return postAction('/api/product/price/delete_by_product', { product_id: productId })
}

export function upsertProductPrice(productId: number, priceType: string, price: number) {
  return postAction('/api/product/price/upsert', { product_id: productId, price_type: priceType, price })
}

export function syncBasePrice(productId: number) {
  return postAction('/api/product/sync_base_price', { product_id: productId })
}

export function deleteProductUnits(productId: number) {
  return postAction('/api/product/unit/delete_by_product', { product_id: productId })
}

export function createProductUnit(unit: ProductUnit & { product_id: number }) {
  return postAction('/api/product/unit/create', unit)
}

export async function uploadProductImage(productId: number, file: File): Promise<{ url?: string; error?: string }> {
  const fd = new FormData()
  fd.append('file', file)
  const resp = await fetch(`/api/product/upload_image?product_id=${productId}`, {
    method: 'POST',
    credentials: 'same-origin',
    body: fd,
  })
  return resp.json()
}

export async function applyProductImageUrl(productId: number, url: string): Promise<{ url?: string; error?: string }> {
  const resp = await fetch('/api/product/image/url', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ product_id: productId, url }),
  })
  return resp.json()
}

export function deleteProductImage(productId: number) {
  return getAction(`/api/product/delete_image?product_id=${productId}`)
}

export async function importProducts(file: File): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch('/api/product/import', { method: 'POST', credentials: 'same-origin', body: file })
  return { ok: resp.ok, message: await resp.text() }
}
