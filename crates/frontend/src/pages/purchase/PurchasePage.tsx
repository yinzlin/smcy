import { useState } from 'react'
import { App, Button, Input, Modal, Popconfirm, Select, Space, Table, Tag, Upload } from 'antd'
import type { TableProps } from 'antd'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  approvePurchaseOrder,
  deletePurchaseOrder,
  exportPurchaseOrdersUrl,
  exportPrintUrl,
  getPurchaseOrderDetail,
  importPurchaseOrders,
  listPurchaseOrders,
  listSuppliers,
  listUsers,
  settlePurchaseOrder,
  unapprovePurchaseOrder,
  type PurchaseOrderDetail,
  type PurchaseOrderListItem,
} from '../../api/purchase'
import { useAuth } from '../../stores/auth'
import { scrollX } from '../../utils/tableScroll'
import PurchaseFormCard from './PurchaseFormCard'

const STATUS_LABELS: Record<string, string> = {
  pending: '待审核',
  confirmed: '已审核',
  sorting: '分拣中',
  sorted: '已分拣',
  delivering: '配送中',
  delivered: '已送达',
  accepted: '已验收',
  settled: '已结算',
  cancelled: '已作废',
}

export default function PurchasePage() {
  const { message, modal } = App.useApp()
  const { user } = useAuth()
  const isSuperAdmin = user?.role === 'super_admin'
  const queryClient = useQueryClient()

  const [keywordInput, setKeywordInput] = useState('')
  const [keyword, setKeyword] = useState('')
  const [supplierId, setSupplierId] = useState<number | null>(null)
  const [isSettled, setIsSettled] = useState<number | null>(null)
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)
  const [sortField, setSortField] = useState('')
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('desc')
  const [editing, setEditing] = useState<PurchaseOrderDetail | null>(null)

  const { data: suppliers = [] } = useQuery({ queryKey: ['suppliers'], queryFn: listSuppliers })
  const { data: users = [] } = useQuery({ queryKey: ['users'], queryFn: listUsers })

  const { data: listResp, isLoading } = useQuery({
    queryKey: ['purchaseOrders', { page, pageSize, keyword, supplierId, isSettled, sortField, sortOrder }],
    queryFn: () =>
      listPurchaseOrders({
        page,
        page_size: pageSize,
        keyword,
        supplier_id: supplierId,
        is_settled: isSettled,
        sort_field: sortField,
        sort_order: sortOrder,
      }),
  })
  const orders = listResp?.data ?? []

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['purchaseOrders'] })

  const runAction = async (action: () => Promise<{ ok: boolean; message: string }>, successText: string) => {
    const res = await action()
    if (res.ok) {
      message.success(successText)
      refresh()
    } else {
      message.error(res.message || '操作失败')
    }
  }

  const openUnapproveModal = (id: number) => {
    let reason = ''
    modal.confirm({
      title: '请输入反审核原因（必填）',
      content: (
        <Input.TextArea
          rows={3}
          placeholder="反审核原因"
          onChange={(e) => {
            reason = e.target.value
          }}
        />
      ),
      okText: '确定',
      cancelText: '取消',
      onOk: () => {
        if (!reason.trim()) {
          message.warning('反审核必须填写原因')
          return Promise.reject()
        }
        return runAction(() => unapprovePurchaseOrder(id, reason.trim()), '反审核成功，订单已解锁')
      },
    })
  }

  const [exportOrderId, setExportOrderId] = useState<number | null>(null)
  const [exportUserId, setExportUserId] = useState<number>(0)

  const loadDetail = async (id: number) => {
    try {
      const detail = await getPurchaseOrderDetail(id)
      setEditing(detail)
      window.scrollTo({ top: 0, behavior: 'smooth' })
    } catch {
      message.error('加载订单详情失败')
    }
  }

  const sums = orders.reduce(
    (acc, o) => ({
      amount: acc.amount + o.total_amount,
      discounted: acc.discounted + o.total_amount * (1 - (o.discount_rate || 0) / 100),
      reduction: acc.reduction + (o.amount_reduction || 0),
      final: acc.final + (o.final_amount || 0),
    }),
    { amount: 0, discounted: 0, reduction: 0, final: 0 },
  )

  const sortable = (key: string, title: string) => ({
    title,
    sorter: true as const,
    sortOrder: (sortField === key ? (sortOrder === 'asc' ? 'ascend' : 'descend') : null) as 'ascend' | 'descend' | null,
  })

  const columns: TableProps<PurchaseOrderListItem>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    { dataIndex: 'order_no', key: 'order_no', width: 170, ...sortable('order_no', '订单号') },
    { dataIndex: 'order_date', key: 'order_date', width: 110, ...sortable('order_date', '日期') },
    { dataIndex: 'supplier_name', key: 'unit_name', width: 150, ...sortable('unit_name', '供应商') },
    { title: '入库仓库', dataIndex: 'warehouse_name', width: 110, render: (v: string) => v || '' },
    { title: '金额', dataIndex: 'total_amount', width: 90, align: 'right', render: (v: number) => v.toFixed(2) },
    {
      title: '下浮后',
      key: 'discounted',
      width: 90,
      align: 'right',
      render: (_v, o) => (o.total_amount * (1 - (o.discount_rate || 0) / 100)).toFixed(2),
    },
    { title: '折减', dataIndex: 'amount_reduction', width: 80, align: 'right', render: (v: number) => v.toFixed(2) },
    { title: '最终金额', dataIndex: 'final_amount', width: 90, align: 'right', render: (v: number) => v.toFixed(2) },
    {
      dataIndex: 'status',
      key: 'status',
      width: 100,
      ...sortable('status', '状态'),
      render: (v: string) => (
        <Tag color={v === 'confirmed' ? 'success' : v === 'pending' ? 'warning' : 'default'}>{STATUS_LABELS[v] ?? v}</Tag>
      ),
    },
    {
      title: '是否结算',
      dataIndex: 'is_settled',
      width: 90,
      render: (v: number) => (v === 1 ? <Tag color="success">已结</Tag> : <Tag color="warning">未结</Tag>),
    },
    {
      title: '操作',
      key: 'action',
      width: 390,
      render: (_v, o) => (
        <Space size={4} wrap>
          {o.status === 'pending' ? (
            <Popconfirm
              title={`确定将该订单设为「${o.is_settled === 1 ? '未结' : '已结'}」？`}
              onConfirm={() => void runAction(() => settlePurchaseOrder(o.id, o.is_settled === 1 ? 0 : 1), '操作成功')}
            >
              <Button size="small" onClick={(e) => e.stopPropagation()}>
                {o.is_settled === 1 ? '设为未结' : '设为已结'}
              </Button>
            </Popconfirm>
          ) : (
            <Button
              size="small"
              disabled
              title="该单据已审核，需先反审核后才能修改结算状态"
              onClick={(e) => e.stopPropagation()}
            >
              设为已结
            </Button>
          )}
          {o.status === 'pending' && (
            <Popconfirm
              title="确定审核通过该订单？审核后订单将被锁定，修改需管理员反审核。"
              onConfirm={() => void runAction(() => approvePurchaseOrder(o.id), '审核成功')}
            >
              <Button size="small" type="primary" ghost onClick={(e) => e.stopPropagation()}>
                审核
              </Button>
            </Popconfirm>
          )}
          {(o.status === 'confirmed' || isSuperAdmin) && (
            <Button
              size="small"
              onClick={(e) => {
                e.stopPropagation()
                openUnapproveModal(o.id)
              }}
            >
              反审核
            </Button>
          )}
          <Button
            size="small"
            onClick={(e) => {
              e.stopPropagation()
              setExportUserId(0)
              setExportOrderId(o.id)
            }}
          >
            导出采购单
          </Button>
          <Popconfirm title="确定删除该订单？" onConfirm={() => void runAction(() => deletePurchaseOrder(o.id), '删除成功')}>
            <Button size="small" danger onClick={(e) => e.stopPropagation()}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ]

  return (
    <>
      <PurchaseFormCard
        key={editing?.id ?? 'new'}
        editing={editing}
        onSaved={() => {
          setEditing(null)
          refresh()
        }}
        onCancelEdit={() => setEditing(null)}
      />

      <div style={{ display: 'flex', gap: 8, marginBottom: 12, flexWrap: 'wrap' }}>
        <Input.Search
          placeholder="搜索订单号、供应商、日期..."
          style={{ width: 260 }}
          value={keywordInput}
          onChange={(e) => setKeywordInput(e.target.value)}
          onSearch={(v) => {
            setKeyword(v.trim())
            setPage(1)
          }}
          allowClear
        />
        <Select
          style={{ width: 200 }}
          placeholder="全部供应商"
          allowClear
          showSearch
          optionFilterProp="label"
          value={supplierId ?? undefined}
          onChange={(v) => {
            setSupplierId(v ?? null)
            setPage(1)
          }}
          options={suppliers.map((s) => ({ value: s.id, label: s.name }))}
        />
        <Select
          style={{ width: 150 }}
          value={isSettled}
          onChange={(v) => {
            setIsSettled(v)
            setPage(1)
          }}
          options={[
            { value: null, label: '是否结算:全部' },
            { value: 0, label: '是否结算:未结' },
            { value: 1, label: '是否结算:已结' },
          ]}
        />
        <Button
          type="primary"
          onClick={() => {
            setKeyword(keywordInput.trim())
            setPage(1)
          }}
        >
          搜索
        </Button>
        <Button
          onClick={() => {
            setKeywordInput('')
            setKeyword('')
            setSupplierId(null)
            setIsSettled(null)
            setPage(1)
          }}
        >
          重置
        </Button>
        <Button
          onClick={() => {
            setEditing(null)
            window.scrollTo({ top: 0, behavior: 'smooth' })
          }}
        >
          取消
        </Button>
        <Button type="primary" ghost href={exportPurchaseOrdersUrl({ keyword, supplier_id: supplierId, sort_field: sortField, sort_order: sortOrder })}>
          导出
        </Button>
        <Upload
          accept=".csv"
          showUploadList={false}
          beforeUpload={(file) => {
            void file.text().then(async (text) => {
              const res = await importPurchaseOrders(text)
              if (res.ok) {
                message.success(res.message || '导入成功')
                refresh()
              } else {
                message.error(res.message || '导入失败')
              }
            })
            return false
          }}
        >
          <Button>导入</Button>
        </Upload>
      </div>

      <Table<PurchaseOrderListItem>
        rowKey="id"
        size="small"
        loading={isLoading}
        columns={columns}
        dataSource={orders}
        scroll={scrollX('max-content')}
        onRow={(o) => ({ onClick: () => void loadDetail(o.id), style: { cursor: 'pointer' } })}
        onChange={(_p, _f, sorter) => {
          const s = Array.isArray(sorter) ? sorter[0] : sorter
          if (s?.order) {
            setSortField(String(s.columnKey ?? s.field ?? ''))
            setSortOrder(s.order === 'ascend' ? 'asc' : 'desc')
          } else {
            setSortField('')
          }
        }}
        pagination={{
          current: page,
          pageSize,
          total: listResp?.total ?? 0,
          showSizeChanger: true,
          showTotal: (t) => `共 ${t} 条记录`,
          onChange: (p, ps) => {
            setPage(p)
            setPageSize(ps)
          },
        }}
        summary={() =>
          orders.length > 0 ? (
            <Table.Summary.Row style={{ fontWeight: 600, background: '#fafafa' }}>
              <Table.Summary.Cell index={0} colSpan={5} align="right">
                合计
              </Table.Summary.Cell>
              <Table.Summary.Cell index={5} align="right">
                {sums.amount.toFixed(2)}
              </Table.Summary.Cell>
              <Table.Summary.Cell index={6} align="right">
                {sums.discounted.toFixed(2)}
              </Table.Summary.Cell>
              <Table.Summary.Cell index={7} align="right">
                {sums.reduction.toFixed(2)}
              </Table.Summary.Cell>
              <Table.Summary.Cell index={8} align="right">
                {sums.final.toFixed(2)}
              </Table.Summary.Cell>
              <Table.Summary.Cell index={9} colSpan={3} />
            </Table.Summary.Row>
          ) : null
        }
      />

      <Modal
        title="导出采购单：选择经手人"
        open={exportOrderId !== null}
        onCancel={() => setExportOrderId(null)}
        onOk={() => {
          if (exportOrderId !== null) {
            window.location.href = exportPrintUrl(exportOrderId, exportUserId)
          }
          setExportOrderId(null)
        }}
        okText="导出"
        cancelText="取消"
      >
        <Select
          style={{ width: '100%' }}
          value={exportUserId}
          onChange={(v) => setExportUserId(v)}
          options={[
            { value: 0, label: '0. 无经手人（使用订单已存）' },
            ...users
              .filter((u) => (u.nickname || u.username) !== '')
              .map((u) => ({
                value: u.id,
                label: `${u.nickname || u.username}${u.phone ? ` (${u.phone})` : ''}`,
              })),
          ]}
        />
      </Modal>
    </>
  )
}
