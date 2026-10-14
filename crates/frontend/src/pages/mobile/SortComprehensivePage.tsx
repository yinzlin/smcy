// 对应老页函数 page_mobile_sort_comprehensive，原路由 /mobile/sort_comprehensive
import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  fetchSortComprehensive,
  saveCorrections,
  sortComprehensiveExcelUrl,
} from '../../api/mobile/sortComprehensive'
import type { SortComprehensiveItem, SortComprehensivePurchaser } from '../../api/mobile/sortComprehensive'

const CHECKED_KEY = 'sort_comprehensive_checked_ids'
const EXPANDED_KEY = 'sort_comprehensive_expanded'
const CORRECTIONS_KEY = 'sort_comprehensive_corrections'

/** 与老页 <style> 完全一致（.history-bar / .btn-print 老页本就无样式定义，保持原样） */
const PAGE_CSS = `
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif; background: #f5f7fa; }
.sticky-header { position: sticky; top: 0; z-index: 100; }
.page-header { background: linear-gradient(135deg, #06b6d4 0%, #0ea5e9 100%); color: white; padding: 16px 20px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
.page-header h1 { font-size: 18px; margin: 0; font-weight: 600; }
.header-info { font-size: 13px; opacity: 0.9; margin-top: 4px; }
.switch-links { display: flex; gap: 8px; margin-top: 8px; flex-wrap: wrap; }
.switch-link { padding: 6px 12px; background: rgba(255,255,255,0.2); border-radius: 6px; font-size: 13px; text-decoration: none; color: white; }
.switch-link:hover { background: rgba(255,255,255,0.3); }
.stats-bar { display: flex; gap: 12px; margin-top: 12px; }
.stat-item { background: rgba(255,255,255,0.2); padding: 8px 12px; border-radius: 8px; flex: 1; text-align: center; }
.stat-value { font-size: 16px; font-weight: bold; }
.stat-label { font-size: 11px; opacity: 0.8; }
.content-area { padding: 12px; padding-bottom: 80px; }
.filter-bar { background: white; padding: 12px; border-bottom: 1px solid #eee; display: flex; gap: 8px; }
.filter-bar input { flex: 1; padding: 10px 14px; border: 1px solid #ddd; border-radius: 8px; font-size: 14px; }
.filter-bar button { padding: 10px 16px; border: none; border-radius: 8px; background: #3b82f6; color: white; font-size: 14px; }
.filter-bar button.clear { background: #f3f4f6; color: #666; }
.purchaser-section { margin-bottom: 16px; border-radius: 12px; overflow: hidden; box-shadow: 0 2px 8px rgba(0,0,0,0.08); }
.purchaser-header { background: #fff; padding: 14px 16px; display: flex; align-items: center; justify-content: space-between; cursor: pointer; border-bottom: 1px solid #f0f0f0; }
.purchaser-header h3 { font-size: 16px; margin: 0; font-weight: 600; color: #333; }
.purchaser-stats { font-size: 13px; color: #666; }
.expand-icon { font-size: 18px; color: #999; transition: transform 0.2s; }
.expand-icon.expanded { transform: rotate(180deg); }
.purchaser-body { background: #fafafa; }
.category-row { padding: 0 12px; }
.category-title { padding: 10px 14px; border-radius: 8px; margin: 8px 4px; color: white; font-size: 14px; font-weight: 600; display: flex; align-items: center; justify-content: space-between; }
.sort-card { display: flex; align-items: center; gap: 12px; padding: 12px 14px; background: white; margin: 4px; border-radius: 8px; border-bottom: 1px solid #f5f5f5; transition: background 0.2s; }
.sort-card:last-child { border-bottom: none; }
.sort-card:hover { background: #f9fafb; }
.sort-card.checked { background: #f0fdf4; }
.checkbox-wrapper { flex-shrink: 0; }
.checkbox-custom { width: 24px; height: 24px; border-radius: 6px; border: 2px solid #ddd; display: flex; align-items: center; justify-content: center; cursor: pointer; transition: all 0.2s; }
.checkbox-custom.checked { background: #10b981; border-color: #10b981; }
.checkbox-custom.checked::after { content: '✓'; color: white; font-size: 14px; font-weight: bold; }
.item-info { flex: 1; min-width: 0; }
.item-name { font-size: 14px; font-weight: 600; color: #333; margin-bottom: 2px; }
.item-detail { font-size: 11px; color: #666; display: flex; gap: 10px; flex-wrap: wrap; }
.item-detail span { background: #f3f4f6; padding: 2px 5px; border-radius: 3px; }
.quantity-badge { flex-shrink: 0; text-align: right; }
.quantity-value { font-size: 16px; font-weight: bold; color: #3b82f6; }
.quantity-unit { font-size: 10px; color: #666; }
.correction-input { width: 60px; padding: 6px; border: 1px solid #ddd; border-radius: 4px; font-size: 14px; text-align: center; }
.correction-input:focus { outline: none; border-color: #3b82f6; }
.correction-label { font-size: 11px; color: #666; margin-top: 2px; }
.corrected-tag { background: #fef3c7; color: #d97706; padding: 2px 5px; border-radius: 3px; font-size: 11px; }
.bottom-bar { background: white; padding: 6px 12px; position: fixed; bottom: 0; left: 0; right: 0; display: flex; gap: 6px; box-shadow: 0 -2px 8px rgba(0,0,0,0.05); }
.bottom-bar button { flex: 1; padding: 6px; border: none; border-radius: 6px; font-size: 11px; font-weight: 600; }
.btn-select-all { background: #f3f4f6; color: #333; }
.btn-clear-all { background: #fee2e2; color: #dc2626; }
.btn-export { background: #06b6d4; color: white; }
.empty-state { text-align: center; padding: 60px 20px; color: #999; }
.empty-icon { font-size: 48px; margin-bottom: 16px; }
.cat-hunxian { background: linear-gradient(135deg, #dc2626 0%, #ef4444 100%); }
.cat-xianshu { background: linear-gradient(135deg, #16a34a 0%, #22c55e 100%); }
.cat-liangyou { background: linear-gradient(135deg, #1d4ed8 0%, #3b82f6 100%); }
.cat-douzhi { background: linear-gradient(135deg, #ca8a04 0%, #eab308 100%); }
.cat-fenmian { background: linear-gradient(135deg, #64748b 0%, #94a3b8 100%); }
.cat-shuiguo { background: linear-gradient(135deg, #ea580c 0%, #f97316 100%); }
.cat-other { background: linear-gradient(135deg, #6b7280 0%, #9ca3af 100%); }
`

