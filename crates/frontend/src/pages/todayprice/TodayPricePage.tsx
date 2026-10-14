import { useMemo, useState } from 'react'
import { App, Button, Card, Col, DatePicker, Input, InputNumber, Result, Row, Space, Statistic, Table, Tag, Typography } from 'antd'
import { DownloadOutlined, SaveOutlined, ThunderboltOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  exportTodayPriceUrl,
  fetchSortItemsBySupplier,
  fetchTodayPriceItems,
  generatePurchaseForOrder,
  saveTodayPrices,
  type TodayPriceItem,
  type TodayPriceSupplier,
} from '../../api/todayprice'
import { useAuth } from '../../stores/auth'
import { scrollX } from '../../utils/tableScroll'

const fmtPrice = (v: number | undefined) => (!v || v <= 0 ? '-' : Number(v).toFixed(2))

interface Props {
  embeddedDate?: string
}

export default function TodayPricePage({ embeddedDate }: Props) {
  const { message, modal } = App.useApp()
  const { user } = useAuth()
  const queryClient = useQueryClient()
  const [date, setDate] = useState(embeddedDate ?? dayjs().format('YYYY-MM-DD'))
  const [dateInput, setDateInput] = useState(embeddedDate ?? dayjs().format('YYYY-MM-DD'))
  const [keyword, setKeyword] = useState('')
  /** 今日价录入值：`${supplier_id}_${product_id}` -> price */
  const [todayPrices, setTodayPrices] = useState<Record<string, number>>({})

  const { data: suppliers = [], isLoading } = useQuery({
    queryKey: ['todayPriceItems', date],
    queryFn: () => fetchTodayPriceItems(date),
    enabled: user?.role === 'super_admin',
  })

  const stats = useMemo(() => {
    let totalCount = 0
    suppliers.forEach((s) => {
      totalCount += s.items?.length ?? 0
    })
    return { totalCount, supplierCount: suppliers.length, savedCount: Object.keys(todayPrices).length }
  }, [suppliers, todayPrices])

  if (user && user.role !== 'super_admin') {
    return <Result status="403" title="无权限" subTitle="今日进价采集仅超级管理员可用" />
  }

  const priceKey = (sid: number, pid: number) => `${sid}_${pid}`

  const setPrice = (sid: number, pid: number, value: number) => {
    setTodayPrices((m) => {
      const next = { ...m }
      if (value > 0) next[priceKey(sid, pid)] = value
      else delete next[priceKey(sid, pid)]
      return next
    })
  }

  const savePrices = async () => {
    const keys = Object.keys(todayPrices)
    if (keys.length === 0) {
      message.warning('请先录入今日价')
      return
    }
    const items = keys.map((key) => {
      const [sid, pid] = key.split('_')
      return { supplier_id: Number(sid), product_id: Number(pid), price: todayPrices[key] }
    })
    try {
      const resp = await saveTodayPrices(items)
      message.success(resp.message || '保存成功')
      setTodayPrices({})
      void queryClient.invalidateQueries({ queryKey: ['todayPriceItems'] })
    } catch (e) {
      message.error(`保存失败：${e instanceof Error ? e.message : e}`)
    }
  }

  // 生成采购订单（自供应商分拣页迁入）：按当前日期取当日销售订单，逐单生成/补全，同供应商同日期自动合并
  const generatePurchaseOrdersForDate = async () => {
    let sortSuppliers: Awaited<ReturnType<typeof fetchSortItemsBySupplier>>
    try {
      sortSuppliers = await fetchSortItemsBySupplier(date)
    } catch (e) {
      message.error(`获取当日销售订单失败：${e instanceof Error ? e.message : e}`)
      return
    }
    const orderIds: number[] = []
    ;(sortSuppliers || []).forEach((supplier) => {
      supplier.purchasers?.forEach((purchaser) => {
        purchaser.items?.forEach((item) => {
          if (item.order_id && !orderIds.includes(item.order_id)) orderIds.push(item.order_id)
        })
      })
    })
    if (orderIds.length === 0) {
      message.warning(`${date} 没有可生成采购订单的销售订单`)
      return
    }
    const ok = await new Promise<boolean>((resolve) => {
      modal.confirm({
        title: '生成采购订单',
        content: `将为 ${date} 的 ${orderIds.length} 个销售订单生成或补全采购订单（已审核的自动跳过，同供应商同日期自动合并）。是否继续？`,
        okText: '确定',
        cancelText: '取消',
        onOk: () => resolve(true),
        onCancel: () => resolve(false),
      })
    })
    if (!ok) return

    let createdCount = 0
    let mergedCount = 0
    let refreshedCount = 0
    let excludedCount = 0
    const errors: string[] = []
    const toRefresh: number[] = []

    // 第一遍：未生成的新建；已有待分拣的收集待补全；已审核的跳过
    for (const orderId of orderIds) {
      try {
        const data = await generatePurchaseForOrder(orderId, false)
        if (data.error) {
          if (data.excluded) excludedCount += Number(data.excluded_count || 1)
          else errors.push(String(data.message || `订单 ${orderId} 生成失败`))
          continue
        }
        if (data.warning) {
          toRefresh.push(orderId)
          continue
        }
        createdCount += Number(data.count || 0)
        mergedCount += Number(data.merged || 0)
        excludedCount += Number(data.excluded_count || 0)
      } catch {
        errors.push(`订单 ${orderId} 生成失败`)
      }
    }

    // 第二遍：补全未审核（待分拣）的采购订单——按新销售单明细增删改
    for (const orderId of toRefresh) {
      try {
        const data = await generatePurchaseForOrder(orderId, true)
        if (data.error) {
          if (data.excluded) excludedCount += Number(data.excluded_count || 1)
          else errors.push(String(data.message || `订单 ${orderId} 补全失败`))
          continue
        }
        refreshedCount += 1
        createdCount += Number(data.count || 0)
        mergedCount += Number(data.merged || 0)
        excludedCount += Number(data.excluded_count || 0)
      } catch {
        errors.push(`订单 ${orderId} 补全失败`)
      }
    }

    let msg = `成功生成 ${createdCount} 张采购订单`
    if (refreshedCount > 0) msg += `，补全 ${refreshedCount} 张未审核采购订单`
    if (mergedCount > 0) msg += `，合并 ${mergedCount} 条明细`
    if (excludedCount > 0) msg += `（已审核 ${excludedCount} 张自动跳过）`
    if (errors.length > 0) msg += `\n\n以下订单生成失败：\n${errors.join('\n')}`
    modal.info({ title: '生成结果', content: <div style={{ whiteSpace: 'pre-line' }}>{msg}</div> })
  }

  const doExport = (kind: 'excel' | 'a4' | 'category', printValues: boolean) => {
    window.location.href = exportTodayPriceUrl(kind, date, printValues)
  }

  return (
    <>
      <Card style={{ marginBottom: 16 }}>
        <Row gutter={[16, 16]} align="middle">
          <Col>
            <Space wrap>
              <span>日期：</span>
              <DatePicker
                value={dateInput ? dayjs(dateInput) : null}
                onChange={(d) => setDateInput(d ? d.format('YYYY-MM-DD') : '')}
                allowClear
              />
              <Button
                type="primary"
                onClick={() => {
                  setDate(dateInput)
                  setTodayPrices({})
                }}
              >
                检索
              </Button>
              <Button
                onClick={() => {
                  setDateInput('')
                  setDate('')
                  setTodayPrices({})
                }}
              >
                清除
              </Button>
            </Space>
          </Col>
          <Col flex="auto">
            <Input.Search
              placeholder="搜索商品名称..."
              style={{ maxWidth: 280 }}
              allowClear
              onSearch={setKeyword}
              onChange={(e) => {
                if (!e.target.value) setKeyword('')
              }}
            />
          </Col>
          <Col>
            <Button type="primary" icon={<ThunderboltOutlined />} onClick={() => void generatePurchaseOrdersForDate()}>
              生成采购订单
            </Button>
          </Col>
        </Row>
        <Row gutter={16} style={{ marginTop: 16 }}>
          <Col xs={12} md={6} lg={4}>
            <Statistic title="商品种类" value={stats.totalCount} />
          </Col>
          <Col xs={12} md={6} lg={4}>
            <Statistic title="供应商数" value={stats.supplierCount} />
          </Col>
          <Col xs={12} md={6} lg={4}>
            <Statistic title="已录入" value={stats.savedCount} valueStyle={{ color: '#16a34a' }} />
          </Col>
        </Row>
      </Card>

      {suppliers.length === 0 && !isLoading ? (
        <Card>
          <div style={{ textAlign: 'center', padding: '60px 0', color: '#999' }}>
            <div style={{ fontSize: 48 }}>📭</div>
            <div>{date ? `${date} 暂无采购订单` : '当前待分拣暂无采购订单'}</div>
          </div>
        </Card>
      ) : (
        suppliers
          .map((supplier) => ({ supplier, items: filterItems(supplier, keyword) }))
          .filter(({ items }) => items.length > 0)
          .map(({ supplier, items }) => (
            <Card
              key={supplier.supplier_id}
              title={`🏭 ${supplier.supplier_name}`}
              extra={<span>共 {supplier.items.length} 种</span>}
              style={{ marginBottom: 16 }}
            >
              <Table<TodayPriceItem>
                rowKey="product_id"
                size="small"
                pagination={false}
                columns={supplierColumns(supplier, todayPrices, setPrice)}
                dataSource={items}
                scroll={scrollX('max-content')}
              />
            </Card>
          ))
      )}

      <div style={{ position: 'sticky', bottom: 0, background: '#fff', padding: '12px 0', borderTop: '1px solid #f0f0f0' }}>
        <Space wrap>
          <Button type="primary" icon={<SaveOutlined />} onClick={() => void savePrices()}>
            保存今日价
          </Button>
          <Button type="primary" ghost icon={<DownloadOutlined />} onClick={() => doExport('excel', true)}>
            导出XLSX
          </Button>
          <Button icon={<DownloadOutlined />} onClick={() => doExport('excel', false)}>
            导出(手写价)
          </Button>
          <Button type="primary" ghost icon={<DownloadOutlined />} onClick={() => doExport('category', true)}>
            导出(按分类)
          </Button>
          <Button type="primary" ghost icon={<DownloadOutlined />} onClick={() => doExport('a4', true)}>
            导出(A4)
          </Button>
          <Button icon={<DownloadOutlined />} onClick={() => doExport('a4', false)}>
            导出A4(手写价)
          </Button>
        </Space>
      </div>
    </>
  )
}

