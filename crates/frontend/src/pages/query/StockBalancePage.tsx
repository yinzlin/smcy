import { useState } from 'react'
import { App, Button, DatePicker, Input, Modal, Select, Table } from 'antd'
import { DownloadOutlined, SearchOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import dayjs from 'dayjs'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2 } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'
import { listCategories } from '../../api/product'
import {
  fetchAllStockFlow,
  queryStockBalance,
  stockBalanceExportUrl,
  type StockBalanceItem,
  type StockBalanceQuery,
  type StockFlowItem,
} from '../../api/query/stock'

const PAGE_SIZE = 20

export default function StockBalancePage() {
  const { message } = App.useApp()

  // 筛选草稿与已提交快照：进入页面不自动加载，点「查询」后才请求（照老页面行为）
  const [productName, setProductName] = useState('')
  const [categoryId, setCategoryId] = useState('')
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [submitted, setSubmitted] = useState<
    Pick<StockBalanceQuery, 'productName' | 'categoryId' | 'startDate' | 'endDate'> | null
  >(null)
  const [page, setPage] = useState(1)
  const [flowProductId, setFlowProductId] = useState<number | null>(null)

  const { data: categories = [] } = useQuery({ queryKey: ['categories'], queryFn: listCategories })

  const { data, isLoading } = useQuery({
    queryKey: ['stockBalance', submitted, page],
    queryFn: () => queryStockBalance({ ...submitted!, page, pageSize: PAGE_SIZE }),
    enabled: submitted !== null,
    placeholderData: (prev) => prev,
  })

  // 老页面：mode=history 时按单据日期区间展示收发存，否则为实时余额（动态表头）
  const mode = data?.mode === 'history' ? 'history' : 'realtime'

  // 日期校验文案照抄老页面
  const validateDates = (): boolean => {
    if ((startDate && !endDate) || (!startDate && endDate)) {
      message.warning('开始日期和截止日期需同时填写，或都不填')
      return false
    }
    if (startDate && endDate && startDate > endDate) {
      message.warning('开始日期不能晚于截止日期')
      return false
    }
    return true
  }

  const search = () => {
    if (!validateDates()) return
    setSubmitted({ productName, categoryId, startDate, endDate })
    setPage(1)
  }

  const exportExcel = () => {
    if (!validateDates()) return
    window.location.href = stockBalanceExportUrl({ productName, categoryId, startDate, endDate })
  }

  const columns: TableProps<StockBalanceItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name', width: 180 },
    { title: '规格', dataIndex: 'spec', width: 120, render: (v: string | null) => v || '' },
    { title: '单位', dataIndex: 'unit', width: 70, render: (v: string | null) => v || '' },
    ...(mode === 'history'
      ? [
          { title: '期初数量', dataIndex: 'opening_qty', width: 100, align: 'right' as const, render: (v: number | undefined) => fmt2(v) },
          { title: '本期入库', dataIndex: 'period_in', width: 100, align: 'right' as const, render: (v: number | undefined) => fmt2(v) },
          { title: '本期出库', dataIndex: 'period_out', width: 100, align: 'right' as const, render: (v: number | undefined) => fmt2(v) },
          { title: '期末数量', dataIndex: 'closing_qty', width: 100, align: 'right' as const, render: (v: number | undefined) => fmt2(v) },
          { title: '期末金额', dataIndex: 'amount', width: 110, align: 'right' as const, render: (v: number | undefined) => fmt2(v) },
        ]
      : [
          { title: '库存数量', dataIndex: 'quantity', width: 100, align: 'right' as const, render: (v: number | undefined) => fmt2(v) },
          { title: '库存金额', dataIndex: 'amount', width: 110, align: 'right' as const, render: (v: number | undefined) => fmt2(v) },
        ]),
    {
      title: '操作',
      key: 'action',
      width: 100,
      render: (_v, o) => (
        <Button size="small" onClick={() => setFlowProductId(o.product_id)}>
          查看流水
        </Button>
      ),
    },
  ]

  // 查看流水：Modal 简表替代老页面的 priceAlert 文本弹窗
  const { data: flowItems = [], isFetching: flowLoading } = useQuery({
    queryKey: ['stockBalanceFlow', flowProductId],
    queryFn: () => fetchAllStockFlow(flowProductId!),
    enabled: flowProductId !== null,
  })

  const flowColumns: TableProps<StockFlowItem>['columns'] = [
    { title: '类型', dataIndex: 'movement_type', width: 140 },
    {
      title: '数量',
      width: 140,
      align: 'right' as const,
      render: (_v, o) => {
        // 入库带 + 号、出库带 - 号，后拼单位（照老页面 viewFlow 口径）
        const inQty = o.in_quantity || 0
        const qty = inQty > 0 ? '+' + fmt2(inQty) : '-' + fmt2(o.out_quantity)
        return qty + (o.unit || '')
      },
    },
    { title: '时间', dataIndex: 'create_time' },
  ]

  return (
    <>
      <ReportCard
        title="实时库存余额查询"
        filters={
          <>
            <FilterItem label="商品名称" width={220}>
              <Input
                value={productName}
                placeholder="输入商品名称搜索"
                onChange={(e) => setProductName(e.target.value)}
                onPressEnter={search}
              />
            </FilterItem>
            <FilterItem label="分类" width={180}>
              <Select
                style={{ width: '100%' }}
                value={categoryId}
                onChange={setCategoryId}
                options={[
                  { value: '', label: '全部分类' },
                  ...categories.map((c) => ({ value: String(c.id), label: c.name })),
                ]}
              />
            </FilterItem>
            <FilterItem label="开始日期">
              <DatePicker
                value={startDate ? dayjs(startDate) : null}
                onChange={(d) => setStartDate(d ? d.format('YYYY-MM-DD') : '')}
              />
            </FilterItem>
            <FilterItem label="截止日期">
              <DatePicker
                value={endDate ? dayjs(endDate) : null}
                onChange={(d) => setEndDate(d ? d.format('YYYY-MM-DD') : '')}
              />
            </FilterItem>
          </>
        }
        actions={
          <>
            <Button type="primary" icon={<SearchOutlined />} onClick={search}>
              查询
            </Button>
            <Button
              icon={<DownloadOutlined />}
              onClick={exportExcel}
              style={{ background: '#52c41a', borderColor: '#52c41a', color: '#fff' }}
            >
              导出Excel
            </Button>
          </>
        }
        desc={`不填日期：查询当前实时库存余额；填写起止日期：按单据日期查询该时间段的收发存（期初数量、本期入库、本期出库、期末数量），与实际审核/验收时间无关。进入页面不自动加载，请点击"查询"。`}
      >
        <Table<StockBalanceItem>
          rowKey="product_id"
          size="small"
          loading={isLoading}
          columns={columns}
          dataSource={data?.items ?? []}
          scroll={scrollX('max-content')}
          locale={{ emptyText: submitted ? '没有符合条件的记录' : '请设置查询条件后点击"查询"' }}
          pagination={{
            current: page,
            pageSize: PAGE_SIZE,
            total: data?.total ?? 0,
            showSizeChanger: false,
            showTotal: (t) => `共 ${t} 条，第 ${page} / ${Math.max(1, Math.ceil(t / PAGE_SIZE))} 页`,
            onChange: (p) => setPage(p),
          }}
        />
      </ReportCard>

      <Modal
        title={`库存流水(共${flowItems.length}条)`}
        open={flowProductId !== null}
        footer={null}
        onCancel={() => setFlowProductId(null)}
        width={640}
      >
        <Table<StockFlowItem>
          rowKey={(_, i) => String(i)}
          size="small"
          loading={flowLoading}
          columns={flowColumns}
          dataSource={flowItems}
          scroll={scrollX('max-content')}
          pagination={false}
        />
      </Modal>
    </>
  )
}
