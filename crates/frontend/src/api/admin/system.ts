import { apiFetch } from '../client'

// ---------- 库存管理（旧路由 /inventory） ----------

export interface InventoryRow {
  id: number
  product_id: number
  warehouse_id: number
  warehouse_name: string | null
  name: string
  spec: string | null
  quantity: number
  min_stock: number
  max_stock: number
}

export function listInventory(): Promise<InventoryRow[]> {
  return apiFetch<InventoryRow[]>('/api/inventory/list')
}

// ---------- 系统参数（旧路由 /system） ----------

export type SystemConfigMap = Record<string, string>

/**
 * 老页面为服务端渲染时查 system_config 回填表单；
 * 后端目前仅有 POST /api/system/config（保存），无 GET 路由，
 * 请求失败时返回 null，由页面用老页面默认值兜底。
 */
export async function fetchSystemConfig(): Promise<SystemConfigMap | null> {
  try {
    const resp = await fetch('/api/system/config', { credentials: 'same-origin' })
    if (!resp.ok) return null
    return (await resp.json()) as SystemConfigMap
  } catch {
    return null
  }
}

/** 老页面 saveConfig：POST 全部表单字段，成功后端返回文本"设置保存成功" */
export async function saveSystemConfig(data: SystemConfigMap): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch('/api/system/config', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  return { ok: resp.ok, message: await resp.text() }
}

// ---------- 操作日志（旧路由 /system/operation_log） ----------

export interface OperationLogRow {
  id: number
  user_id: number
  username: string
  action: string
  action_label: string
  target_type: string
  target_id: string
  detail: string
  created_at: string
}

export interface OperationLogResp {
  data: OperationLogRow[]
  page: number
  page_size: number
  total: number
  total_pages: number
}

export interface OperationLogQuery {
  page: number
  page_size: number
  username: string
  action: string
  start_date: string
  end_date: string
}

/** URL 构造照抄老页面 buildLogUrl */
export function listOperationLogs(q: OperationLogQuery): Promise<OperationLogResp> {
  const params = new URLSearchParams()
  params.set('page', String(q.page))
  params.set('page_size', String(q.page_size))
  if (q.username) params.set('username', q.username)
  if (q.action) params.set('action', q.action)
  if (q.start_date) params.set('start_date', q.start_date)
  if (q.end_date) params.set('end_date', q.end_date)
  return apiFetch<OperationLogResp>(`/api/system/operation_log?${params.toString()}`)
}

/** 导出 Excel：应用当前筛选条件，下载全量匹配记录（照抄老页面 exportLogs） */
export function operationLogExportUrl(q: Omit<OperationLogQuery, 'page' | 'page_size'>): string {
  const params = new URLSearchParams()
  if (q.username) params.set('username', q.username)
  if (q.action) params.set('action', q.action)
  if (q.start_date) params.set('start_date', q.start_date)
  if (q.end_date) params.set('end_date', q.end_date)
  const qs = params.toString()
  return `/api/system/operation_log/export${qs ? `?${qs}` : ''}`
}

// ---------- 用户管理（旧路由 /user） ----------

export interface AdminUser {
  id: number
  username: string
  nickname: string
  phone: string
  role: string
  status: number
  supplier_id: number
  purchaser_id: number
}

export interface UserDetail {
  id: number
  username: string
  nickname: string
  role: string
  status: number
  supplier_id: number
  purchaser_id: number
}

export interface UserSavePayload {
  username: string
  nickname: string
  password: string
  role: string
  supplier_id: number
  purchaser_id: number
}

export interface UserSaveResp {
  success: boolean
  message: string
}

/** 仅返回启用用户（后端 WHERE status = 1） */
export function listAdminUsers(): Promise<AdminUser[]> {
  return apiFetch<AdminUser[]>('/api/user/list')
}

export function getUserDetail(id: number): Promise<{ success: boolean; message?: string; user?: UserDetail }> {
  return apiFetch<{ success: boolean; message?: string; user?: UserDetail }>(`/api/user/${id}`)
}

export function createUser(payload: UserSavePayload): Promise<UserSaveResp> {
  return apiFetch<UserSaveResp>('/api/user', { method: 'POST', body: JSON.stringify(payload) })
}

export function updateUser(id: number, payload: UserSavePayload): Promise<UserSaveResp> {
  return apiFetch<UserSaveResp>(`/api/user/${id}`, { method: 'PUT', body: JSON.stringify(payload) })
}

export function updateUserStatus(id: number, status: number): Promise<UserSaveResp> {
  return apiFetch<UserSaveResp>(`/api/user/${id}/status`, { method: 'PUT', body: JSON.stringify({ status }) })
}

export function deleteUser(id: number): Promise<UserSaveResp> {
  return apiFetch<UserSaveResp>(`/api/user/${id}`, { method: 'DELETE' })
}

// ---------- 数据备份（旧路由 /backup） / 数据恢复（旧路由 /restore） ----------

export interface BackupRecord {
  id: number
  backup_time: string
  file_name: string
  size: number
}

/**
 * 老页面为服务端渲染 backup_record 列表，后端暂无对应 GET 接口；
 * 预留 GET /api/backup/list（后端补一行路由即可生效），失败时返回空列表。
 */
export async function listBackups(): Promise<BackupRecord[]> {
  try {
    return await apiFetch<BackupRecord[]>('/api/backup/list')
  } catch {
    return []
  }
}

export function backupDownloadUrl(id: number): string {
  return `/api/backup/download/${id}`
}

/** 老页面 doBackup：POST 后 alert 返回文本（如"备份成功，文件大小：X 字节"） */
export async function executeBackup(): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch('/api/backup', { method: 'POST', credentials: 'same-origin' })
  return { ok: resp.ok, message: await resp.text() }
}

export async function deleteBackup(id: number): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch(`/api/backup/delete/${id}`, { method: 'DELETE', credentials: 'same-origin' })
  return { ok: resp.ok, message: await resp.text() }
}

/** 恢复成功后端返回文本，成功后老页面跳转首页 */
export async function restoreBackup(id: number): Promise<{ ok: boolean; message: string }> {
  const resp = await fetch(`/api/restore/${id}`, { method: 'POST', credentials: 'same-origin' })
  return { ok: resp.ok, message: await resp.text() }
}

/** 老页面 restoreFromFile：FormData 上传 .db 文件 */
export async function restoreFromFile(file: File): Promise<{ ok: boolean; message: string }> {
  const formData = new FormData()
  formData.append('file', file)
  const resp = await fetch('/api/restore/file', { method: 'POST', credentials: 'same-origin', body: formData })
  return { ok: resp.ok, message: await resp.text() }
}
