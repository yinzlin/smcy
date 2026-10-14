import { Button, Table } from 'antd'
import { DownloadOutlined, SearchOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import ReportCard from '../../components/report/ReportCard'
import { fmt2 } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'
import {
  queryStockWarning,
  STOCK_WARNING_EXPORT_URL,
  type StockWarningHighItem,
  type StockWarningLowItem,
} from '../../api/query/stock'

export default function StockWarningPage() {
  // 老页面进入即自动查询
  const { data, isLoading, isFetching, refetch } = useQuery({
    queryKey: ['stockWarning'],
    queryFn: queryStockWarning,
  })

  const lowColumns: TableProps<StockWarningLowItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name', width: 180 },
    { title: '规格', dataIndex: 'spec', width: 120, render: (v: string | null) => v || '' },
    { title: '单位', dataIndex: 'unit', width: 70, render: (v: string | null) => v || '' },
    { title: '当前库存', dataIndex: 'current_stock', width: 100, align: 'right' as const, render: (v: number) => fmt2(v) },
    { title: '最低库存', dataIndex: 'min_stock', width: 100, align: 'right' as const, render: (v: number) => fmt2(v) },
    {
      title: '缺货数量',
      key: 'shortage',
      width: 100,
      align: 'right' as const,
      render: (_v, o) => fmt2(Math.max(0, (o.min_stock || 0) - (o.current_stock || 0))),
    },
  ]

  const highColumns: TableProps<StockWarningHighItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name', width: 180 },
    { title: '规格', dataIndex: 'spec', width: 120, render: (v: string | null) => v || '' },
    { title: '单位', dataIndex: 'unit', width: 70, render: (v: string | null) => v || '' },
    { title: '当前库存', dataIndex: 'current_stock', width: 100, align: 'right' as const, render: (v: number) => fmt2(v) },
    { title: '最高库存', dataIndex: 'max_stock', width: 100, align: 'right' as const, render: (v: number) => fmt2(v) },
    {
      title: '积压数量',
      key: 'overstock',
      width: 100,
      align: 'right' as const,
      render: (_v, o) => fmt2(Math.max(0, (o.current_stock || 0) - (o.max_stock || 0))),
    },
  ]

  return (
    <ReportCard
      title="库存上下限预警"
      actions={
        <>
          <Button type="primary" icon={<SearchOutlined />} loading={isFetching} onClick={() => void refetch()}>
            查询
          </Button>
          <Button
            icon={<DownloadOutlined />}
            href={STOCK_WARNING_EXPORT_URL}
            style={{ background: '#52c41a', borderColor: '#52c41a', color: '#fff' }}
          >
            导出Excel
          </Button>
        </>
      }
    >
      {/* 低于最低库存（缺货）：照老页面 border-danger 红边卡片 */}
      <div style={{ border: '1px solid #ffccc7', borderRadius: 8, padding: '12px 16px' }}>
        <div style={{ fontSize: 15, fontWeight: 600, marginBottom: 8, color: '#cf1322' }}>
          低于最低库存（缺货）
        </div>
        <Table<StockWarningLowItem>
          rowKey={(_, i) => String(i)}
          size="small"
          loading={isLoading}
          columns={lowColumns}
          dataSource={data?.low_stock ?? []}
          scroll={scrollX('max-content')}
          pagination={false}
          locale={{ emptyText: '暂无数据' }}
        />
      </div>

      {/* 高于最高库存（积压）：照老页面 border-warning 黄边卡片 */}
      <div style={{ border: '1px solid #ffe58f', borderRadius: 8, padding: '12px 16px', marginTop: 16 }}>
        <div style={{ fontSize: 15, fontWeight: 600, marginBottom: 8, color: '#d48806' }}>
          高于最高库存（积压）
        </div>
        <Table<StockWarningHighItem>
          rowKey={(_, i) => String(i)}
          size="small"
          columns={highColumns}
          dataSource={data?.high_stock ?? []}
          scroll={scrollX('max-content')}
          pagination={false}
          locale={{ emptyText: '暂无数据' }}
        />
      </div>
    </ReportCard>
  )
}
