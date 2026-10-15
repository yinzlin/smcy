// 今日进价采集（对应老 page_mobile_today_price，1:1 手写复刻，与移动分拣页同族）
import { useEffect, useState } from 'react'
import {
  EmptyState,
  PageHeader,
  SortStyles,
  todayStr,
} from '../mobile/sortShared'
import {
  exportTodayPriceUrl,
  fetchSortItemsBySupplier,
  fetchTodayPriceItems,
  generatePurchaseForOrder,
  saveTodayPrices,
  type TodayPriceSupplier,
} from '../../api/todayprice'
import { useAuth } from '../../stores/auth'

/** 本页专属样式：橙色主题覆盖 + 价格卡片（共享外壳的绿色变体在老页中为本页的橙色） */
const TP_CSS = `
.msr-tp .history-bar { background: #fff7ed; }
.msr-tp .history-bar button { background: #f97316; }
.msr-tp .history-bar button.clear { background: #f3f4f6; }
.msr-tp .cat-supplier { background: linear-gradient(135deg, #f97316 0%, #fb923c 100%); }
.msr-tp .price-card { display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-bottom: 1px solid #f0f0f0; }
.msr-tp .price-card:last-child { border-bottom: none; }
.msr-tp .price-badge { flex-shrink: 0; text-align: right; }
.msr-tp .price-label { font-size: 11px; color: #666; margin-bottom: 2px; }
.msr-tp .price-input { width: 84px; padding: 8px; border: 1px solid #ddd; border-radius: 6px; font-size: 15px; text-align: center; }
.msr-tp .price-input:focus { outline: none; border-color: #f97316; }
.msr-tp .saved-tag { background: #dcfce7; color: #16a34a; padding: 2px 5px; border-radius: 3px; font-size: 11px; }
.msr-tp .btn-save { background: #f97316; color: white; }
.msr-tp .btn-export { background: #10b981; color: white; }
.msr-tp .btn-export-plain { background: #3b82f6; color: white; }
`

const fmtPrice = (v: number | undefined) => (!v || v <= 0 ? '-' : Number(v).toFixed(2))

/** 录入值合法（正数）才算"已录入"，与老页 parseFloat>0 口径一致 */
const isValidPrice = (raw: string) => {
  const n = parseFloat(raw)
  return !isNaN(n) && n > 0
}

