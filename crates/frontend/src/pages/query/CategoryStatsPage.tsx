import { useState } from 'react'
import { Button, DatePicker, Table } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2 } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'
import {
  fetchCategoryStats,
  type CategoryStatsQuery,
  type CategoryStatsRow,
} from '../../api/query/finance'

export default function CategoryStatsPage() {
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  // 老页面初始不查询，点「查询」才发请求（老页面也无导出按钮）
  const [applied, setApplied] = useState<CategoryStatsQuery | null>(null)

  const { data, isFetching } = useQuery({
    queryKey: ['categoryStats', applied],
    queryFn: () => fetchCategoryStats(applied!),
    enabled: applied !== null,
  })

  const search = () => setApplied({ start_date: startDate, end_date: endDate })

  const columns: TableProps<CategoryStatsRow>['columns'] = [
    { title: '品类名称', dataIndex: 'category_name' },
    { title: '采购数量', dataIndex: 'purchase_quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '采购金额', dataIndex: 'purchase_amount', align: 'right', render: (v: number) => fmt2(v) },
    { title: '销售数量', dataIndex: 'sales_quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '销售金额', dataIndex: 'sales_amount', align: 'right', render: (v: number) => fmt2(v) },
    { title: '库存数量', dataIndex: 'stock_quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '库存金额', dataIndex: 'stock_amount', align: 'right', render: (v: number) => fmt2(v) },
    {
      title: '毛利',
      key: 'margin',
      align: 'right',
      render: (_v, o) => fmt2(o.sales_amount - o.purchase_amount),
    },
  ]

  return (
    <ReportCard
      title="品类进销存统计"
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
        <Button type="primary" onClick={search} loading={isFetching}>查询</Button>
      }
    >
      <Table<CategoryStatsRow>
        rowKey="category_name"
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
