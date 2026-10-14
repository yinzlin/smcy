import { apiFetch } from '../client'
import { postAction } from '../product'

/** /api/category/tree 返回的嵌套分类节点 */
export interface CategoryTreeNode {
  id: number
  name: string
  parent_id: number | null
  sort_order: number
  children: CategoryTreeNode[]
}

export interface Supplier {
  id: number
  name: string
  contact: string | null
  phone: string | null
  address: string | null
  business_scope: string | null
  remark: string | null
  category_id: number | null
  category_name: string | null
  audit_status: string | null
  is_designated: number
  designated_no: string | null
  legal_person: string | null
  credit_code: string | null
  license_no: string | null
  license_expire: string | null
  contract_no: string | null
  contract_start: string | null
  contract_end: string | null
}

export interface Purchaser {
  id: number
  name: string
  contact: string | null
  phone: string | null
  address: string | null
  business_scope: string | null
  remark: string | null
  category_id: number | null
  category_name: string | null
  audit_status: string | null
}

export interface Warehouse {
  id: number
  name: string
  code: string | null
  address: string | null
  contact: string | null
  phone: string | null
  status: number
  sort_order: number
  audit_status: string | null
  create_at: string | null
  update_at: string | null
}

export interface CreateSupplierInput {
  name: string
  contact: string | null
  phone: string | null
  address: string | null
  business_scope: string | null
  remark: string | null
  category_id: number | null
  is_designated: number
  designated_no: string | null
}

export interface UpdateSupplierInput extends CreateSupplierInput {
  id: number
  legal_person: string | null
  credit_code: string | null
  license_no: string | null
  license_expire: string | null
  contract_no: string | null
  contract_start: string | null
  contract_end: string | null
}

export interface CreatePurchaserInput {
  name: string
  contact: string | null
  phone: string | null
  address: string | null
  business_scope: string | null
  remark: string | null
  category_id: number | null
}

export interface UpdatePurchaserInput extends CreatePurchaserInput {
  id: number
}

export interface SaveWarehouseInput {
  name: string
  code: string | null
  contact: string | null
  phone: string | null
  address: string | null
  status: number
  sort_order: number
}

export interface CreateCategoryInput {
  name: string
  parent_id: number | null
  entity_type: string
  sort_order: number
}

function listQueryString(categoryId: number | null, keyword: string): string {
  const params: string[] = []
  if (categoryId) params.push(`category_id=${categoryId}`)
  if (keyword) params.push(`keyword=${encodeURIComponent(keyword)}`)
  return params.length > 0 ? `?${params.join('&')}` : ''
}

// ---------------- 供应商 ----------------

export function listSuppliers(categoryId: number | null, keyword: string): Promise<Supplier[]> {
  return apiFetch<Supplier[]>(`/api/supplier/list${listQueryString(categoryId, keyword)}`)
}

export function createSupplier(input: CreateSupplierInput) {
  return postAction('/api/supplier/create', input)
}

export function updateSupplier(input: UpdateSupplierInput) {
  return postAction('/api/supplier/update', input)
}

export function deleteSupplier(id: number) {
  return postAction('/api/supplier/delete', { id })
}

export function approveSupplier(id: number) {
  return postAction('/api/supplier/approve', { id })
}

export function unapproveSupplier(id: number) {
  return postAction('/api/supplier/unapprove', { id })
}

export const SUPPLIER_EXPORT_URL = '/api/supplier/export'

export async function importSupplier(file: File): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch('/api/supplier/import', { method: 'POST', credentials: 'same-origin', body: file })
  return { ok: resp.ok, message: await resp.text() }
}

// ---------------- 采购方 ----------------

export function listPurchasers(categoryId: number | null, keyword: string): Promise<Purchaser[]> {
  return apiFetch<Purchaser[]>(`/api/purchaser/list${listQueryString(categoryId, keyword)}`)
}

export function createPurchaser(input: CreatePurchaserInput) {
  return postAction('/api/purchaser/create', input)
}

export function updatePurchaser(input: UpdatePurchaserInput) {
  return postAction('/api/purchaser/update', input)
}

export function deletePurchaser(id: number) {
  return postAction('/api/purchaser/delete', { id })
}

export function approvePurchaser(id: number) {
  return postAction('/api/purchaser/approve', { id })
}

export function unapprovePurchaser(id: number) {
  return postAction('/api/purchaser/unapprove', { id })
}

export const PURCHASER_EXPORT_URL = '/api/purchaser/export'

export async function importPurchaser(file: File): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch('/api/purchaser/import', { method: 'POST', credentials: 'same-origin', body: file })
  return { ok: resp.ok, message: await resp.text() }
}

// ---------------- 仓库 ----------------

export function listWarehouses(): Promise<Warehouse[]> {
  return apiFetch<Warehouse[]>('/api/warehouse/list')
}

export function createWarehouse(input: SaveWarehouseInput) {
  return postAction('/api/warehouse/create', input)
}

export function updateWarehouse(input: SaveWarehouseInput & { id: number }) {
  return postAction('/api/warehouse/update', input)
}

export function deleteWarehouse(id: number) {
  return postAction('/api/warehouse/delete', { id })
}

export function approveWarehouse(id: number) {
  return postAction('/api/warehouse/approve', { id })
}

export function unapproveWarehouse(id: number) {
  return postAction('/api/warehouse/unapprove', { id })
}

// ---------------- 分类（供应商/采购方共用） ----------------

export function listCategoryTree(entityType: string): Promise<CategoryTreeNode[]> {
  return apiFetch<CategoryTreeNode[]>(`/api/category/tree?entity_type=${encodeURIComponent(entityType)}`)
}

export function createCategory(input: CreateCategoryInput) {
  return postAction('/api/category/create', input)
}

export function renameCategory(id: number, name: string) {
  return postAction('/api/category/rename', { id, name })
}

export function deleteCategory(id: number) {
  return postAction('/api/category/delete', { id })
}
