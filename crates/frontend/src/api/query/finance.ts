import { apiFetch } from '../client'

// ===== 收支流水查询 /api/query/income_expense =====

export interface IncomeExpenseRecord {
  date: string
  type: string
  description: string
  amount: number
  account: string | null
  remark: string | null
}

export interface IncomeExpenseResp {
  total_income: number
  total_expense: number
  records: IncomeExpenseRecord[]
}

export interface IncomeExpenseQuery {
  start_date: string
  end_date: string
  type: string
}

/** 老页面 JS 直接拼接三个参数（为空也带空值），此处保持一致 */
export async function fetchIncomeExpense(q: IncomeExpenseQuery): Promise<IncomeExpenseResp> {
  const params = new URLSearchParams()
  params.set('start_date', q.start_date)
  params.set('end_date', q.end_date)
  params.set('type', q.type)
  return apiFetch<IncomeExpenseResp>(`/api/query/income_expense?${params.toString()}`)
}

export const INCOME_EXPENSE_EXPORT_URL = '/api/query/income_expense/export'

// ===== 毛利明细查询 /api/query/profit_detail =====

export interface ProfitDetailRow {
  order_no: string
  purchaser_name: string
  order_date: string
  sales_amount: number
  cost_amount: number
}

export interface ProfitDetailQuery {
  start_date: string
  end_date: string
}

export async function fetchProfitDetail(q: ProfitDetailQuery): Promise<ProfitDetailRow[]> {
  const params = new URLSearchParams()
  params.set('start_date', q.start_date)
  params.set('end_date', q.end_date)
  return apiFetch<ProfitDetailRow[]>(`/api/query/profit_detail?${params.toString()}`)
}

export const PROFIT_DETAIL_EXPORT_URL = '/api/query/profit_detail/export'

// ===== 品类进销存统计 /api/query/category_stats =====

export interface CategoryStatsRow {
  category_name: string
  purchase_quantity: number
  purchase_amount: number
  sales_quantity: number
  sales_amount: number
  stock_quantity: number
  stock_amount: number
}

export interface CategoryStatsQuery {
  start_date: string
  end_date: string
}

export async function fetchCategoryStats(q: CategoryStatsQuery): Promise<CategoryStatsRow[]> {
  const params = new URLSearchParams()
  params.set('start_date', q.start_date)
  params.set('end_date', q.end_date)
  return apiFetch<CategoryStatsRow[]>(`/api/query/category_stats?${params.toString()}`)
}

export const CATEGORY_STATS_EXPORT_URL = '/api/query/category_stats/export'

// ===== 单据汇总查询 /api/query/document_summary =====

export interface DocumentSummaryRow {
  month: string
  purchase_count: number
  sales_count: number
  purchase_amount: number
  sales_amount: number
}

export async function fetchDocumentSummary(month: string): Promise<DocumentSummaryRow[]> {
  const params = new URLSearchParams()
  params.set('month', month)
  return apiFetch<DocumentSummaryRow[]>(`/api/query/document_summary?${params.toString()}`)
}

export const DOCUMENT_SUMMARY_EXPORT_URL = '/api/query/document_summary/export'

// ===== 进销存汇总报表 /api/query/overview =====

export interface OverviewSummaryRow {
  name: string
  amount: number
  quantity: number
}

export interface OverviewResp {
  purchase_total: number
  sales_total: number
  stock_total: number
  profit_total: number
  purchase_by_supplier: OverviewSummaryRow[]
  sales_by_purchaser: OverviewSummaryRow[]
}

export async function fetchOverview(month: string): Promise<OverviewResp> {
  const params = new URLSearchParams()
  params.set('month', month)
  return apiFetch<OverviewResp>(`/api/query/overview?${params.toString()}`)
}

export const OVERVIEW_EXPORT_URL = '/api/query/overview/export'

// ===== 财务结算查询 /api/query/finance_settlement =====

export type FinanceOrderType = 'purchase' | 'sales'

export interface FinanceSettlementRow {
  order_no: string
  order_date: string
  order_type: FinanceOrderType
  party_name: string
  total_amount: number
  discount_amount: number
  amount_reduction: number
  final_amount: number
  paid_amount: number
  unpaid_amount: number
  is_settled: number
}

export interface FinanceSettlementSummary {
  total_amount: number
  discount_amount: number
  reduction_amount: number
  paid_amount: number
  unpaid_amount: number
}

export interface FinanceSettlementResp {
  data: FinanceSettlementRow[]
  summary: FinanceSettlementSummary
  page: number
  page_size: number
  total: number
  total_pages: number
}

export interface FinanceSettlementQuery {
  start_date: string
  end_date: string
  type: string
  is_settled: string
  keyword: string
  page: number
  page_size: number
}

/** 老页面 JS：空参数不拼接，page/page_size 必传 */
export async function fetchFinanceSettlement(q: FinanceSettlementQuery): Promise<FinanceSettlementResp> {
  const params = new URLSearchParams()
  if (q.start_date) params.set('start_date', q.start_date)
  if (q.end_date) params.set('end_date', q.end_date)
  if (q.type) params.set('type', q.type)
  if (q.is_settled) params.set('is_settled', q.is_settled)
  if (q.keyword) params.set('keyword', q.keyword)
  params.set('page', String(q.page))
  params.set('page_size', String(q.page_size))
  return apiFetch<FinanceSettlementResp>(`/api/query/finance_settlement?${params.toString()}`)
}