export default function TodayPricePage() {
  const { user, checked, fetchMe } = useAuth()
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
      <div className="msr msr-tp">
        <SortStyles />
        <style>{TP_CSS}</style>
      </div>
    )
  }

  if (user?.role !== 'super_admin') {
    return (
      <div className="msr msr-tp">
        <SortStyles />
        <style>{TP_CSS}</style>
        <EmptyState icon="🔒" text="您没有权限访问此页面" />
      </div>
    )
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
      window.alert('请先录入今日价')
      return
    }
    try {
      const resp = await saveTodayPrices(items)
      window.alert(resp.message || '保存成功')
      setInputs({})
      void loadItems(date)
    } catch (e) {
      console.error('保存失败:', e)
      window.alert('保存失败，请重试')
    }
  }

  // 生成采购订单（与老页一致）：按当前日期取当日销售订单，逐单生成/补全，同供应商同日期自动合并
  const generatePurchaseOrdersForDate = async () => {
    let sortSuppliers: Awaited<ReturnType<typeof fetchSortItemsBySupplier>>
    try {
      sortSuppliers = await fetchSortItemsBySupplier(date)
    } catch (e) {
      window.alert(`获取当日销售订单失败：${e instanceof Error ? e.message : e}`)
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
      window.alert(`${date || '当前'} 没有可生成采购订单的销售订单`)
      return
    }
    if (
      !window.confirm(
        `将为 ${date || '当前待分拣'} 的 ${orderIds.length} 个销售订单生成或补全采购订单（已审核的自动跳过，同供应商同日期自动合并）。\n是否继续？`,
      )
    ) {
      return
    }

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
    window.alert(msg)
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
    <div className="msr msr-tp">
      <SortStyles />
      <style>{TP_CSS}</style>
      <div className="sticky-header">
        <PageHeader
          gradient="linear-gradient(135deg, #f97316 0%, #fb923c 100%)"
          title="💰 今日进价采集"
          info="按供应商汇总当天商品，采集并录入今日进价（最近价）"
          links={[
            { label: '统筹分拣', to: '/mobile/sort' },
            { label: '按单位分拣', to: '/mobile/sort-by-purchaser' },
            { label: '按分类分拣', to: '/mobile/sort-by-category' },
            { label: '按供应商分拣', to: '/mobile/sort-by-supplier' },
            { label: '综合分拣', to: '/mobile/sort-comprehensive' },
          ]}
          actionLinks={[{ label: '📋 生成采购订单', onClick: () => void generatePurchaseOrdersForDate() }]}
          stats={[
            { value: totalCount, label: '商品种类' },
            { value: suppliers.length, label: '供应商数' },
            { value: savedCount, label: '已录入' },
          ]}
        />
        <div className="history-bar">
          <input
            type="date"
            title="选择日期检索该日期的历史销售订单，留空显示当前待分拣"
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
          <button onClick={() => void loadItems(date)}>检索</button>
          <button
            className="clear"
            onClick={() => {
              setDate('')
              void loadItems('')
            }}
          >
            清除
          </button>
        </div>
        <div className="filter-bar">
          <input
            type="text"
            placeholder="搜索商品名称..."
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
          />
          <button className="clear" onClick={() => setKeyword('')}>
            清除
          </button>
        </div>
      </div>

      <div className="content-area content-area-padded">
        {!loaded ? (
          <EmptyState icon="📭" text="暂无采购订单" />
        ) : sections.length === 0 ? (
          <EmptyState icon="🔍" text="没有找到匹配的商品" />
        ) : (
          sections.map(({ supplier, items }) => (
            <div className="category-section" key={supplier.supplier_id}>
              <div className="category-header cat-supplier">
                <h3>🏭 {supplier.supplier_name}</h3>
                <div className="category-stats">共 {supplier.items.length} 种</div>
              </div>
              <div className="category-body">
                {items.map((item) => {
                  const key = `${supplier.supplier_id}_${item.product_id}`
                  const raw = inputs[key]
                  const hasInput = raw !== undefined && isValidPrice(raw)
                  const defaultPrice = item.supplier_price > 0 ? item.supplier_price : item.purchase_price || 0
                  return (
                    <div className="price-card" key={item.product_id}>
                      <div className="item-info">
                        <div className="item-name">
                          {item.product_name}
                          {item.spec ? ` ${item.spec}` : ''}
                        </div>
                        <div className="item-detail">
                          <span>基本单位: {item.base_unit}</span>
                          <span>合计: {item.total_qty} {item.base_unit}</span>
                          {item.qty_detail && <span>明细: {item.qty_detail}</span>}
                        </div>
                        <div className="item-detail" style={{ marginTop: 4 }}>
                          <span style={{ color: '#16a34a' }}>最高 {fmtPrice(item.max_purchase_price)}</span>
                          <span style={{ color: '#dc2626' }}>最低 {fmtPrice(item.min_purchase_price)}</span>
                          <span style={{ color: '#d97706' }}>最近 {fmtPrice(item.purchase_price)}</span>
                          {item.supplier_price > 0 && <span style={{ color: '#2563eb' }}>该商 {fmtPrice(item.supplier_price)}</span>}
                          {hasInput && <span className="saved-tag">已录入</span>}
                        </div>
                      </div>
                      <div className="price-badge">
                        <div className="price-label">今日价</div>
                        <input
                          type="number"
                          min={0}
                          step="any"
                          className="price-input"
                          placeholder={defaultPrice > 0 ? Number(defaultPrice).toFixed(2) : ''}
                          value={raw ?? ''}
                          onChange={(e) => setInput(supplier.supplier_id, item.product_id, e.target.value)}
                          onClick={(e) => e.stopPropagation()}
                        />
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          ))
        )}
      </div>

      <div className="bottom-bar">
        <button className="btn-save" onClick={() => void savePrices()}>保存今日价</button>
        <button className="btn-export" onClick={() => doExport('excel', true)}>导出XLSX</button>
        <button className="btn-export-plain" onClick={() => doExport('excel', false)}>导出(手写价)</button>
        <button className="btn-export" onClick={() => doExport('category', true)}>导出(按分类)</button>
        <button className="btn-export" onClick={() => doExport('a4', true)}>导出(A4)</button>
        <button className="btn-export-plain" onClick={() => doExport('a4', false)}>导出A4(手写价)</button>
      </div>
    </div>
  )
}
