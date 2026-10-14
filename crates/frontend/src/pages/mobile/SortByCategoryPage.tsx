// 对应老页函数 page_mobile_sort_by_category，原路由 /mobile/sort_by_category
import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  exportSortByCategoryUrl,
  fetchSortItemsByCategory,
  saveSortCorrections,
  type SortCategory,
} from '../../api/mobile/sortByCategory'

const CHECKED_KEY = 'sort_by_category_checked_ids'
const CORRECTIONS_KEY = 'sort_by_category_corrections'

function todayLocal(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

function getCategoryClass(name: string): string {
  if (name.includes('荤鲜')) return 'cat-hunxian'
  if (name.includes('鲜蔬')) return 'cat-xianshu'
  if (name.includes('粮油') || name.includes('干调')) return 'cat-liangyou'
  if (name.includes('豆制品')) return 'cat-douzhi'
  if (name.includes('粉面')) return 'cat-fenmian'
  if (name.includes('水果')) return 'cat-shuiguo'
  return 'cat-other'
}

function loadCheckedIds(): Set<number> {
  try {
    const saved = localStorage.getItem(CHECKED_KEY)
    if (saved) return new Set(JSON.parse(saved) as number[])
  } catch {
    // 忽略损坏的本地缓存
  }
  return new Set()
}

function loadCorrections(): Record<number, number> {
  try {
    const saved = localStorage.getItem(CORRECTIONS_KEY)
    if (saved) return JSON.parse(saved) as Record<number, number>
  } catch {
    // 忽略损坏的本地缓存
  }
  return {}
}

export default function SortByCategoryPage() {
  const [categories, setCategories] = useState<SortCategory[]>([])
  const [loaded, setLoaded] = useState(false)
  const [date, setDate] = useState(todayLocal)
  const [keyword, setKeyword] = useState('')
  const [checkedIds, setCheckedIds] = useState<Set<number>>(loadCheckedIds)
  const [corrections, setCorrections] = useState<Record<number, number>>(loadCorrections)

  const loadItems = useCallback(async (d: string) => {
    try {
      const data = await fetchSortItemsByCategory(d)
      setCategories(data)
      setLoaded(true)
    } catch (e) {
      console.error('加载失败:', e)
    }
  }, [])

  // 默认加载当天数据
  useEffect(() => {
    void loadItems(todayLocal())
  }, [loadItems])

  const persistChecked = (ids: Set<number>) => {
    localStorage.setItem(CHECKED_KEY, JSON.stringify(Array.from(ids)))
  }
  const persistCorrections = (c: Record<number, number>) => {
    localStorage.setItem(CORRECTIONS_KEY, JSON.stringify(c))
  }

  const toggleCheck = (itemId: number) => {
    setCheckedIds((prev) => {
      const next = new Set(prev)
      if (next.has(itemId)) next.delete(itemId)
      else next.add(itemId)
      persistChecked(next)
      return next
    })
  }

  const toggleSelectAll = () => {
    const allIds: number[] = []
    categories.forEach((cat) => {
      if (cat.purchasers) {
        cat.purchasers.forEach((p) => {
          p.items.forEach((item) => allIds.push(item.item_id))
        })
      }
    })
    setCheckedIds((prev) => {
      const next = new Set(prev)
      if (next.size === allIds.length) next.clear()
      else allIds.forEach((id) => next.add(id))
      persistChecked(next)
      return next
    })
  }

  const clearSelection = () => {
    const next = new Set<number>()
    persistChecked(next)
    setCheckedIds(next)
  }

  const updateCorrectedQuantity = (itemId: number, value: string) => {
    const numValue = parseFloat(value)
    setCorrections((prev) => {
      const next = { ...prev }
      if (numValue && numValue > 0) next[itemId] = numValue
      else delete next[itemId]
      persistCorrections(next)
      return next
    })
  }

  const clearCorrections = () => {
    const next: Record<number, number> = {}
    persistCorrections(next)
    setCorrections(next)
  }

  const saveCorrectionsToServer = async () => {
    const keys = Object.keys(corrections)
    if (keys.length === 0) {
      window.alert('没有需要保存的修正')
      return
    }
    const payload = keys.map((k) => ({ id: parseInt(k, 10), quantity: corrections[Number(k)] }))
    try {
      const text = await saveSortCorrections(payload)
      window.alert(text)
      clearCorrections()
      void loadItems(date)
    } catch (e) {
      console.error('保存失败:', e)
      window.alert('保存失败，请重试')
    }
  }

  const clearSearch = () => setKeyword('')

  const clearHistory = () => {
    setDate('')
    setKeyword('')
    void loadItems('')
  }

  const exportExcel = () => {
    window.location.href = exportSortByCategoryUrl(date)
  }

  const stats = useMemo(() => {
    let totalCount = 0
    categories.forEach((cat) => {
      if (cat.purchasers) {
        cat.purchasers.forEach((p) => {
          totalCount += p.items.length
        })
      }
    })
    return { totalCount, checkedCount: checkedIds.size, uncheckedCount: totalCount - checkedIds.size }
  }, [categories, checkedIds])

  // 与老页 renderItems 一致：按关键词过滤，仅渲染含匹配商品的单位与分类头
  const kw = keyword.trim().toLowerCase()
  let hasItems = false
  const sections = categories.map((cat) => {
    const filteredPurchasers = (cat.purchasers ?? [])
      .map((p) => ({
        purchaser: p,
        items: p.items.filter((item) => item.product_name.toLowerCase().includes(kw)),
      }))
      .filter((x) => x.items.length > 0)
    if (filteredPurchasers.length > 0) hasItems = true
    const totalQty = filteredPurchasers.reduce((sum, x) => sum + x.purchaser.total_quantity, 0)
    return { cat, filteredPurchasers, totalQty }
  })

  return (
    <div className="sbc-page" style={{ minHeight: '100vh' }}>
      <style>{CSS}</style>
      <div className="sticky-header">
        <div className="page-header">
          <h1>🏷️ 按分类分拣</h1>
          <div className="header-info">按商品分类汇总采购清单，便于分发给不同供应商</div>
          <div className="switch-links">
            <Link to="/mobile/sort" className="switch-link">统筹分拣</Link>
            <Link to="/mobile/sort-by-purchaser" className="switch-link">按单位分拣</Link>
            <Link to="/mobile/sort-comprehensive" className="switch-link">综合分拣</Link>
            <Link to="/today-price" className="switch-link" style={{ background: 'rgba(255,255,255,0.45)', fontWeight: 'bold' }}>💰 今日进价</Link>
          </div>
          <div className="stats-bar">
            <div className="stat-item">
              <div className="stat-value">{stats.totalCount}</div>
              <div className="stat-label">商品种类</div>
            </div>
            <div className="stat-item">
              <div className="stat-value">{stats.checkedCount}</div>
              <div className="stat-label">已采购</div>
            </div>
            <div className="stat-item">
              <div className="stat-value">{stats.uncheckedCount}</div>
              <div className="stat-label">待采购</div>
            </div>
          </div>
        </div>

        <div className="history-bar">
          <input
            type="date"
            value={date}
            title="选择日期检索该日期的历史分拣，留空显示当前待分拣"
            onChange={(e) => setDate(e.target.value)}
          />
          <button onClick={() => void loadItems(date)}>检索历史分拣</button>
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
          sections.map(({ cat, filteredPurchasers, totalQty }, catIdx) => (
            <div className="category-section" key={`${cat.category_name}-${catIdx}`}>
              {filteredPurchasers.length > 0 && (
                <>
                  <div className={`category-header ${getCategoryClass(cat.category_name)}`}>
                    <h3>{cat.category_name}</h3>
                    <div className="category-stats">共 {totalQty.toFixed(0)} 件</div>
                  </div>
                  <div className="category-body">
                    {filteredPurchasers.map(({ purchaser, items }, pIdx) => (
                      <div className="purchaser-section" key={`${purchaser.purchaser_name}-${pIdx}`}>
                        <div className="purchaser-header">
                          <div className="purchaser-name">📍 {purchaser.purchaser_name}</div>
                          <div className="purchaser-qty">共 {purchaser.total_quantity.toFixed(0)} 件</div>
                        </div>
                        {items.map((item) => {
                          const isChecked = checkedIds.has(item.item_id)
                          const corrected = corrections[item.item_id]
                          const isCorrected = corrected !== undefined
                          const displayQty = isCorrected ? corrected : item.quantity
                          return (
                            <div
                              className={`sort-card${isChecked ? ' checked' : ''}`}
                              key={item.item_id}
                              onClick={() => toggleCheck(item.item_id)}
                            >
                              <div className="checkbox-wrapper">
                                <div className={`checkbox-custom${isChecked ? ' checked' : ''}`} />
                              </div>
                              <div className="item-info">
                                <div className="item-name">{item.product_name}</div>
                                <div className="item-detail">
                                  <span>{item.unit}</span>
                                  {item.remark && item.remark.trim() ? (
                                    <span style={{ color: '#d97706' }}>备注: {item.remark}</span>
                                  ) : null}
                                  {isCorrected ? (
                                    <span className="corrected-tag">修正: {item.quantity}→{displayQty}</span>
                                  ) : null}
                                </div>
                              </div>
                              <div className="quantity-badge">
                                <div className="quantity-value">{displayQty}</div>
                                <div className="quantity-unit">{item.unit}</div>
                                <input
                                  key={`${item.item_id}-${isCorrected ? corrected : 'orig'}`}
                                  type="number"
                                  min={0}
                                  step="any"
                                  className="correction-input"
                                  placeholder="修正"
                                  defaultValue={isCorrected ? corrected : ''}
                                  onBlur={(e) => updateCorrectedQuantity(item.item_id, e.target.value)}
                                  onClick={(e) => e.stopPropagation()}
                                />
                              </div>
                            </div>
                          )
                        })}
                      </div>
                    ))}
                  </div>
                </>
              )}
            </div>
          ))
        )}
      </div>

      <div className="bottom-bar">
        <button className="btn-select-all" onClick={toggleSelectAll}>全选</button>
        <button className="btn-clear-all" onClick={clearSelection}>清空</button>
        <button className="btn-clear-all" onClick={clearCorrections}>清除修正</button>
        <button className="btn-print" onClick={() => void saveCorrectionsToServer()}>保存修正</button>
        <button className="btn-export" onClick={exportExcel}>导出XLSX</button>
      </div>
    </div>
  )
}

