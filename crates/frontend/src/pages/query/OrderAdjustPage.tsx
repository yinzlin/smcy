/**
 * 订单调整与同屏比对（旧路由 /query/order_adjust 迁移）
 * 结构照抄老页面：变更订单列表 → 真实/调整后订单同屏比对 → 调整操作（虚增/变更数量/替换）→ 本单调整记录（可回滚）
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import {
  App, AutoComplete, Badge, Button, Card, Col, DatePicker, Input, InputNumber, Radio, Row, Select, Space, Table, Tag, Typography,
} from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  createSupplement,
  deleteSupplement,
  fetchAdjustedOrders,
  fetchOrderDetail,
  fetchOrderSearch,
  fetchProductPickList,
  fetchSupplementCompare,
  fetchSupplementRecords,
  type AdjustedOrderRow,
  type CompareItem,
  type OrderSearchItem,
  type ProductPickItem,
  type SelectedOrder,
  type SupplementCreateBody,
  type SupplementRecord,
} from '../../api/query/orderadjust'
import { listPurchasers, type SalesOrderDetail } from '../../api/sales'

type AdjType = 'new_item' | 'increase_quantity' | 'replace'

/** 替换明细的加行 */
interface ReplaceLine {
  product_id: number
  product_name: string
  alias1: string
  alias2: string
  spec: string
  unit: string
  unit_price: number
  quantity: number
  amount: number
}

/** 本单调整记录类型文案（照抄老页面 typeMap） */
const ADJ_TYPE_MAP: Record<string, string> = {
  new_item: '虚增明细',
  increase_quantity: '变更数量',
  replace_remove: '替换-冲减',
  replace_add: '替换-换入',
}

const BLANK_EMPTY_TEXT = ' '

