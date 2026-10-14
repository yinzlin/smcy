import { useState } from 'react'
import { App, Button, DatePicker, Descriptions, Modal, Select, Space, Spin, Table, Tag } from 'antd'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import {
  fetchPurchaseOrderDetail,
  fetchPurchaseOrderQuery,
  purchaseOrderQueryExportUrl,
  type PurchaseOrderQueryRow,
} from '../../api/query/orders'
import { listSuppliers, type PurchaseOrderDetail } from '../../api/purchase'
import { fmt2, fmtMoney } from '../../utils/format'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'

const PAGE_SIZE = 20

/** 老页面 badge 规则：已完成/全部到货=绿、作废=红、其他=橙 */
function statusTagColor(status: string): string {
  if (status === '已完成' || status === '全部到货') return 'green'
  if (status === '作废') return 'red'
  return 'orange'
}

interface AppliedFilters {
  supplier_id: string
  start_date: string
  end_date: string
  status: string
}

export default function PurchaseOrderQueryPage() {
  const { message } = App.useApp()

  // 筛选输入（老页面：点「查询」才把条件作用于请求，进入页面不自动查询）
  const [supplierId, setSupplierId] = useState('')
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [status, setStatus] = useState('')
  const [applied, setApplied] = useState<AppliedFilters | null>(null)
  const [page, setPage] = useState(1)

  const [detailOpen, setDetailOpen] = useState(false)
  const [detail, setDetail] = useState<PurchaseOrderDetail | null>(null)
  const [detailLoading, setDetailLoading] = useState(false)

  const { data: suppliers = [] } = useQuery({ queryKey: ['suppliers'], queryFn: listSuppliers })

  const listQuery = useQuery({
    queryKey: ['purchaseOrderQuery', applied, page],
    queryFn: () => {
      if (!applied) throw new Error('未执行查询')
      return fetchPurchaseOrderQuery({ ...applied, page })
    },
    enabled: applied !== null,
  })
  const resp = listQuery.data
  const rows = resp?.data ?? []

  const handleSearch = () => {
    setPage(1)
    setApplied({ supplier_id: supplierId, start_date: startDate, end_date: endDate, status })
  }

  const viewDetail = async (id: number) => {
    setDetailOpen(true)
    setDetail(null)
    setDetailLoading(true)
    try {
      const d = await fetchPurchaseOrderDetail(id)
      setDetail(d)
    } catch (e) {
      message.error(`加载明细失败：${e instanceof Error ? e.message : e}`)
      setDetailOpen(false)
    } finally {
      setDetailLoading(false)
    }
  }

  const columns: TableProps<PurchaseOrderQueryRow>['columns'] = [
    { title: '订单号', dataIndex: 'order_no' },
    { title: '供应商', dataIndex: 'supplier_name' },
    { title: '日期', dataIndex: 'order_date' },
    {
      title: '金额',
      key: 'amount',
      align: 'right',
      render: (_v, o) => fmtMoney(o.final_amount || o.total_amount),
    },
    {
      title: '状态',
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

  // 明细行（老页面规格空显示 -，订购/数量 toFixed(2)，单价/金额 ¥ 前缀）
  type DetailItem = PurchaseOrderDetail['items'][number] & { key: number }
  const itemColumns: TableProps<DetailItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name', render: (v: string | null) => v || '' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '-' },
    { title: '单位', dataIndex: 'unit', render: (v: string | null) => v || '' },
    { title: '订购数量', dataIndex: 'ordered_quantity', align: 'right', render: (v: number | null) => fmt2(v) },
    { title: '数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '单价', dataIndex: 'unit_price', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '金额', dataIndex: 'amount', align: 'right', render: (v: number) => fmtMoney(v) },
  ]
  const detailItems: DetailItem[] = detail ? detail.items.map((it, idx) => ({ ...it, key: idx })) : []
  const itemTotal = detailItems.reduce((s, it) => s + (it.amount || 0), 0)

  return (
    <>
      <ReportCard
        title="采购订单查询"
        filters={
          <>
            <FilterItem label="供应商" width={200}>
              <Select
                showSearch
                optionFilterProp="label"
                style={{ width: '100%' }}
                value={supplierId}
                onChange={setSupplierId}
                options={[
                  { value: '', label: '全部供应商' },
                  ...suppliers.map((s) => ({ value: String(s.id), label: s.name })),
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
                  { value: '未到货', label: '未到货' },
                  { value: '部分到货', label: '部分到货' },
                  { value: '全部到货', label: '全部到货' },
                  { value: '作废', label: '作废' },
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
            <Button href={purchaseOrderQueryExportUrl()}>导出Excel</Button>
          </Space>
        }
      >
        <Table<PurchaseOrderQueryRow>
          rowKey="id"
          size="small"
          loading={listQuery.isFetching}
          columns={columns}
          dataSource={rows}
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
                  {fmtMoney(rows.reduce((s, o) => s + (o.final_amount || o.total_amount), 0))}
                </Table.Summary.Cell>
                <Table.Summary.Cell index={2} colSpan={2}>
                  {''}
                </Table.Summary.Cell>
              </Table.Summary.Row>
            ) : null
          }
        />
      </ReportCard>

      {/* 明细弹窗：老页面字段顺序 订单号/供应商、订单日期/订单状态、订单金额/实付金额、入库仓库/备注 */}
      <Modal
        title="采购订单明细"
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
              <Descriptions.Item label="供应商">{detail.supplier_name}</Descriptions.Item>
              <Descriptions.Item label="订单日期">{detail.order_date || ''}</Descriptions.Item>
              <Descriptions.Item label="订单状态">{detail.status || ''}</Descriptions.Item>
              <Descriptions.Item label="订单金额">{fmtMoney(detail.total_amount)}</Descriptions.Item>
              <Descriptions.Item label="实付金额">{fmtMoney(detail.final_amount)}</Descriptions.Item>
              <Descriptions.Item label="入库仓库">{detail.warehouse_name || '-'}</Descriptions.Item>
              <Descriptions.Item label="备注">{detail.remark || '-'}</Descriptions.Item>
            </Descriptions>
            <Table<DetailItem>
              rowKey="key"
              size="small"
              pagination={false}
              columns={itemColumns}
              dataSource={detailItems}
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
