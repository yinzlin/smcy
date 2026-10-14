import { useState } from 'react'
import { Button, DatePicker, Table } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2 } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'
import {
  PROFIT_DETAIL_EXPORT_URL,
  fetchProfitDetail,
  type ProfitDetailQuery,
  type ProfitDetailRow,
} from '../../api/query/finance'

const BG_SUCCESS = '#198754'

export default function ProfitDetailPage() {
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  // 老页面初始不查询，点「查询」才发请求
  const [applied, setApplied] = useState<ProfitDetailQuery | null>(null)

  const { data, isFetching } = useQuery({
    queryKey: ['profitDetail', applied],
    queryFn: () => fetchProfitDetail(applied!),
    enabled: applied !== null,
  })

  const search = () => setApplied({ start_date: startDate, end_date: endDate })

  const columns: TableProps<ProfitDetailRow>['columns'] = [
    { title: '订单号', dataIndex: 'order_no' },
    { title: '采购单位', dataIndex: 'purchaser_name' },
    { title: '日期', dataIndex: 'order_date' },
    { title: '销售金额', dataIndex: 'sales_amount', align: 'right', render: (v: number) => fmt2(v) },
    { title: '成本金额', dataIndex: 'cost_amount', align: 'right', render: (v: number) => fmt2(v) },
    {
      title: '毛利',
      key: 'margin',
      align: 'right',
      render: (_v, o) => fmt2(o.sales_amount - o.cost_amount),
    },
    {
      title: '毛利率',
      key: 'margin_rate',
      align: 'right',
      render: (_v, o) => {
        const margin = o.sales_amount - o.cost_amount
        // 老页面口径：sales_amount > 0 时 (毛利/销售额*100).toFixed(1)，否则 '0'，后拼 '%'
        const rate = o.sales_amount > 0 ? (margin / o.sales_amount * 100).toFixed(1) : '0'
        return rate + '%'
      },
    },
  ]

  return (
    <ReportCard
      title="毛利明细查询"
      filters={
        <>
          <FilterItem label="开始日期" width={220}>
            <DatePicker style={{ width: '100%' }} value={startDate ? dayjs(startDate) : null} onChange={(d) => setStartDate(d ? d.format('YYYY-MM-DD') : '')} />
          </FilterItem>
          <FilterItem label="结束日期" width={220}>
            <DatePicker style={{ width: '100%' }} value={endDate ? dayjs(endDate) : null} onChange={(d) => setEndDate(d ? d.format('YYYY-MM-DD') : '')} />
          </FilterItem>
        </>
      }
      actions={
        <>
          <Button type="primary" onClick={search} loading={isFetching}>查询</Button>
          <Button style={{ background: BG_SUCCESS, color: '#fff' }} href={PROFIT_DETAIL_EXPORT_URL}>导出Excel</Button>
        </>
      }
    >
      <Table<ProfitDetailRow>
        rowKey={(_, i) => String(i)}
        size="small"
        bordered
        loading={isFetching}
        columns={columns}
        dataSource={data ?? []}
        scroll={scrollX('max-content')}
        pagination={false}
      />
    </ReportCard>
  )
}
