// 对应老页函数 page_mobile_sort_by_supplier，原路由 /mobile/sort_by_supplier
import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  exportSortBySupplierExcelUrl,
  fetchSortItemsBySupplier,
  saveSortCorrections,
  type SortBySupplierGroup,
  type SortBySupplierItem,
  type SortCorrectionEntry,
} from '../../api/mobile/sortBySupplier'

const CHECKED_KEY = 'sort_by_supplier_checked_ids'
const CORRECTIONS_KEY = 'sort_by_supplier_corrections'

function todayStr(): string {
  const d0 = new Date()
  return `${d0.getFullYear()}-${String(d0.getMonth() + 1).padStart(2, '0')}-${String(d0.getDate()).padStart(2, '0')}`
}

function loadCheckedIds(): Set<number> {
  const saved = localStorage.getItem(CHECKED_KEY)
  const ids = new Set<number>()
  if (saved) {
    ;(JSON.parse(saved) as number[]).forEach((id) => ids.add(id))
  }
  return ids
}

function loadCorrections(): Record<number, number> {
  const saved = localStorage.getItem(CORRECTIONS_KEY)
  if (saved) return JSON.parse(saved) as Record<number, number>
  return {}
}

/* 老页 CSS 逐条照抄；* 与 body 规则按 SPA 场景收敛到页面根节点，
   button/input font-family 继承用于复刻老页 bootstrap reboot 的效果 */
const PAGE_CSS = `
.mobile-sort-supplier-page { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif; background: #f5f7fa; min-height: 100vh; }
.mobile-sort-supplier-page * { margin: 0; padding: 0; box-sizing: border-box; }
.mobile-sort-supplier-page button, .mobile-sort-supplier-page input { font-family: inherit; }
.sticky-header { position: sticky; top: 0; z-index: 100; }
.page-header { background: linear-gradient(135deg, #10b981 0%, #34d399 100%); color: white; padding: 16px 20px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
.page-header h1 { font-size: 18px; margin: 0; font-weight: 600; }
.header-info { font-size: 13px; opacity: 0.9; margin-top: 4px; }
.switch-links { display: flex; gap: 8px; margin-top: 8px; }
.switch-link { padding: 6px 12px; background: rgba(255,255,255,0.2); border-radius: 6px; font-size: 13px; text-decoration: none; color: white; }
.switch-link:hover { background: rgba(255,255,255,0.3); }
.stats-bar { display: flex; gap: 12px; margin-top: 12px; }
.stat-item { background: rgba(255,255,255,0.2); padding: 8px 12px; border-radius: 8px; flex: 1; text-align: center; }
.stat-value { font-size: 16px; font-weight: bold; }
.stat-label { font-size: 11px; opacity: 0.8; }
.content-area { padding: 12px; padding-bottom: 80px; }
.category-section { border-radius: 12px; margin-bottom: 16px; overflow: hidden; box-shadow: 0 2px 8px rgba(0,0,0,0.08); }
.category-header { padding: 14px 16px; display: flex; align-items: center; justify-content: space-between; }
.category-header h3 { font-size: 16px; margin: 0; color: white; font-weight: 600; }
.category-stats { font-size: 13px; opacity: 0.9; color: white; }
.category-body { background: white; padding: 0; }
.sort-card { display: flex; align-items: center; gap: 14px; padding: 14px 16px; border-bottom: 1px solid #f0f0f0; transition: background 0.2s; }
.sort-card:last-child { border-bottom: none; }
.sort-card:hover { background: #f9fafb; }
.sort-card.checked { background: #f0fdf4; }
.checkbox-wrapper { flex-shrink: 0; }
.checkbox-custom { width: 26px; height: 26px; border-radius: 6px; border: 2px solid #ddd; display: flex; align-items: center; justify-content: center; cursor: pointer; transition: all 0.2s; }
.checkbox-custom.checked { background: #10b981; border-color: #10b981; }
.checkbox-custom.checked::after { content: '✓'; color: white; font-size: 16px; font-weight: bold; }
.item-info { flex: 1; min-width: 0; }
.item-name { font-size: 15px; font-weight: 600; color: #333; margin-bottom: 3px; }
.item-detail { font-size: 12px; color: #666; display: flex; gap: 12px; flex-wrap: wrap; }
.item-detail span { background: #f3f4f6; padding: 2px 6px; border-radius: 3px; }
.quantity-badge { flex-shrink: 0; text-align: right; }
.quantity-value { font-size: 18px; font-weight: bold; color: #3b82f6; }
.quantity-unit { font-size: 11px; color: #666; }
.filter-bar { background: white; padding: 12px; border-bottom: 1px solid #eee; display: flex; gap: 8px; }
.filter-bar input { flex: 1; padding: 10px 14px; border: 1px solid #ddd; border-radius: 8px; font-size: 14px; }
.filter-bar button { padding: 10px 16px; border: none; border-radius: 8px; background: #3b82f6; color: white; font-size: 14px; }
.filter-bar button.clear { background: #f3f4f6; color: #666; }
.history-bar { background: #ecfdf5; padding: 12px; border-bottom: 1px solid #eee; display: flex; gap: 8px; }
.history-bar input { flex: 1; padding: 10px 14px; border: 1px solid #ddd; border-radius: 8px; font-size: 14px; }
.history-bar button { padding: 10px 16px; border: none; border-radius: 8px; background: #10b981; color: white; font-size: 14px; white-space: nowrap; }
.history-bar button.clear { background: #f3f4f6; color: #666; }
.bottom-bar { background: white; padding: 6px 12px; position: fixed; bottom: 0; left: 0; right: 0; display: flex; gap: 6px; box-shadow: 0 -2px 8px rgba(0,0,0,0.05); }
.bottom-bar button { flex: 1; padding: 6px; border: none; border-radius: 6px; font-size: 11px; font-weight: 600; }
.btn-select-all { background: #f3f4f6; color: #333; }
.btn-clear-all { background: #fee2e2; color: #dc2626; }
.btn-export { background: #10b981; color: white; }
.empty-state { text-align: center; padding: 60px 20px; color: #999; }
.empty-icon { font-size: 48px; margin-bottom: 16px; }
.cat-supplier { background: linear-gradient(135deg, #10b981 0%, #34d399 100%); }
.correction-input { width: 60px; padding: 6px; border: 1px solid #ddd; border-radius: 4px; font-size: 14px; text-align: center; }
.correction-input-wide { width: 92px; }
.correction-input:focus { outline: none; border-color: #3b82f6; }
.corrected-tag { background: #fef3c7; color: #d97706; padding: 2px 5px; border-radius: 3px; font-size: 11px; }
.purchaser-section { margin: 0 12px; border-bottom: 1px solid #f0f0f0; padding: 10px 0; }
.purchaser-section:last-child { border-bottom: none; }
.purchaser-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; padding: 8px 12px; background: #f8fafc; border-radius: 6px; }
.purchaser-name { font-size: 14px; font-weight: 600; color: #333; }
.purchaser-qty { font-size: 12px; color: #666; }
`