// 老页 CSS 逐条照抄（选择器统一加 .sbc-page 作用域，避免污染全局；值未做任何改动）
const CSS = `
  .sbc-page, .sbc-page * { margin: 0; padding: 0; box-sizing: border-box; }
  .sbc-page { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif; background: #f5f7fa; }
  .sbc-page .sticky-header { position: sticky; top: 0; z-index: 100; }
  .sbc-page .page-header { background: linear-gradient(135deg, #7c3aed 0%, #a855f7 100%); color: white; padding: 16px 20px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
  .sbc-page .page-header h1 { font-size: 18px; margin: 0; font-weight: 600; }
  .sbc-page .header-info { font-size: 13px; opacity: 0.9; margin-top: 4px; }
  .sbc-page .switch-links { display: flex; gap: 8px; margin-top: 8px; }
  .sbc-page .switch-link { padding: 6px 12px; background: rgba(255,255,255,0.2); border-radius: 6px; font-size: 13px; text-decoration: none; color: white; }
  .sbc-page .switch-link:hover { background: rgba(255,255,255,0.3); }
  .sbc-page .stats-bar { display: flex; gap: 12px; margin-top: 12px; }
  .sbc-page .stat-item { background: rgba(255,255,255,0.2); padding: 8px 12px; border-radius: 8px; flex: 1; text-align: center; }
  .sbc-page .stat-value { font-size: 16px; font-weight: bold; }
  .sbc-page .stat-label { font-size: 11px; opacity: 0.8; }
  .sbc-page .content-area { padding: 12px; padding-bottom: 80px; }
  .sbc-page .category-section { border-radius: 12px; margin-bottom: 16px; overflow: hidden; box-shadow: 0 2px 8px rgba(0,0,0,0.08); }
  .sbc-page .category-header { padding: 14px 16px; display: flex; align-items: center; justify-content: space-between; }
  .sbc-page .category-header h3 { font-size: 16px; margin: 0; color: white; font-weight: 600; }
  .sbc-page .category-stats { font-size: 13px; opacity: 0.9; color: white; }
  .sbc-page .category-body { background: white; padding: 0; }
  .sbc-page .sort-card { display: flex; align-items: center; gap: 14px; padding: 14px 16px; border-bottom: 1px solid #f0f0f0; transition: background 0.2s; }
  .sbc-page .sort-card:last-child { border-bottom: none; }
  .sbc-page .sort-card:hover { background: #f9fafb; }
  .sbc-page .sort-card.checked { background: #f0fdf4; }
  .sbc-page .checkbox-wrapper { flex-shrink: 0; }
  .sbc-page .checkbox-custom { width: 26px; height: 26px; border-radius: 6px; border: 2px solid #ddd; display: flex; align-items: center; justify-content: center; cursor: pointer; transition: all 0.2s; }
  .sbc-page .checkbox-custom.checked { background: #10b981; border-color: #10b981; }
  .sbc-page .checkbox-custom.checked::after { content: '✓'; color: white; font-size: 16px; font-weight: bold; }
  .sbc-page .item-info { flex: 1; min-width: 0; }
  .sbc-page .item-name { font-size: 15px; font-weight: 600; color: #333; margin-bottom: 3px; }
  .sbc-page .item-detail { font-size: 12px; color: #666; display: flex; gap: 12px; flex-wrap: wrap; }
  .sbc-page .item-detail span { background: #f3f4f6; padding: 2px 6px; border-radius: 3px; }
  .sbc-page .quantity-badge { flex-shrink: 0; text-align: right; }
  .sbc-page .quantity-value { font-size: 18px; font-weight: bold; color: #3b82f6; }
  .sbc-page .quantity-unit { font-size: 11px; color: #666; }
  .sbc-page .filter-bar { background: white; padding: 12px; border-bottom: 1px solid #eee; display: flex; gap: 8px; }
  .sbc-page .filter-bar input { flex: 1; padding: 10px 14px; border: 1px solid #ddd; border-radius: 8px; font-size: 14px; }
  .sbc-page .filter-bar button { padding: 10px 16px; border: none; border-radius: 8px; background: #3b82f6; color: white; font-size: 14px; }
  .sbc-page .filter-bar button.clear { background: #f3f4f6; color: #666; }
  .sbc-page .history-bar { background: #ecfdf5; padding: 12px; border-bottom: 1px solid #eee; display: flex; gap: 8px; }
  .sbc-page .history-bar input { flex: 1; padding: 10px 14px; border: 1px solid #ddd; border-radius: 8px; font-size: 14px; }
  .sbc-page .history-bar button { padding: 10px 16px; border: none; border-radius: 8px; background: #10b981; color: white; font-size: 14px; white-space: nowrap; }
  .sbc-page .history-bar button.clear { background: #f3f4f6; color: #666; }
  .sbc-page .bottom-bar { background: white; padding: 6px 12px; position: fixed; bottom: 0; left: 0; right: 0; display: flex; gap: 6px; box-shadow: 0 -2px 8px rgba(0,0,0,0.05); }
  .sbc-page .bottom-bar button { flex: 1; padding: 6px; border: none; border-radius: 6px; font-size: 11px; font-weight: 600; }
  .sbc-page .btn-select-all { background: #f3f4f6; color: #333; }
  .sbc-page .btn-clear-all { background: #fee2e2; color: #dc2626; }
  .sbc-page .btn-export { background: #8b5cf6; color: white; }
  .sbc-page .empty-state { text-align: center; padding: 60px 20px; color: #999; }
  .sbc-page .empty-icon { font-size: 48px; margin-bottom: 16px; }
  .sbc-page .cat-hunxian { background: linear-gradient(135deg, #dc2626 0%, #ef4444 100%); }
  .sbc-page .cat-xianshu { background: linear-gradient(135deg, #16a34a 0%, #22c55e 100%); }
  .sbc-page .cat-liangyou { background: linear-gradient(135deg, #1d4ed8 0%, #3b82f6 100%); }
  .sbc-page .cat-douzhi { background: linear-gradient(135deg, #ca8a04 0%, #eab308 100%); }
  .sbc-page .cat-fenmian { background: linear-gradient(135deg, #64748b 0%, #94a3b8 100%); }
  .sbc-page .cat-shuiguo { background: linear-gradient(135deg, #ea580c 0%, #f97316 100%); }
  .sbc-page .cat-other { background: linear-gradient(135deg, #6b7280 0%, #9ca3af 100%); }
  .sbc-page .correction-input { width: 60px; padding: 6px; border: 1px solid #ddd; border-radius: 4px; font-size: 14px; text-align: center; }
  .sbc-page .correction-input:focus { outline: none; border-color: #3b82f6; }
  .sbc-page .corrected-tag { background: #fef3c7; color: #d97706; padding: 2px 5px; border-radius: 3px; font-size: 11px; }
  .sbc-page .purchaser-section { margin: 0 12px; border-bottom: 1px solid #f0f0f0; padding: 10px 0; }
  .sbc-page .purchaser-section:last-child { border-bottom: none; }
  .sbc-page .purchaser-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; padding: 8px 12px; background: #f8fafc; border-radius: 6px; }
  .sbc-page .purchaser-name { font-size: 14px; font-weight: 600; color: #333; }
  .sbc-page .purchaser-qty { font-size: 12px; color: #666; }
`