function todayStr(): string {
  const d0 = new Date()
  return (
    d0.getFullYear() +
    '-' +
    String(d0.getMonth() + 1).padStart(2, '0') +
    '-' +
    String(d0.getDate()).padStart(2, '0')
  )
}

function loadIdSet(key: string): Set<number> {
  try {
    const saved = localStorage.getItem(key)
    if (saved) return new Set(JSON.parse(saved) as number[])
  } catch {
    // localStorage 数据损坏时按空处理
  }
  return new Set()
}

function loadCorrections(): Record<number, number> {
  try {
    const saved = localStorage.getItem(CORRECTIONS_KEY)
    if (saved) return JSON.parse(saved) as Record<number, number>
  } catch {
    // localStorage 数据损坏时按空处理
  }
  return {}
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

export default function SortComprehensivePage() {
  const [purchasers, setPurchasers] = useState<SortComprehensivePurchaser[]>([])
  const [loaded, setLoaded] = useState(false)
  const [historyDate, setHistoryDate] = useState(todayStr)
  const [searchInput, setSearchInput] = useState('')
  const [checkedIds, setCheckedIds] = useState<Set<number>>(() => loadIdSet(CHECKED_KEY))
  const [expandedPurchasers, setExpandedPurchasers] = useState<Set<number>>(() => loadIdSet(EXPANDED_KEY))
  const [correctedQuantities, setCorrectedQuantities] = useState<Record<number, number>>(loadCorrections)

  const loadItems = async (date: string) => {
    try {
      const data = await fetchSortComprehensive(date)
      setPurchasers(data)
      setLoaded(true)
    } catch (e) {
      console.error('加载失败:', e)
    }
  }

  useEffect(() => {
    document.title = '综合分拣'
    // 与老页一致：默认当天日期，进入页面即按当天加载
    void loadItems(todayStr())
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const saveCheckedState = (ids: Set<number>) => {
    localStorage.setItem(CHECKED_KEY, JSON.stringify(Array.from(ids)))
  }
  const saveExpandedState = (ids: Set<number>) => {
    localStorage.setItem(EXPANDED_KEY, JSON.stringify(Array.from(ids)))
  }
  const saveCorrectedQuantities = (c: Record<number, number>) => {
    localStorage.setItem(CORRECTIONS_KEY, JSON.stringify(c))
  }

  const toggleCheck = (itemId: number) => {
    setCheckedIds((prev) => {
      const next = new Set(prev)
      if (next.has(itemId)) next.delete(itemId)
      else next.add(itemId)
      saveCheckedState(next)
      return next
    })
  }

  const toggleExpand = (purchaserId: number) => {
    setExpandedPurchasers((prev) => {
      const next = new Set(prev)
      if (next.has(purchaserId)) next.delete(purchaserId)
      else next.add(purchaserId)
      saveExpandedState(next)
      return next
    })
  }

  const toggleSelectAll = () => {
    const allIds: number[] = []
    purchasers.forEach((p) => {
      p.categories.forEach((c) => {
        c.items.forEach((item) => allIds.push(item.id))
      })
    })
    setCheckedIds((prev) => {
      const next = new Set(prev)
      if (next.size === allIds.length) next.clear()
      else allIds.forEach((id) => next.add(id))
      saveCheckedState(next)
      return next
    })
  }

  const clearSelection = () => {
    const next = new Set<number>()
    setCheckedIds(next)
    saveCheckedState(next)
  }

  const updateCorrectedQuantity = (itemId: number, value: string) => {
    const numValue = parseFloat(value)
    setCorrectedQuantities((prev) => {
      const next = { ...prev }
      if (numValue && numValue > 0) next[itemId] = numValue
      else delete next[itemId]
      saveCorrectedQuantities(next)
      return next
    })
  }

  const clearCorrections = () => {
    setCorrectedQuantities({})
    saveCorrectedQuantities({})
  }

  const saveCorrectionsToServer = async () => {
    const keys = Object.keys(correctedQuantities)
    if (keys.length === 0) {
      window.alert('没有需要保存的修正')
      return
    }
    const corrections = keys.map((k) => ({ id: parseInt(k, 10), quantity: correctedQuantities[Number(k)] }))
    try {
      const text = await saveCorrections(corrections)
      window.alert(text)
      clearCorrections()
      void loadItems(historyDate)
    } catch (e) {
      console.error('保存失败:', e)
      window.alert('保存失败，请重试')
    }
  }

  const clearSearch = () => setSearchInput('')

  const clearHistory = () => {
    setHistoryDate('')
    setSearchInput('')
    void loadItems('')
  }

  const exportExcel = () => {
    window.location.href = sortComprehensiveExcelUrl(historyDate)
  }

  const getDisplayQuantity = (item: SortComprehensiveItem): number =>
    correctedQuantities[item.id] !== undefined ? correctedQuantities[item.id] : item.quantity

  // ---- 统计（与老页 updateStats 一致：checkedIds.size 可能含历史残留 id，原样保留） ----
  let totalItems = 0
  purchasers.forEach((p) => {
    p.categories.forEach((c) => {
      totalItems += c.items.length
    })
  })

  // ---- 渲染（与老页 renderItems 一致：按关键字过滤，隐藏无可见分类的采购单位） ----
  const keyword = searchInput.trim().toLowerCase()
  let hasItems = false
  const sections = purchasers.map((purchaser) => {
    const visibleCats = purchaser.categories
      .map((category) => ({
        category,
        filtered: category.items.filter((item) => item.product_name.toLowerCase().includes(keyword)),
      }))
      .filter((x) => x.filtered.length > 0)

    if (visibleCats.length === 0) return null
    hasItems = true
    const isExpanded = expandedPurchasers.has(purchaser.purchaser_id)

    return (
      <div className="purchaser-section" key={purchaser.purchaser_id}>
        <div className="purchaser-header" onClick={() => toggleExpand(purchaser.purchaser_id)}>
          <h3>{purchaser.purchaser_name}</h3>
          <div className="purchaser-stats">{purchaser.categories.length}个分类</div>
          <div className={isExpanded ? 'expand-icon expanded' : 'expand-icon'}>▼</div>
        </div>
        {isExpanded && (
          <div className="purchaser-body">
            {visibleCats.map(({ category, filtered }) => {
              const totalQty = filtered.reduce((sum, item) => sum + item.quantity, 0)
              return (
                <div className="category-row" key={category.category_id}>
                  <div className={`category-title ${getCategoryClass(category.category_name)}`}>
                    <span>{category.category_name}</span>
                    <span>
                      {filtered.length}种 / {totalQty.toFixed(0)}
                    </span>
                  </div>
                  {filtered.map((item) => {
                    const isChecked = checkedIds.has(item.id)
                    const displayQty = getDisplayQuantity(item)
                    const isCorrected = correctedQuantities[item.id] !== undefined
                    return (
                      <div
                        className={isChecked ? 'sort-card checked' : 'sort-card'}
                        key={item.id}
                        onClick={() => toggleCheck(item.id)}
                      >
                        <div className="checkbox-wrapper">
                          <div className={isChecked ? 'checkbox-custom checked' : 'checkbox-custom'} />
                        </div>
                        <div className="item-info">
                          <div className="item-name">{item.product_name}</div>
                          <div className="item-detail">
                            <span>{item.unit}</span>
                            {item.remarks && item.remarks.length > 0 && (
                              <span style={{ color: '#d97706' }}>备注: {item.remarks.join(', ')}</span>
                            )}
                            {isCorrected && (
                              <span className="corrected-tag">
                                修正: {item.quantity}→{displayQty}
                              </span>
                            )}
                          </div>
                        </div>
                        <div className="quantity-badge">
                          <div className="quantity-value">{displayQty}</div>
                          <div className="quantity-unit">{item.unit}</div>
                          <input
                            key={`${item.id}-${isCorrected ? correctedQuantities[item.id] : 'orig'}`}
                            type="number"
                            min="0"
                            step="any"
                            className="correction-input"
                            placeholder="修正"
                            defaultValue={isCorrected ? correctedQuantities[item.id] : ''}
                            onClick={(e) => e.stopPropagation()}
                            onBlur={(e) => updateCorrectedQuantity(item.id, e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') updateCorrectedQuantity(item.id, (e.target as HTMLInputElement).value)
                            }}
                          />
                        </div>
                      </div>
                    )
                  })}
                </div>
              )
            })}
          </div>
        )}
      </div>
    )
  })

  return (
    <div>
      <style>{PAGE_CSS}</style>
      <div className="sticky-header">
        <div className="page-header">
          <h1>🔄 综合分拣</h1>
          <div className="header-info">先按采购单位，再按商品分类汇总采购清单</div>
          <div className="switch-links">
            <Link to="/mobile/sort" className="switch-link">
              统筹分拣
            </Link>
            <Link to="/mobile/sort-by-purchaser" className="switch-link">
              按单位分拣
            </Link>
            <Link to="/mobile/sort-by-category" className="switch-link">
              按分类分拣
            </Link>
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
              <div className="stat-value">{purchasers.length}</div>
              <div className="stat-label">采购单位</div>
            </div>
            <div className="stat-item">
              <div className="stat-value">{checkedIds.size}</div>
              <div className="stat-label">已采购</div>
            </div>
            <div className="stat-item">
              <div className="stat-value">{totalItems - checkedIds.size}</div>
              <div className="stat-label">待采购</div>
            </div>
          </div>
        </div>

        <div className="history-bar">
          <input
            type="date"
            value={historyDate}
            title="选择日期检索该日期的历史分拣，留空显示当前待分拣"
            onChange={(e) => setHistoryDate(e.target.value)}
          />
          <button onClick={() => void loadItems(historyDate)}>检索历史分拣</button>
          <button className="clear" onClick={clearHistory}>
            清除
          </button>
        </div>

        <div className="filter-bar">
          <input
            type="text"
            value={searchInput}
            placeholder="搜索商品名称..."
            onChange={(e) => setSearchInput(e.target.value)}
          />
          <button className="clear" onClick={clearSearch}>
            清除
          </button>
        </div>
      </div>

      <div className="content-area">
        {hasItems ? (
          sections
        ) : loaded ? (
          <div className="empty-state">
            <div className="empty-icon">🔍</div>
            <div>没有找到匹配的商品</div>
          </div>
        ) : (
          <div className="empty-state">
            <div className="empty-icon">📭</div>
            <div>暂无采购订单</div>
          </div>
        )}
      </div>

      <div className="bottom-bar">
        <button className="btn-select-all" onClick={toggleSelectAll}>
          全选
        </button>
        <button className="btn-clear-all" onClick={clearSelection}>
          清空
        </button>
        <button className="btn-clear-all" onClick={clearCorrections}>
          清除修正
        </button>
        <button className="btn-print" onClick={() => void saveCorrectionsToServer()}>
          保存修正
        </button>
        <button className="btn-export" onClick={exportExcel}>
          导出XLSX
        </button>
      </div>
    </div>
  )
}
