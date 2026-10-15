// 今日进价采集（对应老 page_mobile_today_price），与移动分拣族同族全屏页，表现层统一为 AntD
import { useEffect, useState } from 'react'
import { App, Button, Input, Result, Space, Spin, Tag } from 'antd'
import {
  exportTodayPriceUrl,
  fetchSortItemsBySupplier,
  fetchTodayPriceItems,
  generatePurchaseForOrder,
  saveTodayPrices,
  type TodayPriceSupplier,
} from '../../api/todayprice'
import {
  MOBILE_ACCENT,
  MobileBottomBar,
  MobileDateBar,
  MobileEmpty,
  MobileHeader,
  MobileListRow,
  MobileSearchBar,
  MobileSection,
} from '../mobile/mobileKit'
import { useAuth } from '../../stores/auth'
import { todayStr } from '../../utils/format'

const fmtPrice = (v: number | undefined) => (!v || v <= 0 ? '-' : Number(v).toFixed(2))

/** 录入值合法（正数）才算"已录入"，与老页 parseFloat>0 口径一致 */
const isValidPrice = (raw: string) => {
  const n = parseFloat(raw)
  return !isNaN(n) && n > 0
}

export default function TodayPricePage() {
  const { user, checked, fetchMe } = useAuth()
  const { message, modal } = App.useApp()
  const [suppliers, setSuppliers] = useState<TodayPriceSupplier[]>([])
  const [loaded, setLoaded] = useState(false)
  const [date, setDate] = useState(todayStr())
  const [keyword, setKeyword] = useState('')
  /** 今日价录入原文：`${supplier_id}_${product_id}` -> 输入框原始字符串（保留用户输入过程态） */
  const [inputs, setInputs] = useState<Record<string, string>>({})

  // 独立于主布局的全屏页：自行探测登录态（老页服务端校验 super_admin）
  useEffect(() => {
    if (!checked) void fetchMe()
  }, [checked, fetchMe])

  const loadItems = async (d: string) => {
    try {
      setSuppliers(await fetchTodayPriceItems(d))
    } catch (e) {
      console.error('加载失败:', e)
    } finally {
      setLoaded(true)
    }
  }

  useEffect(() => {
    if (checked && user?.role === 'super_admin') void loadItems(todayStr())
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [checked, user?.role])

  if (!checked) {
    return (
      <div className="mobile-content" style={{ textAlign: 'center', paddingTop: 80 }}>
        <Spin tip="加载中..." size="large" />
      </div>
    )
  }

  if (user?.role !== 'super_admin') {
    return <Result status="403" title="您没有权限访问此页面" />
  }

  const savedCount = Object.values(inputs).filter(isValidPrice).length
  let totalCount = 0
  suppliers.forEach((s) => {
    totalCount += s.items?.length ?? 0
  })

  const setInput = (sid: number, pid: number, raw: string) => {
    const key = `${sid}_${pid}`
    setInputs((m) => {
      const next = { ...m }
      if (raw === '') delete next[key]
      else next[key] = raw
      return next
    })
  }

  const savePrices = async () => {
    const items = Object.entries(inputs)
      .filter(([, raw]) => isValidPrice(raw))
      .map(([key, raw]) => {
        const [sid, pid] = key.split('_')
        return { supplier_id: Number(sid), product_id: Number(pid), price: parseFloat(raw) }
      })
    if (items.length === 0) {
      message.warning('请先录入今日价')
      return
    }
    try {
      const resp = await saveTodayPrices(items)
      message.success(resp.message || '保存成功')
      setInputs({})
      void loadItems(date)
    } catch (e) {
      console.error('保存失败:', e)
      message.error('保存失败，请重试')
    }
  }

  // 生成采购订单（与老页一致）：按当前日期取当日销售订单，逐单生成/补全，同供应商同日期自动合并
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
      message.warning(`${date || '当前'} 没有可生成采购订单的销售订单`)
      return
    }

    const run = async () => {
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
      modal.info({ title: '生成采购订单', content: <div style={{ whiteSpace: 'pre-wrap' }}>{msg}</div>, okText: '知道了' })
    }

    modal.confirm({
      title: '生成采购订单',
      content: `将为 ${date || '当前待分拣'} 的 ${orderIds.length} 个销售订单生成或补全采购订单（已审核的自动跳过，同供应商同日期自动合并）。是否继续？`,
      okText: '继续',
      cancelText: '取消',
      onOk: run,
    })
  }

  const doExport = (kind: 'excel' | 'a4' | 'category', printValues: boolean) => {
    window.location.href = exportTodayPriceUrl(kind, date, printValues)
  }

  const kw = keyword.trim().toLowerCase()
  const sections = suppliers
    .map((supplier) => ({
      supplier,
      items: (supplier.items ?? []).filter(
        (it) => it.product_name.toLowerCase().includes(kw) || (it.spec || '').toLowerCase().includes(kw),
      ),
    }))
    .filter(({ items }) => items.length > 0)

  return (
    <>
      <MobileHeader
        color={MOBILE_ACCENT.todayPrice}
        title="💰 今日进价采集"
        info="按供应商汇总当天商品，采集并录入今日进价（最近价）"
        stats={[
          { value: totalCount, label: '商品种类' },
          { value: suppliers.length, label: '供应商数' },
          { value: savedCount, label: '已录入' },
        ]}
      >
        <MobileDateBar
          value={date}
          onChange={setDate}
          onSearch={() => void loadItems(date)}
          onClear={() => {
            setDate('')
            void loadItems('')
          }}
        />
        <MobileSearchBar value={keyword} onChange={setKeyword} onClear={() => setKeyword('')} />
        <div className="mobile-bar">
          <Button type="primary" block onClick={() => void generatePurchaseOrdersForDate()}>
            📋 生成采购订单
          </Button>
        </div>
      </MobileHeader>

      <div className="mobile-content">
        {!loaded ? (
          <MobileEmpty text="暂无采购订单" />
        ) : sections.length === 0 ? (
          <MobileEmpty text="没有找到匹配的商品" />
        ) : (
          sections.map(({ supplier, items }) => (
            <MobileSection
              key={supplier.supplier_id}
              color={MOBILE_ACCENT.todayPrice}
              title={`🏭 ${supplier.supplier_name}`}
              extra={`共 ${supplier.items.length} 种`}
            >
              {items.map((item) => {
                const key = `${supplier.supplier_id}_${item.product_id}`
                const raw = inputs[key]
                const hasInput = raw !== undefined && isValidPrice(raw)
                const defaultPrice = item.supplier_price > 0 ? item.supplier_price : item.purchase_price || 0
                return (
                  <MobileListRow key={item.product_id}>
                    <div className="mobile-row-main">
                      <div className="mobile-row-title">
                        {item.product_name}
                        {item.spec ? ` ${item.spec}` : ''}
                      </div>
                      <Space size={4} wrap>
                        <Tag>基本单位: {item.base_unit}</Tag>
                        <Tag>
                          合计: {item.total_qty} {item.base_unit}
                        </Tag>
                        {item.qty_detail ? <Tag>明细: {item.qty_detail}</Tag> : null}
                      </Space>
                      <Space size={4} wrap style={{ marginTop: 4 }}>
                        <Tag color="green">最高 {fmtPrice(item.max_purchase_price)}</Tag>
                        <Tag color="red">最低 {fmtPrice(item.min_purchase_price)}</Tag>
                        <Tag color="orange">最近 {fmtPrice(item.purchase_price)}</Tag>
                        {item.supplier_price > 0 ? <Tag color="blue">该商 {fmtPrice(item.supplier_price)}</Tag> : null}
                        {hasInput ? <Tag color="cyan">已录入</Tag> : null}
                      </Space>
                    </div>
                    <div className="mobile-row-side">
                      <div className="mobile-qty-unit">今日价</div>
                      <Input
                        type="number"
                        min={0}
                        placeholder={defaultPrice > 0 ? Number(defaultPrice).toFixed(2) : ''}
                        value={raw ?? ''}
                        onChange={(e) => setInput(supplier.supplier_id, item.product_id, e.target.value)}
                        style={{ width: 96, textAlign: 'center' }}
                      />
                    </div>
                  </MobileListRow>
                )
              })}
            </MobileSection>
          ))
        )}
      </div>

      <MobileBottomBar>
        <Button type="primary" onClick={() => void savePrices()}>
          保存今日价
        </Button>
        <Button onClick={() => doExport('excel', true)}>导出XLSX</Button>
        <Button onClick={() => doExport('excel', false)}>导出(手写价)</Button>
        <Button onClick={() => doExport('category', true)}>导出(按分类)</Button>
        <Button onClick={() => doExport('a4', true)}>导出(A4)</Button>
        <Button onClick={() => doExport('a4', false)}>导出A4(手写价)</Button>
      </MobileBottomBar>
    </>
  )
}
