import { useState } from 'react'
import { Button, InputNumber, Table } from 'antd'
import { SearchOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2 } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'
import { querySlowStock, type SlowStockItem } from '../../api/query/stock'

export default function SlowStockPage() {
  // 老页面默认 days=30 且进入即自动查询
  const [days, setDays] = useState<number | null>(30)
  const [submittedDays, setSubmittedDays] = useState<number | ''>(30)

  const { data = [], isLoading } = useQuery({
    queryKey: ['slowStock', submittedDays],
    queryFn: () => querySlowStock(submittedDays),
    placeholderData: (prev) => prev,
  })

  const search = () => setSubmittedDays(days ?? '')

  const columns: TableProps<SlowStockItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name', width: 180 },
    { title: '规格', dataIndex: 'spec', width: 120, render: (v: string | null) => v || '' },
    { title: '单位', dataIndex: 'unit', width: 70, render: (v: string | null) => v || '' },
    { title: '当前库存', dataIndex: 'stock_quantity', width: 100, align: 'right' as const, render: (v: number) => fmt2(v) },
    { title: '库存金额', dataIndex: 'stock_amount', width: 110, align: 'right' as const, render: (v: number) => fmt2(v) },
    // 空值展示"从未出库"（照老页面）
    { title: '最后出库日期', dataIndex: 'last_out_date', width: 130, render: (v: string | null) => v || '从未出库' },
    { title: '呆滞天数', dataIndex: 'days', width: 90, align: 'right' as const },
  ]

  return (
    <ReportCard
      title="呆滞库存查询"
      filters={
        <FilterItem label="无出库天数" width={180}>
          <InputNumber
            min={1}
            precision={0}
            value={days}
            addonAfter="天"
            style={{ width: 140 }}
            onChange={(v) => setDays(v)}
          />
        </FilterItem>
      }
      actions={
        <Button type="primary" icon={<SearchOutlined />} onClick={search}>
          查询
        </Button>
      }
    >
      <Table<SlowStockItem>
        rowKey={(_, i) => String(i)}
        size="small"
        loading={isLoading}
        columns={columns}
        dataSource={data}
        scroll={scrollX('max-content')}
        pagination={false}
        locale={{ emptyText: '暂无数据' }}
      />
    </ReportCard>
  )
}
