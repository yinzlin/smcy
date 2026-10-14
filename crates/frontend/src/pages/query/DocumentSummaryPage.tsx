import { useState } from 'react'
import { Button, DatePicker, Table } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2 } from '../../utils/format'
import {
  DOCUMENT_SUMMARY_EXPORT_URL,
  fetchDocumentSummary,
  type DocumentSummaryRow,
} from '../../api/query/finance'

const BG_SUCCESS = '#198754'

export default function DocumentSummaryPage() {
  const [month, setMonth] = useState('')
  // 老页面初始不查询，点「查询」才发请求
  const [appliedMonth, setAppliedMonth] = useState<string | null>(null)

  const { data, isFetching } = useQuery({
    queryKey: ['documentSummary', appliedMonth],
    queryFn: () => fetchDocumentSummary(appliedMonth!),
    enabled: appliedMonth !== null,
  })

  const search = () => setAppliedMonth(month)

  const columns: TableProps<DocumentSummaryRow>['columns'] = [
    { title: '月份', dataIndex: 'month' },
    { title: '采购订单数', dataIndex: 'purchase_count', align: 'right' },
    { title: '销售订单数', dataIndex: 'sales_count', align: 'right' },
    { title: '采购金额', dataIndex: 'purchase_amount', align: 'right', render: (v: number) => fmt2(v) },
    { title: '销售金额', dataIndex: 'sales_amount', align: 'right', render: (v: number) => fmt2(v) },
  ]

  return (
    <ReportCard
      title="单据汇总查询"
      filters={
        <FilterItem label="月份" width={220}>
          <DatePicker picker="month" style={{ width: '100%' }} value={month ? dayjs(month) : null} onChange={(d) => setMonth(d ? d.format('YYYY-MM') : '')} />
        </FilterItem>
      }
      actions={
        <>
          <Button type="primary" onClick={search} loading={isFetching}>查询</Button>
          <Button style={{ background: BG_SUCCESS, color: '#fff' }} href={DOCUMENT_SUMMARY_EXPORT_URL}>导出Excel</Button>
        </>
      }
    >
      <Table<DocumentSummaryRow>
        rowKey="month"
        size="small"
        bordered
        loading={isFetching}
        columns={columns}
        dataSource={data ?? []}
        pagination={false}
      />
    </ReportCard>
  )
}
