import { useState } from 'react'
import { App, Button, DatePicker, Descriptions, Modal, Select, Space, Spin, Table, Tag } from 'antd'
import type { TableColumnsType, TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import {
  fetchSalesOrderDetail,
  fetchSalesOrderQuery,
  salesOrderQueryExportUrl,
  type SalesOrderQueryRow,
} from '../../api/query/orders'
import { listPurchasers, type SalesOrderDetail } from '../../api/sales'
import { fmt2, fmtMoney } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'

const PAGE_SIZE = 20

/** 老页面 badge 规则：已完成=绿、未发货=灰、部分发货=橙、取消=红、其他=蓝(info) */
function statusTagColor(status: string): string {
  if (status === '已完成') return 'green'
  if (status === '未发货') return 'default'
  if (status === '部分发货') return 'orange'
  if (status === '取消') return 'red'
  return 'cyan'
}

interface AppliedFilters {
  purchaser_id: string
  start_date: string
  end_date: string
  status: string
}

export default function SalesOrderQueryPage() {
  const { message } = App.useApp()

  // 筛选输入（老页面进入页面即查询一次；点「查询」重置到第一页）
  const [purchaserId, setPurchaserId] = useState('')
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [status, setStatus] = useState('')
  const [applied, setApplied] = useState<AppliedFilters>({ purchaser_id: '', start_date: '', end_date: '', status: '' })
  const [page, setPage] = useState(1)
  const [sortField, setSortField] = useState('')
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('desc')

  const [detailOpen, setDetailOpen] = useState(false)
  const [detail, setDetail] = useState<SalesOrderDetail | null>(null)
  const [detailLoading, setDetailLoading] = useState(false)

  const { data: purchasers = [] } = useQuery({ queryKey: ['purchasers'], queryFn: listPurchasers })

  const listQuery = useQuery({
    queryKey: ['salesOrderQuery', applied, page, sortField, sortOrder],
    queryFn: () =>
      fetchSalesOrderQuery({ ...applied, page, sort_field: sortField, sort_order: sortOrder }),
  })
  const resp = listQuery.data
  const rows = resp?.data ?? []

  const handleSearch = () => {
    setPage(1)
    setApplied({ purchaser_id: purchaserId, start_date: startDate, end_date: endDate, status })
  }

  // 老页面：同字段切换 asc/desc，新字段从 asc 开始；排序不改当前页码
  const toggleSort = (field: string) => {
    if (sortField === field) {
      setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc')
    } else {
      setSortField(field)
      setSortOrder('asc')
    }
  }

  const sortIndicator = (field: string) => (sortField === field ? (sortOrder === 'asc' ? ' ▲' : ' ▼') : '')
  const sortableHeader = (field: string, label: string): TableColumnsType<SalesOrderQueryRow>[number] => ({
    title: <span>{label}{sortIndicator(field)}</span>,
    onHeaderCell: () => ({ onClick: () => toggleSort(field), style: { cursor: 'pointer' } }),
  })

  const viewDetail = async (id: number) => {
    setDetailOpen(true)
    setDetail(null)
    setDetailLoading(true)
    try {
      const d = await fetchSalesOrderDetail(id)
      setDetail(d)
    } catch (e) {
      message.error(`加载明细失败：${e instanceof Error ? e.message : e}`)
      setDetailOpen(false)
    } finally {
      setDetailLoading(false)
    }
  }

  const handleExport = () => {
    // 老页面导出取当前输入框值（而非已应用条件）
    window.location.href = salesOrderQueryExportUrl({
      purchaser_id: purchaserId,
      start_date: startDate,
      end_date: endDate,
      status,
    })
  }

  const columns: TableProps<SalesOrderQueryRow>['columns'] = [
    { ...sortableHeader('order_no', '订单号'), dataIndex: 'order_no' },
    { ...sortableHeader('unit_name', '采购单位'), dataIndex: 'purchaser_name' },
    { ...sortableHeader('order_date', '日期'), dataIndex: 'order_date' },
    {
      ...sortableHeader('total_amount', '金额'),
      dataIndex: 'total_amount',
      align: 'right',
      render: (v: number) => fmtMoney(v),
    },
    { title: '下浮后', dataIndex: 'final_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    {
      ...sortableHeader('status', '状态'),
      dataIndex: 'status',
      render: (v: string) => <Tag color={statusTagColor(v)}>{v}</Tag>,
    },
    {
      title: '操作',
      key: 'action',
      render: (_v, o) => (
        <Button size="small" onClick={() => void viewDetail(o.id)}>
          查看明细
        </Button>
      ),
    },
  ]

  // 明细行（老页面：预售/数量 toFixed(2)，规格空显示 -，单价/金额 ¥ 前缀）
  type DetailItem = SalesOrderDetail['items'][number] & { key: number }
  const itemColumns: TableProps<DetailItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name', render: (v: string | null) => v || '' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '-' },
    { title: '单位', dataIndex: 'unit', render: (v: string | null) => v || '' },
    { title: '预售数量', dataIndex: 'pre_sale_quantity', align: 'right', render: (v: number | null) => fmt2(v) },
    { title: '数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '单价', dataIndex: 'unit_price', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '金额', dataIndex: 'amount', align: 'right', render: (v: number) => fmtMoney(v) },
  ]
  const detailItems: DetailItem[] = detail ? detail.items.map((it, idx) => ({ ...it, key: idx })) : []
  const itemTotal = detailItems.reduce((s, it) => s + (it.amount || 0), 0)

  return (
    <>
      <ReportCard
        title="销售订单查询"
        filters={
          <>
            <FilterItem label="采购单位" width={200}>
              <Select
                showSearch
                optionFilterProp="label"
                style={{ width: '100%' }}
                value={purchaserId}
                onChange={setPurchaserId}
                options={[
                  { value: '', label: '全部采购单位' },
                  ...purchasers.map((p) => ({ value: String(p.id), label: p.name })),
                ]}
              />
            </FilterItem>
            <FilterItem label="开始日期" width={150}>
              <DatePicker<string>
                style={{ width: '100%' }}
                value={startDate || null}
                onChange={(d) => setStartDate(d ?? '')}
              />
            </FilterItem>
            <FilterItem label="结束日期" width={150}>
              <DatePicker<string>
                style={{ width: '100%' }}
                value={endDate || null}
                onChange={(d) => setEndDate(d ?? '')}
              />
            </FilterItem>
            <FilterItem label="状态" width={140}>
              <Select
                style={{ width: '100%' }}
                value={status}
                onChange={setStatus}
                options={[
                  { value: '', label: '全部状态' },
                  { value: '未发货', label: '未发货' },
                  { value: '部分发货', label: '部分发货' },
                  { value: '已完成', label: '已完成' },
                  { value: '取消', label: '取消' },
                ]}
              />
            </FilterItem>
          </>
        }
        actions={
          <Space wrap>
            <Button type="primary" onClick={handleSearch}>
              查询
            </Button>
            <Button onClick={handleExport}>导出Excel</Button>
          </Space>
        }
      >
        <Table<SalesOrderQueryRow>
          rowKey="id"
          size="small"
          loading={listQuery.isFetching}
          columns={columns}
          dataSource={rows}
          scroll={scrollX('max-content')}
          pagination={
            resp && resp.total_pages > 1
              ? {
                  current: resp.page,
                  pageSize: PAGE_SIZE,
                  total: resp.total,
                  showSizeChanger: false,
                  showTotal: (t) => `共 ${t} 条记录，当前第 ${resp.page}/${resp.total_pages} 页`,
                  onChange: (p) => setPage(p),
                }
              : false
          }
          summary={() =>
            rows.length > 0 ? (
              <Table.Summary.Row>
                <Table.Summary.Cell index={0} colSpan={3}>
                  合计
                </Table.Summary.Cell>
                <Table.Summary.Cell index={1} align="right">
                  {fmtMoney(rows.reduce((s, o) => s + o.total_amount, 0))}
                </Table.Summary.Cell>
                <Table.Summary.Cell index={2} align="right">
                  {fmtMoney(rows.reduce((s, o) => s + (o.final_amount || 0), 0))}
                </Table.Summary.Cell>
                <Table.Summary.Cell index={3} colSpan={2}>
                  {''}
                </Table.Summary.Cell>
              </Table.Summary.Row>
            ) : null
          }
        />
      </ReportCard>

      {/* 明细弹窗：老页面字段顺序 订单号/采购单位、订单日期/订单状态、订单金额/下浮后金额、备注（整行） */}
      <Modal
        title="订单明细"
        open={detailOpen}
        onCancel={() => setDetailOpen(false)}
        footer={
          <Button onClick={() => setDetailOpen(false)}>关闭</Button>
        }
        width={880}
      >
        {detailLoading && (
          <div style={{ textAlign: 'center', padding: 32 }}>
            <Spin />
          </div>
        )}
        {!detailLoading && detail && (
          <>
            <Descriptions size="small" column={2} style={{ marginBottom: 16 }}>
              <Descriptions.Item label="订单号">{detail.order_no}</Descriptions.Item>
              <Descriptions.Item label="采购单位">{detail.purchaser_name}</Descriptions.Item>
              <Descriptions.Item label="订单日期">{detail.order_date || ''}</Descriptions.Item>
              <Descriptions.Item label="订单状态">{detail.status || ''}</Descriptions.Item>
              <Descriptions.Item label="订单金额">{fmtMoney(detail.total_amount)}</Descriptions.Item>
              <Descriptions.Item label="下浮后金额">{fmtMoney(detail.final_amount)}</Descriptions.Item>
              <Descriptions.Item label="备注" span={2}>
                {detail.remark || '-'}
              </Descriptions.Item>
            </Descriptions>
            <Table<DetailItem>
              rowKey="key"
              size="small"
              pagination={false}
              columns={itemColumns}
              dataSource={detailItems}
              scroll={scrollX('max-content')}
              summary={() => (
                <Table.Summary.Row>
                  <Table.Summary.Cell index={0} colSpan={6}>
                    合计
                  </Table.Summary.Cell>
                  <Table.Summary.Cell index={1} align="right">
                    {fmtMoney(itemTotal)}
                  </Table.Summary.Cell>
                </Table.Summary.Row>
              )}
            />
          </>
        )}
      </Modal>
    </>
  )
}
