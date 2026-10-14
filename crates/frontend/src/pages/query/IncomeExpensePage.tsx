import { useState } from 'react'
import { Button, DatePicker, Select, Table } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2, fmtMoney } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'
import {
  INCOME_EXPENSE_EXPORT_URL,
  fetchIncomeExpense,
  type IncomeExpenseQuery,
  type IncomeExpenseRecord,
} from '../../api/query/finance'

/** Bootstrap 色板：bg-success / bg-danger */
const BG_SUCCESS = '#198754'
const BG_DANGER = '#dc3545'

export default function IncomeExpensePage() {
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [type, setType] = useState('')
  // 老页面初始不查询，点「查询」才发请求
  const [applied, setApplied] = useState<IncomeExpenseQuery | null>(null)

  const { data, isFetching } = useQuery({
    queryKey: ['incomeExpense', applied],
    queryFn: () => fetchIncomeExpense(applied!),
    enabled: applied !== null,
  })

  const search = () => setApplied({ start_date: startDate, end_date: endDate, type })

  const columns: TableProps<IncomeExpenseRecord>['columns'] = [
    { title: '日期', dataIndex: 'date' },
    { title: '类型', dataIndex: 'type' },
    { title: '摘要', dataIndex: 'description' },
    { title: '金额', dataIndex: 'amount', align: 'right', render: (v: number) => fmt2(v) },
    { title: '账户', dataIndex: 'account', render: (v: string | null) => v ?? '' },
    { title: '备注', dataIndex: 'remark', render: (v: string | null) => v ?? '' },
  ]

  return (
    <ReportCard
      title="收支流水查询"
      filters={
        <>
          <FilterItem label="开始日期" width={220}>
            <DatePicker style={{ width: '100%' }} value={startDate ? dayjs(startDate) : null} onChange={(d) => setStartDate(d ? d.format('YYYY-MM-DD') : '')} />
          </FilterItem>
          <FilterItem label="结束日期" width={220}>
            <DatePicker style={{ width: '100%' }} value={endDate ? dayjs(endDate) : null} onChange={(d) => setEndDate(d ? d.format('YYYY-MM-DD') : '')} />
          </FilterItem>
          <FilterItem label="类型" width={220}>
            <Select
              style={{ width: '100%' }}
              value={type}
              onChange={setType}
              options={[
                { value: '', label: '全部' },
                { value: '收入', label: '收入' },
                { value: '支出', label: '支出' },
              ]}
            />
          </FilterItem>
        </>
      }
      actions={
        <>
          <Button type="primary" onClick={search} loading={isFetching}>查询</Button>
          <Button style={{ background: BG_SUCCESS, color: '#fff' }} href={INCOME_EXPENSE_EXPORT_URL}>导出Excel</Button>
        </>
      }
    >
      {/* 汇总卡片：总收入（绿）/ 总支出（红），初始 ¥0.00 */}
      <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap', marginBottom: 16 }}>
        <div style={{ flex: '1 1 320px', background: BG_SUCCESS, color: '#fff', borderRadius: 8, padding: '16px 24px' }}>
          <div style={{ fontSize: 16, fontWeight: 600 }}>总收入</div>
          <div style={{ fontSize: 26, fontWeight: 600 }}>{fmtMoney(data?.total_income)}</div>
        </div>
        <div style={{ flex: '1 1 320px', background: BG_DANGER, color: '#fff', borderRadius: 8, padding: '16px 24px' }}>
          <div style={{ fontSize: 16, fontWeight: 600 }}>总支出</div>
          <div style={{ fontSize: 26, fontWeight: 600 }}>{fmtMoney(data?.total_expense)}</div>
        </div>
      </div>
      <Table<IncomeExpenseRecord>
        rowKey={(_, i) => String(i)}
        size="small"
        bordered
        loading={isFetching}
        columns={columns}
        dataSource={data?.records ?? []}
        scroll={scrollX('max-content')}
        pagination={false}
      />
    </ReportCard>
  )
}