function filterItems(supplier: TodayPriceSupplier, keyword: string): TodayPriceItem[] {
  const kw = keyword.trim().toLowerCase()
  if (!supplier.items) return []
  if (!kw) return supplier.items
  return supplier.items.filter(
    (item) => item.product_name.toLowerCase().includes(kw) || (item.spec || '').toLowerCase().includes(kw),
  )
}

/** 每个供应商的列定义仅在"今日价"列上不同（输入值与已录入标签按 supplier+product 作用域） */
function supplierColumns(
  supplier: TodayPriceSupplier,
  todayPrices: Record<string, number>,
  setPrice: (sid: number, pid: number, v: number) => void,
): TableProps<TodayPriceItem>['columns'] {
  const base = baseColumns()
  base[base.length - 1] = {
    title: '今日价',
    key: 'input',
    width: 150,
    render: (_v, o) => {
      const key = `${supplier.supplier_id}_${o.product_id}`
      const hasInput = todayPrices[key] !== undefined
      const defaultPrice = o.supplier_price > 0 ? o.supplier_price : o.purchase_price || 0
      return (
        <Space size={4}>
          <InputNumber
            size="small"
            style={{ width: 90 }}
            min={0}
            precision={2}
            controls={false}
            placeholder={defaultPrice > 0 ? Number(defaultPrice).toFixed(2) : ''}
            value={hasInput ? todayPrices[key] : null}
            onChange={(v) => setPrice(supplier.supplier_id, o.product_id, v ?? 0)}
          />
          {hasInput && <Tag color="success">已录入</Tag>}
        </Space>
      )
    },
  }
  return base
}