export default function SortBySupplierPage() {
  const [suppliers, setSuppliers] = useState<SortBySupplierGroup[]>([])
  const [loaded, setLoaded] = useState(false)
  const [historyDate, setHistoryDate] = useState(todayStr)
  const [keyword, setKeyword] = useState('')
  const [checkedIds, setCheckedIds] = useState<Set<number>>(loadCheckedIds)
  const [corrections, setCorrections] = useState<Record<number, number>>(loadCorrections)
  // 递增用于重置非受控修正输入框（老页靠 innerHTML 重绘实现同样效果）
  const [correctionsVersion, setCorrectionsVersion] = useState(0)

  const persistChecked = (ids: Set<number>) => {
    localStorage.setItem(CHECKED_KEY, JSON.stringify(Array.from(ids)))
  }
  const persistCorrections = (c: Record<number, number>) => {
    localStorage.setItem(CORRECTIONS_KEY, JSON.stringify(c))
  }

  // dateOverride 供「清除」按钮在 setState 生效前以空日期强制重载
  const loadItems = async (dateOverride?: string) => {
    const date = dateOverride !== undefined ? dateOverride : historyDate
    try {
      const data = await fetchSortItemsBySupplier(date)
      setSuppliers(data)
      setLoaded(true)
      setCorrectionsVersion((v) => v + 1)
    } catch (e) {
      console.error('加载失败:', e)
    }
  }

  // 默认加载当天数据
  useEffect(() => {
    void loadItems()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const clearHistory = () => {
    setHistoryDate('')
    void loadItems('')
  }

  const updateCorrectedQuantity = (productId: number, value: string) => {
    const numValue = parseFloat(value)
    const next = { ...corrections }
    if (numValue && numValue > 0) {
      next[productId] = numValue
    } else {
      delete next[productId]
    }
    persistCorrections(next)
    setCorrections(next)
  }

  const clearCorrections = () => {
    setCorrections({})
    persistCorrections({})
    setCorrectionsVersion((v) => v + 1)
  }

  const saveCorrectionsToServer = async () => {
    const keys = Object.keys(corrections)
    if (keys.length === 0) {
      window.alert('没有需要保存的修正')
      return
    }
    const payload: SortCorrectionEntry[] = keys.map((k) => ({
      id: parseInt(k, 10),
      quantity: corrections[Number(k)],
    }))
    try {
      const text = await saveSortCorrections(payload)
      window.alert(text)
      clearCorrections()
      await loadItems()
    } catch (e) {
      console.error('保存失败:', e)
      window.alert('保存失败，请重试')
    }
  }

  const toggleCheck = (productId: number) => {
    const next = new Set(checkedIds)
    if (next.has(productId)) {
      next.delete(productId)
    } else {
      next.add(productId)
    }
    persistChecked(next)
    setCheckedIds(next)
  }

  const toggleSelectAll = () => {
    const allIds: number[] = []
    suppliers.forEach((supplier) => {
      if (supplier.purchasers) {
        supplier.purchasers.forEach((purchaser) => {
          purchaser.items.forEach((item) => allIds.push(item.item_id))
        })
      }
    })
    const next = new Set(checkedIds)
    if (next.size === allIds.length) {
      next.clear()
    } else {
      allIds.forEach((id) => next.add(id))
    }
    persistChecked(next)
    setCheckedIds(next)
  }

  const clearSelection = () => {
    const next = new Set<number>()
    persistChecked(next)
    setCheckedIds(next)
  }

  const clearSearch = () => {
    setKeyword('')
  }

  const exportExcel = (withValues: boolean) => {
    window.location.href = exportSortBySupplierExcelUrl(historyDate, withValues)
  }

  // 统计（与老页 updateStats 一致：total 为全部条目数，不按搜索过滤）
  let totalCount = 0
  suppliers.forEach((supplier) => {
    if (supplier.purchasers) {
      supplier.purchasers.forEach((purchaser) => {
        totalCount += purchaser.items.length
      })
    }
  })

  // 渲染数据（与老页 renderItems 一致：按关键字过滤商品；
  // 供应商无匹配商品时仍输出空 category-section 外壳）
  const kw = keyword.trim().toLowerCase()
  const sections = suppliers.map((supplier) => {
    const purchasers = (supplier.purchasers || [])
      .map((purchaser) => ({
        purchaser,
        filtered: purchaser.items.filter((item) => item.product_name.toLowerCase().includes(kw)),
      }))
      .filter((entry) => entry.filtered.length > 0)
    return { supplier, purchasers }
  })
  const hasItems = sections.some((s) => s.purchasers.length > 0)

  const renderItemCard = (item: SortBySupplierItem) => {
    const isChecked = checkedIds.has(item.item_id)
    const isCorrected = corrections[item.item_id] !== undefined
    const displayQty = isCorrected ? corrections[item.item_id] : item.quantity
    return (
      <div
        className={isChecked ? 'sort-card checked' : 'sort-card'}
        key={item.item_id}
        onClick={() => toggleCheck(item.item_id)}
      >
        <div className="checkbox-wrapper">
          <div className={isChecked ? 'checkbox-custom checked' : 'checkbox-custom'} />
        </div>
        <div className="item-info">
          <div className="item-name">{item.product_name}</div>
          <div className="item-detail">
            <span>{item.unit}</span>
            <span>单价 ¥{(item.unit_price || 0).toFixed(2)}</span>
            {item.remark && item.remark.trim() ? (
              <span style={{ color: '#d97706' }}>备注: {item.remark}</span>
            ) : null}
            {isCorrected ? (
              <span className="corrected-tag">
                数量修正: {item.quantity}→{displayQty}
              </span>
            ) : null}
          </div>
        </div>
        <div className="quantity-badge">
          <div className="quantity-value">{displayQty}</div>
          <div className="quantity-unit">{item.unit}</div>
          <input
            type="text"
            inputMode="decimal"
            className="correction-input correction-input-wide"
            placeholder="修正数量"
            key={`${item.item_id}:${correctionsVersion}`}
            defaultValue={isCorrected ? corrections[item.item_id] : ''}
            onBlur={(e) => updateCorrectedQuantity(item.item_id, e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') updateCorrectedQuantity(item.item_id, e.currentTarget.value)
            }}
            onClick={(e) => e.stopPropagation()}
          />
        </div>
      </div>
    )
  }

  return (
    <div className="mobile-sort-supplier-page">
      <style>{PAGE_CSS}</style>
      <div className="sticky-header">
        <div className="page-header">
          <h1>🏭 按供应商分拣</h1>
          <div className="header-info">按供应商分类汇总采购清单，便于分发给不同供应商</div>
          <div className="switch-links">
            <Link to="/mobile/sort" className="switch-link">统筹分拣</Link>
            <Link to="/mobile/sort-by-category" className="switch-link">按分类分拣</Link>
            <Link to="/mobile/sort-by-supplier" className="switch-link">按供应商分拣</Link>
            <Link to="/mobile/sort-by-purchaser" className="switch-link">按单位分拣</Link>
            <Link to="/mobile/sort-comprehensive" className="switch-link">综合分拣</Link>
            <Link
              to="/today-price"
              className="switch-link"
              style={{ background: 'rgba(255,255,255,0.45)', fontWeight: 'bold' }}
            >
              💰 今日进价
            </Link>
          </div>
          <div className="stats-bar">
            <div className="stat-item">
              <div className="stat-value">{totalCount}</div>
              <div className="stat-label">商品种类</div>
            </div>
            <div className="stat-item">
              <div className="stat-value">{checkedIds.size}</div>
              <div className="stat-label">已采购</div>
            </div>
            <div className="stat-item">
              <div className="stat-value">{totalCount - checkedIds.size}</div>
              <div className="stat-label">待采购</div>
            </div>
          </div>
        </div>

        <div className="history-bar">
          <input
            type="date"
            title="选择日期检索该日期的历史分拣，留空显示当前待分拣"
            value={historyDate}
            onChange={(e) => setHistoryDate(e.target.value)}
          />
          <button onClick={() => void loadItems()}>检索历史分拣</button>
          <button className="clear" onClick={clearHistory}>清除</button>
        </div>

        <div className="filter-bar">
          <input
            type="text"
            placeholder="搜索商品名称..."
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
          />
          <button className="clear" onClick={clearSearch}>清除</button>
        </div>
      </div>

      <div className="content-area">
        {!loaded ? (
          <div className="empty-state">
            <div className="empty-icon">📭</div>
            <div>暂无采购订单</div>
          </div>
        ) : !hasItems ? (
          <div className="empty-state">
            <div className="empty-icon">🔍</div>
            <div>没有找到匹配的商品</div>
          </div>
        ) : (
          sections.map(({ supplier, purchasers }) => {
            let totalQty = 0
            purchasers.forEach((entry) => {
              totalQty += entry.purchaser.total_quantity
            })
            return (
              <div className="category-section" key={`${supplier.supplier_name}-${supplier.supplier_id}`}>
                {purchasers.length > 0 ? (
                  <>
                    <div className="category-header cat-supplier">
                      <h3>{supplier.supplier_name}</h3>
                      <div className="category-stats">共 {totalQty.toFixed(0)} 件</div>
                    </div>
                    <div className="category-body">
                      {purchasers.map(({ purchaser, filtered }) => (
                        <div className="purchaser-section" key={`${purchaser.purchaser_name}-${purchaser.purchaser_id}`}>
                          <div className="purchaser-header">
                            <div className="purchaser-name">📍 {purchaser.purchaser_name}</div>
                            <div className="purchaser-qty">共 {purchaser.total_quantity.toFixed(0)} 件</div>
                          </div>
                          {filtered.map(renderItemCard)}
                        </div>
                      ))}
                    </div>
                  </>
                ) : null}
              </div>
            )
          })
        )}
      </div>

      <div className="bottom-bar">
        <button className="btn-select-all" onClick={toggleSelectAll}>全选</button>
        <button className="btn-clear-all" onClick={clearSelection}>清空</button>
        <button className="btn-clear-all" onClick={clearCorrections}>清除修正</button>
        <button className="btn-print" onClick={() => void saveCorrectionsToServer()}>保存修正</button>
        <button className="btn-export" onClick={() => exportExcel(false)}>导出XLSX</button>
        <button className="btn-export" onClick={() => exportExcel(true)}>导出(含数值)</button>
      </div>
    </div>
  )
}
