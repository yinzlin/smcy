import { useState } from 'react'
import { Button, Input, Select, Table } from 'antd'
import { DownloadOutlined, SearchOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2 } from '../../utils/format'
import { listSuppliers } from '../../api/purchase'
import { fetchPurchasePrice, purchasePriceExportUrl, type PurchasePriceQuery, type PurchasePriceRow } from '../../api/query/prices'
import { useAuth } from '../../stores/auth'

const PAGE_SIZE = 20

export default function PurchasePricePage() {
  const { user } = useAuth()
  // 单价/金额为进价信息，仅管理员角色可见（老页面通过 /api/login/check 判断后隐藏列）
  const canSeePrice = user?.role === 'super_admin' || user?.role === 'admin'

  // 筛选草稿（输入中的值）与已提交快照（点「查询」后才真正请求，老页面不自动查询）
  const [productName, setProductName] = useState('')
  const [supplierId, setSupplierId] = useState<number | null>(null)
  const [submitted, setSubmitted] = useState<Pick<PurchasePriceQuery, 'product_name' | 'supplier_id'> | null>(null)
  const [page, setPage] = useState(1)

  const { data: suppliers = [] } = useQuery({ queryKey: ['suppliers'], queryFn: listSuppliers })

  const { data: resp, isLoading } = useQuery({
    queryKey: ['purchasePrice', submitted, page],
    queryFn: () => fetchPurchasePrice({ ...submitted!, page, page_size: PAGE_SIZE }),
    enabled: submitted !== null,
    placeholderData: (prev) => prev,
  })
  const rows = resp?.data ?? []

  const search = () => {
    setSubmitted({ product_name: productName, supplier_id: supplierId })
    setPage(1)
  }

  const exportUrl = purchasePriceExportUrl(submitted ?? { product_name: productName, supplier_id: supplierId })

  // 列顺序照抄老页面：进货日期 品名规格 单位 单价 数量 金额 生产日期 保质期 供应商 备注
  const columns: TableProps<PurchasePriceRow>['columns'] = [
    { title: '进货日期', dataIndex: 'order_date', width: 110 },
    { title: '品名规格', dataIndex: 'product_name', width: 200 },
    { title: '单位', dataIndex: 'unit', width: 70, render: (v: string | null) => v || '' },
    ...(canSeePrice
      ? [
          { title: '单价', dataIndex: 'unit_price', width: 90, align: 'right' as const, render: (v: number) => fmt2(v) },
          { title: '数量', dataIndex: 'quantity', width: 90, align: 'right' as const, render: (v: number) => fmt2(v) },
          { title: '金额', dataIndex: 'amount', width: 100, align: 'right' as const, render: (v: number) => fmt2(v) },
        ]
      : [{ title: '数量', dataIndex: 'quantity', width: 90, align: 'right' as const, render: (v: number) => fmt2(v) }]),
    { title: '生产日期', dataIndex: 'production_date', width: 110, render: (v: string | null) => v || '' },
    { title: '保质期', dataIndex: 'shelf_life', width: 100, render: (v: string | null) => v || '' },
    { title: '供应商', dataIndex: 'supplier_name', width: 160 },
    { title: '备注', dataIndex: 'remark', render: (v: string | null) => v || '' },
  ]

  const totalPages = resp?.total_pages ?? 1

  return (
    <ReportCard
      title="采购台账查询"
      filters={
        <>
          <FilterItem label="品名规格" width={240}>
            <Input
              value={productName}
              placeholder="输入品名规格"
              allowClear
              onChange={(e) => setProductName(e.target.value)}
              onPressEnter={search}
            />
          </FilterItem>
          <FilterItem label="供应商" width={200}>
            <Select<number | null>
              style={{ width: '100%' }}
              value={supplierId}
              onChange={(v) => setSupplierId(v)}
              options={[{ value: null, label: '全部供应商' }, ...suppliers.map((s) => ({ value: s.id, label: s.name }))]}
            />
          </FilterItem>
        </>
      }
      actions={
        <>
          <Button type="primary" icon={<SearchOutlined />} onClick={search}>
            查询
          </Button>
          <Button icon={<DownloadOutlined />} href={exportUrl} style={{ background: '#52c41a', borderColor: '#52c41a', color: '#fff' }}>
            导出Excel
          </Button>
        </>
      }
    >
      <Table<PurchasePriceRow>
        rowKey={(_, i) => String(i)}
        size="small"
        loading={isLoading}
        columns={columns}
        dataSource={rows}
        pagination={{
          current: page,
          pageSize: PAGE_SIZE,
          total: resp?.total ?? 0,
          showSizeChanger: false,
          showTotal: () => `共 ${resp?.total ?? 0} 条记录，当前第 ${page}/${totalPages} 页`,
          onChange: (p) => setPage(p),
        }}
      />
    </ReportCard>
  )
}