function baseColumns(): NonNullable<TableProps<TodayPriceItem>['columns']> {
  return [
    {
      title: '商品',
      dataIndex: 'product_name',
      width: 180,
      render: (v: string, o) => (
        <div>
          <div style={{ fontWeight: 600 }}>{v}</div>
          {o.spec && <Typography.Text type="secondary" style={{ fontSize: 12 }}>{o.spec}</Typography.Text>}
        </div>
      ),
    },
    { title: '基本单位', dataIndex: 'base_unit', width: 90 },
    {
      title: '合计',
      dataIndex: 'total_qty',
      width: 130,
      render: (v: number, o) => (
        <span>
          {v} {o.base_unit}
          {o.qty_detail && (
            <div>
              <Typography.Text type="secondary" style={{ fontSize: 12 }}>明细: {o.qty_detail}</Typography.Text>
            </div>
          )}
        </span>
      ),
    },
    { title: '最高', dataIndex: 'max_purchase_price', width: 80, align: 'right', render: (v: number) => <span style={{ color: '#16a34a' }}>{fmtPrice(v)}</span> },
    { title: '最低', dataIndex: 'min_purchase_price', width: 80, align: 'right', render: (v: number) => <span style={{ color: '#dc2626' }}>{fmtPrice(v)}</span> },
    { title: '最近', dataIndex: 'purchase_price', width: 80, align: 'right', render: (v: number) => <span style={{ color: '#d97706' }}>{fmtPrice(v)}</span> },
    { title: '该商', dataIndex: 'supplier_price', width: 80, align: 'right', render: (v: number) => (v > 0 ? <span style={{ color: '#2563eb' }}>{fmtPrice(v)}</span> : '-') },
    { title: '今日价', key: 'input', width: 150 },
  ]
}