export default function OrderAdjustPage() {
  const { message, modal } = App.useApp()
  const queryClient = useQueryClient()

  // ===== 顶部：订单号搜索 =====
  const [orderSearchText, setOrderSearchText] = useState('')
  const [orderOptions, setOrderOptions] = useState<OrderSearchItem[]>([])
  const orderSearchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const handleOrderSearch = (kw: string) => {
    setOrderSearchText(kw)
    if (orderSearchTimer.current) clearTimeout(orderSearchTimer.current)
    orderSearchTimer.current = setTimeout(() => {
      void fetchOrderSearch(kw.trim())
        .then((resp) => setOrderOptions(resp.items ?? resp.data ?? []))
        .catch(() => setOrderOptions([]))
    }, 250)
  }

  // ===== 有变更的订单列表：筛选/分页/排序 =====
  const [adjKeywordInput, setAdjKeywordInput] = useState('')
  const [adjKeyword, setAdjKeyword] = useState('')
  const [adjPurchaserId, setAdjPurchaserId] = useState('')
  const [adjPage, setAdjPage] = useState(1)
  const [adjSortOrder, setAdjSortOrder] = useState<'asc' | 'desc'>('desc')
  const adjOrdersPageSize = 10

  // 老页面 oninput 300ms 防抖后加载并重置页码
  useEffect(() => {
    const t = setTimeout(() => {
      setAdjKeyword(adjKeywordInput.trim())
      setAdjPage(1)
    }, 300)
    return () => clearTimeout(t)
  }, [adjKeywordInput])

  const adjQuery = useMemo(
    () => ({ page: adjPage, page_size: adjOrdersPageSize, keyword: adjKeyword, purchaser_id: adjPurchaserId, sort_order: adjSortOrder }),
    [adjPage, adjKeyword, adjPurchaserId, adjSortOrder],
  )
  const { data: adjResp, isLoading: adjLoading, refetch: refetchAdjusted } = useQuery({
    queryKey: ['adjustedOrders', adjQuery],
    queryFn: () => fetchAdjustedOrders(adjQuery),
  })

  // ===== 选中订单与调整表单状态 =====
  const [adjOrder, setAdjOrder] = useState<SelectedOrder | null>(null)
  const [adjType, setAdjType] = useState<AdjType>('new_item')
  const [adjDate, setAdjDate] = useState(dayjs().format('YYYY-MM-DD'))

  // 虚增商品明细
  const [newProduct, setNewProduct] = useState<ProductPickItem | null>(null)
  const [newProductText, setNewProductText] = useState('')
  const [newProductOptions, setNewProductOptions] = useState<ProductPickItem[]>([])
  const [newQty, setNewQty] = useState<number | null>(null)
  const [newPrice, setNewPrice] = useState<number | null>(null)

  // 变更已有商品数量
  const [incIdx, setIncIdx] = useState(0)
  const [incQty, setIncQty] = useState<number | null>(null)

  // 替换明细
  const [replaceSrcIdx, setReplaceSrcIdx] = useState(0)
  const [replaceProduct, setReplaceProduct] = useState<ProductPickItem | null>(null)
  const [replaceProductText, setReplaceProductText] = useState('')
  const [replaceProductOptions, setReplaceProductOptions] = useState<ProductPickItem[]>([])
  const [replaceQty, setReplaceQty] = useState<number | null>(null)
  const [replacePrice, setReplacePrice] = useState<number | null>(null)
  const [replaceLines, setReplaceLines] = useState<ReplaceLine[]>([])

  const [saving, setSaving] = useState(false)

  // ===== 选单后加载：订单明细 / 同屏比对 / 调整记录 =====
  const detailQuery = useQuery({
    queryKey: ['adjOrderDetail', adjOrder?.id ?? 0],
    queryFn: () => fetchOrderDetail(adjOrder!.id),
    enabled: adjOrder !== null,
  })
  const realItems: SalesOrderDetail['items'] = detailQuery.data?.items ?? []

  const compareQuery = useQuery({
    queryKey: ['adjCompare', adjOrder?.id ?? 0],
    queryFn: () => fetchSupplementCompare(adjOrder!.id),
    enabled: adjOrder !== null,
  })
  const compare = compareQuery.data

  const recordsQuery = useQuery({
    queryKey: ['adjRecords', adjOrder?.id ?? 0],
    queryFn: () => fetchSupplementRecords(adjOrder!.id),
    enabled: adjOrder !== null,
  })

  const { data: purchasers = [] } = useQuery({ queryKey: ['purchasers'], queryFn: listPurchasers })

  // ===== 选择订单（老 selectAdjOrder：重置表单并进入调整区）=====
  const selectAdjOrder = (o: SelectedOrder) => {
    setAdjOrder({ id: o.id, order_no: o.order_no, purchaser_name: o.purchaser_name })
    setOrderSearchText(o.order_no)
    setAdjDate(dayjs().format('YYYY-MM-DD'))
    setAdjType('new_item')
    setNewProduct(null)
    setNewProductText('')
    setNewQty(null)
    setNewPrice(null)
    setIncIdx(0)
    setIncQty(null)
    setReplaceSrcIdx(0)
    setReplaceProduct(null)
    setReplaceProductText('')
    setReplaceQty(null)
    setReplacePrice(null)
    setReplaceLines([])
  }

  // ===== 商品搜索（老 showAdjProducts：kw 为空时点击即拉取前 50 个）=====
  const searchProductOptions = (kw: string, set: (list: ProductPickItem[]) => void) => {
    void fetchProductPickList(kw)
      .then((resp) => set(resp.items ?? resp.data ?? []))
      .catch(() => set([]))
  }
  const productOptionLabel = (p: ProductPickItem) => {
    const a2 = p.alias2 ? '(' + p.alias2 + ')' : ''
    const price = p.selling_price || p.base_price || 0
    return p.name + a2 + ' - ' + price.toFixed(2) + '元/' + (p.unit || '')
  }

  // ===== 保存/回滚后刷新（老 loadCompare + loadAdjRecords + loadAdjustedOrders）=====
  const refreshAfterMutation = (orderId: number) => {
    void queryClient.invalidateQueries({ queryKey: ['adjCompare', orderId] })
    void queryClient.invalidateQueries({ queryKey: ['adjRecords', orderId] })
    void queryClient.invalidateQueries({ queryKey: ['adjustedOrders'] })
  }

  // ===== 加行（老 addAdjReplaceLine）=====
  const addReplaceLine = () => {
    if (!replaceProduct) {
      message.warning('请选择替换商品')
      return
    }
    const qty = replaceQty ?? 0
    const price = replacePrice ?? 0
    const amount = qty * price
    if (qty <= 0 || amount <= 0) {
      message.warning('请输入有效的数量和单价')
      return
    }
    setReplaceLines((ls) => [
      ...ls,
      {
        product_id: replaceProduct.id,
        product_name: replaceProduct.name,
        alias1: replaceProduct.alias1 || '',
        alias2: replaceProduct.alias2 || '',
        spec: replaceProduct.spec || '',
        unit: replaceProduct.unit || '',
        unit_price: price,
        quantity: qty,
        amount,
      },
    ])
    setReplaceProduct(null)
    setReplaceProductText('')
    setReplaceQty(null)
    setReplacePrice(null)
  }

  // ===== 保存调整（老 addAdjustment：保存前校验与确认文案照抄）=====
  const saveAdjustment = async () => {
    if (!adjOrder) return
    const allocDate = adjDate
    setSaving(true)
    try {
      if (adjType === 'replace') {
        const src = realItems[replaceSrcIdx]
        if (!src) {
          message.warning('请选择被替换的原明细')
          return
        }
        if (replaceLines.length === 0) {
          message.warning('请至少添加一条替换商品')
          return
        }
        const replaceTotal = replaceLines.reduce((s, l) => s + l.amount, 0)
        const diff = replaceTotal - src.amount
        if (Math.abs(diff) > 5.0) {
          message.error(`替换总金额 ${replaceTotal.toFixed(2)} 元与原明细 ${src.amount.toFixed(2)} 元差额 ${diff.toFixed(2)} 元，超过±5元限制`)
          return
        }
        const srcRemark = adjOrder.order_no + ' 订单调整-替换[' + src.product_name + ']'
        // 冲减原明细
        const r1 = await createSupplement({
          target_order_id: adjOrder.id,
          source_order_id: adjOrder.id,
          source_remark: srcRemark + ' 冲减',
          product_id: src.product_id,
          product_name: src.product_name,
          alias1: src.alias1 || '',
          alias2: src.alias2 || '',
          spec: src.spec || '',
          unit: src.unit || '',
          unit_price: src.unit_price,
          quantity: -src.quantity,
          amount: -src.amount,
          allocate_date: allocDate,
          operation_type: 'replace_remove',
          target_order_item_id: src.id,
        })
        if (!r1.ok) {
          message.error('保存失败(冲减): ' + r1.message)
          return
        }
        // 换入替换商品
        for (const line of replaceLines) {
          const r2 = await createSupplement({
            target_order_id: adjOrder.id,
            source_order_id: adjOrder.id,
            source_remark: srcRemark + ' 换入',
            product_id: line.product_id,
            product_name: line.product_name,
            alias1: line.alias1,
            alias2: line.alias2,
            spec: line.spec,
            unit: line.unit,
            unit_price: line.unit_price,
            quantity: line.quantity,
            amount: line.amount,
            allocate_date: allocDate,
            operation_type: 'replace_add',
            target_order_item_id: src.id,
          })
          if (!r2.ok) {
            message.error('保存失败(换入): ' + r2.message)
            return
          }
        }
        setReplaceLines([])
        refreshAfterMutation(adjOrder.id)
        message.success('替换已保存')
        return
      }

      let body: SupplementCreateBody
      if (adjType === 'new_item') {
        if (!newProduct) {
          message.warning('请选择要虚增的商品')
          return
        }
        const qty = newQty ?? 0
        const price = newPrice ?? 0
        const amount = qty * price
        if (qty <= 0 || amount <= 0) {
          message.warning('请输入有效的数量和单价')
          return
        }
        body = {
          target_order_id: adjOrder.id,
          source_order_id: adjOrder.id,
          source_remark: adjOrder.order_no + ' 订单调整-虚增',
          product_id: newProduct.id,
          product_name: newProduct.name,
          alias1: newProduct.alias1 || '',
          alias2: newProduct.alias2 || '',
          spec: newProduct.spec || '',
          unit: newProduct.unit || '',
          unit_price: price,
          quantity: qty,
          amount,
          allocate_date: allocDate,
          operation_type: 'new_item',
          target_order_item_id: null,
        }
      } else {
        const item = realItems[incIdx]
        if (!item) {
          message.warning('请选择目标商品')
          return
        }
        const qty = incQty ?? 0
        if (qty === 0) {
          message.warning('请输入变更数量')
          return
        }
        const amount = qty * item.unit_price
        body = {
          target_order_id: adjOrder.id,
          source_order_id: adjOrder.id,
          source_remark: adjOrder.order_no + ' 订单调整-变更数量',
          product_id: item.product_id,
          product_name: item.product_name,
          alias1: item.alias1 || '',
          alias2: item.alias2 || '',
          spec: item.spec || '',
          unit: item.unit || '',
          unit_price: item.unit_price,
          quantity: qty,
          amount,
          allocate_date: allocDate,
          operation_type: 'increase_quantity',
          target_order_item_id: item.id,
        }
      }
      const res = await createSupplement(body)
      if (res.ok) {
        setNewQty(null)
        setIncQty(null)
        refreshAfterMutation(adjOrder.id)
        message.success('调整已保存')
      } else {
        message.error('保存失败: ' + res.message)
      }
    } finally {
      setSaving(false)
    }
  }

  // ===== 回滚（老 rollbackAdj）=====
  const rollbackAdj = (record: SupplementRecord) => {
    modal.confirm({
      title: '确定回滚该调整记录？',
      okText: '确定',
      cancelText: '取消',
      onOk: async () => {
        const res = await deleteSupplement(record.id)
        if (res.ok) {
          message.success('回滚成功')
          if (adjOrder) refreshAfterMutation(adjOrder.id)
        } else {
          message.error('回滚失败: ' + res.message)
        }
      },
    })
  }

  // ===== 有变更的订单列表列头（老 adjustedOrdersTable）=====
  const adjColumns: TableProps<AdjustedOrderRow>['columns'] = [
    {
      title: '订单号',
      dataIndex: 'order_no',
      render: (v: string, o) => <a onClick={() => selectAdjOrder(o)}>{v}</a>,
    },
    { title: '采购单位', dataIndex: 'purchaser_name' },
    {
      title: (
        <span
          style={{ cursor: 'pointer', whiteSpace: 'nowrap' }}
          onClick={() => {
            setAdjSortOrder((s) => (s === 'desc' ? 'asc' : 'desc'))
            setAdjPage(1)
          }}
        >
          订单日期 {adjSortOrder === 'asc' ? '▲' : '▼'}
        </span>
      ),
      dataIndex: 'order_date',
      width: 120,
    },
    { title: '真实金额', dataIndex: 'total_amount', align: 'right', width: 100, render: (v: number) => (v || 0).toFixed(2) },
    {
      title: '调整金额',
      dataIndex: 'adjust_amount',
      align: 'right',
      width: 110,
      render: (v: number) => {
        const diff = v || 0
        const color = diff > 0 ? '#28a745' : diff < 0 ? '#dc3545' : '#6c757d'
        return <span style={{ color }}>{(diff >= 0 ? '+' : '') + diff.toFixed(2)}</span>
      },
    },
    { title: '调整后金额', dataIndex: 'adjusted_total', align: 'right', width: 110, render: (v: number) => <strong>{(v || 0).toFixed(2)}</strong> },
    { title: '调整条数', dataIndex: 'adjust_count', align: 'center', width: 90 },
    { title: '最近调整日', dataIndex: 'last_adjust_date', width: 110 },
    {
      title: '操作',
      key: 'action',
      width: 80,
      render: (_v, o) => (
        <Button size="small" onClick={() => selectAdjOrder(o)}>
          查看
        </Button>
      ),
    },
  ]

  // ===== 同屏比对：真实订单列（老 realTable，虚增行整行显示 —）=====
  const realColumns: TableProps<CompareItem>['columns'] = [
    {
      title: '商品',
      dataIndex: 'display_name',
      render: (_v: string, it) =>
        it.is_new ? (
          <div style={{ textAlign: 'center', color: '#999', fontSize: 12 }}>—</div>
        ) : (
          <span>{it.display_name || it.product_name}</span>
        ),
    },
    {
      title: '数量',
      dataIndex: 'quantity',
      width: 80,
      render: (v: number, it) =>
        it.is_new ? <div style={{ textAlign: 'center', color: '#999', fontSize: 12 }}>—</div> : <div style={{ textAlign: 'right' }}>{v.toFixed(2)}</div>,
    },
    {
      title: '金额',
      dataIndex: 'amount',
      width: 100,
      render: (v: number, it) =>
        it.is_new ? <div style={{ textAlign: 'center', color: '#999', fontSize: 12 }}>—</div> : <div style={{ textAlign: 'right' }}>{v.toFixed(2)}</div>,
    },
  ]

  // ===== 同屏比对：调整后订单列（老 adjTable，行底色区分虚增/替换/加量）=====
  const adjustedColumns: TableProps<CompareItem>['columns'] = [
    {
      title: '商品',
      dataIndex: 'display_name',
      render: (_v: string, it) => {
        const name = it.display_name || it.product_name
        if (it.is_new) return <span><strong>[虚增]</strong> {name}</span>
        if (it.is_replaced) return <span><del>{name}</del> <Tag color="error">已替换</Tag></span>
        if (it.is_increase) return <span>{name} <Tag color="success">+{it.supplement_quantity.toFixed(2)}</Tag></span>
        return <span>{name}</span>
      },
    },
    { title: '数量', dataIndex: 'total_quantity', width: 80, render: (v: number) => <div style={{ textAlign: 'right' }}>{v.toFixed(2)}</div> },
    { title: '金额', dataIndex: 'total_amount', width: 100, render: (v: number) => <div style={{ textAlign: 'right' }}>{v.toFixed(2)}</div> },
  ]

  // ===== 替换加行列表列头（老 adjReplaceLineList）=====
  const replaceLineColumns: TableProps<ReplaceLine>['columns'] = [
    {
      title: '替换商品',
      dataIndex: 'product_name',
      render: (v: string, l) => v + (l.alias2 ? '(' + l.alias2 + ')' : ''),
    },
    { title: '数量', dataIndex: 'quantity', width: 90, render: (v: number) => <div style={{ textAlign: 'right' }}>{v.toFixed(2)}</div> },
    { title: '单价', dataIndex: 'unit_price', width: 90, render: (v: number) => <div style={{ textAlign: 'right' }}>{v.toFixed(2)}</div> },
    { title: '金额', dataIndex: 'amount', width: 100, render: (v: number) => <div style={{ textAlign: 'right' }}>{v.toFixed(2)}</div> },
    {
      title: '操作',
      key: 'action',
      width: 80,
      render: (_v, _l, index) => (
        <Button size="small" danger onClick={() => setReplaceLines((ls) => ls.filter((_, i) => i !== index))}>
          删除
        </Button>
      ),
    },
  ]

  // ===== 本单调整记录列头（老 adjRecordList）=====
  const recordColumns: TableProps<SupplementRecord>['columns'] = [
    { title: '类型', dataIndex: 'operation_type', width: 100, render: (v: string) => ADJ_TYPE_MAP[v] || v },
    { title: '商品', dataIndex: 'product_name' },
    { title: '数量', dataIndex: 'quantity', width: 90, align: 'right', render: (v: number) => (v || 0).toFixed(2) },
    { title: '金额', dataIndex: 'amount', width: 100, align: 'right', render: (v: number) => (v || 0).toFixed(2) },
    { title: '日期', dataIndex: 'allocate_date', width: 110, render: (v: string) => v || '' },
    {
      title: '操作',
      key: 'action',
      width: 80,
      render: (_v, r) => (
        <Button size="small" danger onClick={() => rollbackAdj(r)}>
          回滚
        </Button>
      ),
    },
  ]

  // ===== 分页（老 renderAdjustedPager：上一页 / 5 页码窗口 / 下一页）=====
  const adjTotal = adjResp?.total ?? 0
  const adjPages = Math.max(1, Math.ceil(adjTotal / adjOrdersPageSize))
  const maxShow = 5
  const rawPagerStart = Math.max(1, adjPage - Math.floor(maxShow / 2))
  const pagerEnd = Math.min(adjPages, rawPagerStart + maxShow - 1)
  const pagerStart = Math.max(1, pagerEnd - maxShow + 1)
  const pagerNums: number[] = []
  for (let p = pagerStart; p <= pagerEnd; p++) pagerNums.push(p)

  const replaceTotal = replaceLines.reduce((s, l) => s + l.amount, 0)
  const replaceSrc = realItems[replaceSrcIdx]
  const replaceDiff = replaceSrc ? replaceTotal - replaceSrc.amount : null

  return (
    <>
      <Card title="订单调整与同屏比对">
        <Typography.Paragraph type="secondary" style={{ fontSize: 12, marginBottom: 16 }}>
          在真实订单基础上虚增商品明细或变更数量，调整内容独立记录、可回滚。真实订单始终保留做底根，同屏比对差异。报销口径统一采用调整后订单。
        </Typography.Paragraph>

        {/* 订单号搜索 */}
        <Row gutter={16} style={{ marginBottom: 16 }}>
          <Col span={8}>
            <div style={{ marginBottom: 4 }}>订单号搜索：</div>
            <AutoComplete
              style={{ width: '100%' }}
              value={orderSearchText}
              placeholder="输入订单号搜索"
              options={orderOptions.map((o) => ({
                value: String(o.id),
                label:
                  o.order_no + ' | ' + (o.purchaser_name || '') + ' | ' + o.order_date + ' | ¥' +
                  (o.final_amount || o.total_amount || 0).toFixed(2),
              }))}
              onChange={handleOrderSearch}
              onSelect={(v) => {
                const found = orderOptions.find((x) => String(x.id) === v)
                if (found) selectAdjOrder(found)
              }}
            />
          </Col>
        </Row>

        {/* 有变更的订单列表 */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
          <span style={{ fontWeight: 600 }}>
            有变更的订单列表 <Badge count={adjTotal} showZero color="#17a2b8" overflowCount={9999999} />
          </span>
          <Button size="small" onClick={() => void refetchAdjusted()}>
            刷新
          </Button>
        </div>
        <Row gutter={16} style={{ marginBottom: 8 }}>
          <Col span={8}>
            <div style={{ marginBottom: 4, fontSize: 12 }}>订单号搜索：</div>
            <Input size="small" placeholder="输入订单号搜索" value={adjKeywordInput} onChange={(e) => setAdjKeywordInput(e.target.value)} />
          </Col>
          <Col span={8}>
            <div style={{ marginBottom: 4, fontSize: 12 }}>采购单位筛选：</div>
            <Select
              size="small"
              style={{ width: '100%' }}
              value={adjPurchaserId}
              onChange={(v) => {
                setAdjPurchaserId(v)
                setAdjPage(1)
              }}
              options={[{ value: '', label: '全部单位' }, ...purchasers.map((p) => ({ value: String(p.id), label: p.name }))]}
            />
          </Col>
        </Row>

        <Table<AdjustedOrderRow>
          rowKey="id"
          size="small"
          bordered
          loading={adjLoading}
          columns={adjColumns}
          dataSource={adjResp?.items ?? []}
          pagination={false}
          locale={{ emptyText: '暂无' }}
        />

        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 8 }}>
          <span style={{ fontSize: 12, color: '#6c757d' }}>
            {'合计：真实金额 '}
            <strong>¥{(adjResp?.total_real_amount ?? 0).toFixed(2)}</strong>
            {' ｜ 调整金额 '}
            <strong>¥{(adjResp?.total_adjust_amount ?? 0).toFixed(2)}</strong>
            {' ｜ 调整后金额 '}
            <strong>¥{(adjResp?.total_adjusted_amount ?? 0).toFixed(2)}</strong>
          </span>
          <Space size={4}>
            <Button size="small" disabled={adjPage <= 1} onClick={() => setAdjPage(adjPage - 1)}>
              上一页
            </Button>
            {pagerNums.map((p) => (
              <Button key={p} size="small" type={p === adjPage ? 'primary' : 'default'} onClick={() => setAdjPage(p)}>
                {p}
              </Button>
            ))}
            <Button size="small" disabled={adjPage >= adjPages} onClick={() => setAdjPage(adjPage + 1)}>
              下一页
            </Button>
          </Space>
        </div>
      </Card>

      {adjOrder && (
        <>
          {/* 同屏比对：真实订单（底根） vs 调整后订单（报销口径） */}
          <Card title={adjOrder.order_no + '（' + (adjOrder.purchaser_name || '') + '）'} style={{ marginTop: 16 }}>
            <Row gutter={16}>
              <Col span={12}>
                <Card
                  styles={{
                    header: { background: '#6c757d', color: '#fff', padding: '4px 12px', minHeight: 32 },
                    body: { padding: 4 },
                  }}
                  title={
                    <span style={{ fontSize: 12 }}>
                      <strong>真实订单（底根）</strong> - ¥{(compare?.real_total ?? 0).toFixed(2)}
                    </span>
                  }
                >
                  <div style={{ maxHeight: 340, overflowY: 'auto' }}>
                    <Table<CompareItem>
                      rowKey="id"
                      size="small"
                      bordered
                      loading={compareQuery.isLoading}
                      columns={realColumns}
                      dataSource={compare?.items ?? []}
                      pagination={false}
                      locale={{ emptyText: BLANK_EMPTY_TEXT }}
                    />
                  </div>
                </Card>
              </Col>
              <Col span={12}>
                <Card
                  styles={{
                    header: { background: '#28a745', color: '#fff', padding: '4px 12px', minHeight: 32 },
                    body: { padding: 4 },
                  }}
                  title={
                    <span style={{ fontSize: 12 }}>
                      <strong>调整后订单（报销口径）</strong> - ¥{(compare?.allocation_total ?? 0).toFixed(2)}
                      <Tag color="warning" style={{ marginLeft: 8 }}>
                        差异 {((compare?.allocation_total ?? 0) - (compare?.real_total ?? 0)).toFixed(2)}
                      </Tag>
                    </span>
                  }
                >
                  <div style={{ maxHeight: 340, overflowY: 'auto' }}>
                    <Table<CompareItem>
                      rowKey="id"
                      size="small"
                      bordered
                      loading={compareQuery.isLoading}
                      columns={adjustedColumns}
                      dataSource={compare?.items ?? []}
                      pagination={false}
                      locale={{ emptyText: BLANK_EMPTY_TEXT }}
                      onRow={(it) => ({
                        style: {
                          background: it.is_new ? '#fff3cd' : it.is_replaced ? '#f8d7da' : it.is_increase ? '#d4edda' : undefined,
                        },
                      })}
                    />
                  </div>
                </Card>
              </Col>
            </Row>
          </Card>

          {/* 调整操作 */}
          <Card title="调整操作" style={{ marginTop: 16 }}>
            <Radio.Group value={adjType} onChange={(e) => setAdjType(e.target.value)} style={{ marginBottom: 12 }}>
              <Radio value="new_item">虚增商品明细</Radio>
              <Radio value="increase_quantity">变更已有商品数量</Radio>
              <Radio value="replace">替换明细</Radio>
            </Radio.Group>

            {adjType === 'new_item' && (
              <Row gutter={16} style={{ marginBottom: 12 }} align="bottom">
                <Col span={8}>
                  <div style={{ marginBottom: 4 }}>选择商品</div>
                  <AutoComplete
                    style={{ width: '100%' }}
                    value={newProductText}
                    placeholder="点击选择商品"
                    options={newProductOptions.map((p) => ({ value: String(p.id), label: productOptionLabel(p), product: p }))}
                    onFocus={() => searchProductOptions('', setNewProductOptions)}
                    onChange={(v) => {
                      if (newProductOptions.some((p) => String(p.id) === v)) return
                      setNewProductText(v)
                      searchProductOptions(v, setNewProductOptions)
                    }}
                    onSelect={(_v, opt) => {
                      const p = (opt as { product: ProductPickItem }).product
                      setNewProduct(p)
                      setNewProductText(p.name + (p.alias2 ? '(' + p.alias2 + ')' : ''))
                      setNewPrice(p.selling_price || p.base_price || 0)
                    }}
                  />
                </Col>
                <Col span={4}>
                  <div style={{ marginBottom: 4 }}>数量</div>
                  <InputNumber style={{ width: '100%' }} step={0.01} value={newQty} onChange={(v) => setNewQty(v)} />
                </Col>
                <Col span={4}>
                  <div style={{ marginBottom: 4 }}>单价</div>
                  <InputNumber style={{ width: '100%' }} step={0.01} value={newPrice} onChange={(v) => setNewPrice(v)} />
                </Col>
                <Col span={4}>
                  <div style={{ marginBottom: 4 }}>金额</div>
                  <Input readOnly value={((newQty ?? 0) * (newPrice ?? 0)).toFixed(2)} />
                </Col>
              </Row>
            )}

            {adjType === 'increase_quantity' && (
              <Row gutter={16} style={{ marginBottom: 12 }} align="bottom">
                <Col span={8}>
                  <div style={{ marginBottom: 4 }}>选择目标商品</div>
                  <Select
                    style={{ width: '100%' }}
                    value={realItems.length > 0 ? incIdx : undefined}
                    onChange={(v) => setIncIdx(v)}
                    options={realItems.map((item, idx) => ({
                      value: idx,
                      label: item.product_name + ' (' + item.quantity.toFixed(2) + ' ' + (item.unit || '') + ')',
                    }))}
                  />
                </Col>
                <Col span={4}>
                  <div style={{ marginBottom: 4 }}>追加数量</div>
                  <InputNumber style={{ width: '100%' }} step={0.01} value={incQty} onChange={(v) => setIncQty(v)} />
                </Col>
                <Col span={4}>
                  <div style={{ marginBottom: 4 }}>单价</div>
                  <Input readOnly value={(realItems[incIdx]?.unit_price ?? 0).toFixed(2)} />
                </Col>
                <Col span={4}>
                  <div style={{ marginBottom: 4 }}>追加金额</div>
                  <Input readOnly value={((incQty ?? 0) * (realItems[incIdx]?.unit_price ?? 0)).toFixed(2)} />
                </Col>
                <Col span={4}>
                  <span style={{ color: '#6c757d', fontSize: 12 }}>
                    {realItems[incIdx]
                      ? '合计数量: ' + (realItems[incIdx].quantity + (incQty ?? 0)).toFixed(2)
                      : '合计数量: 0'}
                  </span>
                </Col>
              </Row>
            )}

            {adjType === 'replace' && (
              <>
                <Row gutter={16} style={{ marginBottom: 12 }} align="bottom">
                  <Col span={8}>
                    <div style={{ marginBottom: 4 }}>被替换的原明细</div>
                    <Select
                      style={{ width: '100%' }}
                      value={realItems.length > 0 ? replaceSrcIdx : undefined}
                      onChange={(v) => setReplaceSrcIdx(v)}
                      options={realItems.map((item, idx) => ({
                        value: idx,
                        label: item.product_name + ' (' + item.quantity.toFixed(2) + ' ' + (item.unit || '') + ' = ' + item.amount.toFixed(2) + '元)',
                      }))}
                    />
                  </Col>
                </Row>
                <Row gutter={16} style={{ marginBottom: 12 }} align="bottom">
                  <Col span={8}>
                    <div style={{ marginBottom: 4 }}>替换商品</div>
                    <AutoComplete
                      style={{ width: '100%' }}
                      value={replaceProductText}
                      placeholder="点击选择替换商品"
                      options={replaceProductOptions.map((p) => ({ value: String(p.id), label: productOptionLabel(p), product: p }))}
                      onFocus={() => searchProductOptions('', setReplaceProductOptions)}
                      onChange={(v) => {
                        if (replaceProductOptions.some((p) => String(p.id) === v)) return
                        setReplaceProductText(v)
                        searchProductOptions(v, setReplaceProductOptions)
                      }}
                      onSelect={(_v, opt) => {
                        const p = (opt as { product: ProductPickItem }).product
                        setReplaceProduct(p)
                        setReplaceProductText(p.name + (p.alias2 ? '(' + p.alias2 + ')' : ''))
                        setReplacePrice(p.selling_price || p.base_price || 0)
                      }}
                    />
                  </Col>
                  <Col span={4}>
                    <div style={{ marginBottom: 4 }}>数量</div>
                    <InputNumber style={{ width: '100%' }} step={0.01} value={replaceQty} onChange={(v) => setReplaceQty(v)} />
                  </Col>
                  <Col span={4}>
                    <div style={{ marginBottom: 4 }}>单价</div>
                    <InputNumber style={{ width: '100%' }} step={0.01} value={replacePrice} onChange={(v) => setReplacePrice(v)} />
                  </Col>
                  <Col span={4}>
                    <div style={{ marginBottom: 4 }}>金额</div>
                    <Input readOnly value={((replaceQty ?? 0) * (replacePrice ?? 0)).toFixed(2)} />
                  </Col>
                  <Col span={4}>
                    <Button type="primary" ghost onClick={addReplaceLine}>
                      加行
                    </Button>
                  </Col>
                </Row>
                <Table<ReplaceLine>
                  rowKey={(_l, i) => 'line-' + (i ?? 0)}
                  size="small"
                  bordered
                  columns={replaceLineColumns}
                  dataSource={replaceLines}
                  pagination={false}
                  locale={{ emptyText: BLANK_EMPTY_TEXT }}
                />
                <div style={{ marginTop: 8 }}>
                  {'替换合计: '}
                  <strong>{replaceTotal.toFixed(2)}</strong>
                  {' 元'}
                  {replaceDiff !== null && (
                    <span
                      style={{ marginLeft: 16, fontSize: 12, color: Math.abs(replaceDiff) <= 5.0 ? '#28a745' : '#dc3545' }}
                    >
                      差额 {replaceDiff.toFixed(2)} 元（{Math.abs(replaceDiff) <= 5.0 ? '在±5元内，可提交' : '超过±5元限制'}）
                    </span>
                  )}
                </div>
              </>
            )}

            <Row gutter={16} align="bottom" style={{ marginTop: 12 }}>
              <Col span={4}>
                <div style={{ marginBottom: 4 }}>调整日期</div>
                <DatePicker
                  style={{ width: '100%' }}
                  value={adjDate ? dayjs(adjDate) : null}
                  onChange={(d) => setAdjDate(d ? d.format('YYYY-MM-DD') : '')}
                />
              </Col>
              <Col span={20}>
                <Button type="primary" size="small" loading={saving} onClick={() => void saveAdjustment()}>
                  保存调整
                </Button>
              </Col>
            </Row>

            <div style={{ fontWeight: 600, margin: '16px 0 8px' }}>本单调整记录（可回滚）</div>
            <Table<SupplementRecord>
              rowKey="id"
              size="small"
              bordered
              loading={recordsQuery.isLoading}
              columns={recordColumns}
              dataSource={recordsQuery.data ?? []}
              pagination={false}
              locale={{ emptyText: BLANK_EMPTY_TEXT }}
            />
          </Card>
        </>
      )}
    </>
  )
}
