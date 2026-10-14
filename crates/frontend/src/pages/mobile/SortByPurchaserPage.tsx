// 对应老页函数 page_mobile_sort_by_purchaser（原路由 /mobile/sort_by_purchaser）
import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  exportSortByPurchaserUrl,
  fetchSortItemsByPurchaser,
  saveSortCorrections,
  type SortByPurchaserGroup,
} from '../../api/mobile/sortByPurchaser'

/** localStorage key 与老页完全一致 */
const CHECKED_KEY = 'sort_by_purchaser_checked_ids'
const CORRECTIONS_KEY = 'sort_by_purchaser_corrections'

/** 与老页内嵌 <style> 逐条一致 */
const PAGE_CSS = `
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif; background: #f5f7fa; }
.sticky-header { position: sticky; top: 0; z-index: 100; }
.page-header { background: linear-gradient(135deg, #059669 0%, #10b981 100%); color: white; padding: 16px 20px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
.page-header h1 { font-size: 18px; margin: 0; font-weight: 600; }
.header-info { font-size: 13px; opacity: 0.9; margin-top: 4px; }
.switch-link { display: inline-block; margin-top: 8px; padding: 6px 12px; background: rgba(255,255,255,0.2); border-radius: 6px; font-size: 13px; text-decoration: none; color: white; }
.switch-link:hover { background: rgba(255,255,255,0.3); }
.stats-bar { display: flex; gap: 12px; margin-top: 12px; }
.stat-item { background: rgba(255,255,255,0.2); padding: 8px 12px; border-radius: 8px; flex: 1; text-align: center; }
.stat-value { font-size: 16px; font-weight: bold; }
.stat-label { font-size: 11px; opacity: 0.8; }
.content-area { padding: 12px; }
.purchaser-section { margin-bottom: 16px; }
.purchaser-header { background: #3b82f6; color: white; padding: 12px 16px; border-radius: 10px 10px 0 0; display: flex; justify-content: space-between; align-items: center; }
.purchaser-header h3 { font-size: 16px; margin: 0; font-weight: 600; }
.purchaser-stats { font-size: 13px; opacity: 0.9; }
.sort-card { background: white; padding: 14px; border-bottom: 1px solid #eee; display: flex; align-items: center; gap: 12px; transition: all 0.2s; }
.sort-card:hover { background: #f9fafb; }
.sort-card.checked { background: #ecfdf5; }
.checkbox-wrapper { flex-shrink: 0; }
.checkbox-custom { width: 26px; height: 26px; border-radius: 6px; border: 2px solid #ddd; display: flex; align-items: center; justify-content: center; cursor: pointer; transition: all 0.2s; }
.checkbox-custom.checked { background: #10b981; border-color: #10b981; }
.checkbox-custom.checked::after { content: '✓'; color: white; font-size: 16px; font-weight: bold; }
.item-info { flex: 1; min-width: 0; }
.item-name { font-size: 15px; font-weight: 600; color: #333; margin-bottom: 2px; }
.item-detail { font-size: 12px; color: #666; display: flex; gap: 12px; }
.item-detail span { background: #f3f4f6; padding: 2px 6px; border-radius: 4px; }
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
.btn-print { background: #10b981; color: white; }
.empty-state { text-align: center; padding: 60px 20px; color: #999; }
.empty-icon { font-size: 48px; margin-bottom: 16px; }
.section-body { background: #fff; border-radius: 0 0 10px 10px; overflow: hidden; }
.correction-input { width: 60px; padding: 6px; border: 1px solid #ddd; border-radius: 4px; font-size: 14px; text-align: center; }
.correction-input:focus { outline: none; border-color: #3b82f6; }
.corrected-tag { background: #fef3c7; color: #d97706; padding: 2px 5px; border-radius: 3px; font-size: 11px; }
`

/** 默认历史日期 = 今天（本地时区 YYYY-MM-DD），与老页初始化逻辑一致 */
function getToday(): string {
  const d = new Date()
  return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0')
}

export default function SortByPurchaserPage() {
  const [purchasers, setPurchasers] = useState<SortByPurchaserGroup[]>([])
  const [checkedIds, setCheckedIds] = useState<Set<number>>(() => new Set())
  const [corrections, setCorrections] = useState<Record<number, number>>({})
  const [date, setDate] = useState(getToday)
  const [keyword, setKeyword] = useState('')
  /** 老页首屏静态空态（📭），首次加载成功后被列表/🔍空态替换 */
  const [loaded, setLoaded] = useState(false)
  /** 修正输入框为非受控组件；清除修正/重新加载后递增 epoch 强制重挂载以同步显示值 */
  const [epoch, setEpoch] = useState(0)

  const saveCheckedState = (ids: Set<number>) => {
    localStorage.setItem(CHECKED_KEY, JSON.stringify(Array.from(ids)))
  }
  const saveCorrectedQuantities = (map: Record<number, number>) => {
    localStorage.setItem(CORRECTIONS_KEY, JSON.stringify(map))
  }

  const loadItems = async (dateStr: string) => {
    try {
      const data = await fetchSortItemsByPurchaser(dateStr)
      setPurchasers(data)
      // 老页 loadCheckedState：把本地保存的勾选并入当前集合（不清空）
      const savedChecked = localStorage.getItem(CHECKED_KEY)
      if (savedChecked) {
        const ids = JSON.parse(savedChecked) as number[]
        const next = new Set(checkedIds)
        ids.forEach((id) => next.add(id))
        setCheckedIds(next)
      }
      // 老页 loadCorrectedQuantities：整体替换为本地保存值
      const savedCorr = localStorage.getItem(CORRECTIONS_KEY)
      if (savedCorr) {
        setCorrections(JSON.parse(savedCorr) as Record<number, number>)
      }
      setLoaded(true)
      setEpoch((e) => e + 1)
    } catch (e) {
      console.error('加载失败:', e)
    }
  }

  useEffect(() => {
    document.title = '按单位分拣'
    void loadItems(getToday())
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const clearHistory = () => {
    setDate('')
    setKeyword('')
    void loadItems('')
  }

  const updateCorrectedQuantity = (itemId: number, value: string) => {
    const numValue = parseFloat(value)
    const next = { ...corrections }
    if (numValue && numValue > 0) next[itemId] = numValue
    else delete next[itemId]
    setCorrections(next)
    saveCorrectedQuantities(next)
  }

  const clearCorrections = () => {
    const next: Record<number, number> = {}
    setCorrections(next)
    saveCorrectedQuantities(next)
    setEpoch((e) => e + 1)
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
      await loadItems(date)
    } catch (e) {
      console.error('保存失败:', e)
      window.alert('保存失败，请重试')
    }
  }

  const toggleCheck = (itemId: number) => {
    const next = new Set(checkedIds)
    if (next.has(itemId)) next.delete(itemId)
    else next.add(itemId)
    setCheckedIds(next)
    saveCheckedState(next)
  }

  const toggleSelectAll = () => {
    const allItems: number[] = []
    purchasers.forEach((p) => p.items.forEach((item) => allItems.push(item.id)))
    const next = new Set(checkedIds)
    if (next.size === allItems.length) next.clear()
    else allItems.forEach((id) => next.add(id))
    setCheckedIds(next)
    saveCheckedState(next)
  }

  const clearSelection = () => {
    const next = new Set<number>()
    setCheckedIds(next)
    saveCheckedState(next)
  }

  const clearSearch = () => setKeyword('')

  const exportExcel = () => {
    window.location.href = exportSortByPurchaserUrl(date)
  }

  // 统计逻辑与老页 updateStats 一致（待采购 = 总商品数 - 已勾选数，不做下限保护）
  let totalItems = 0
  purchasers.forEach((p) => {
    totalItems += p.items.length
  })

  // 过滤渲染逻辑与老页 renderItems 一致：按商品名小写包含过滤，空组整体跳过
  const kw = keyword.trim().toLowerCase()
  const sections = purchasers
    .map((p) => ({ purchaser: p, items: p.items.filter((it) => it.product_name.toLowerCase().includes(kw)) }))
    .filter((s) => s.items.length > 0)

  return (
    <div>
      <style>{PAGE_CSS}</style>
      <div className="sticky-header">
        <div className="page-header">
          <h1>🏢 按单位分拣</h1>
          <div className="header-info">按采购单位分组查看采购清单</div>
          <div className="switch-links">
            <Link to="/mobile/sort" className="switch-link">统筹分拣</Link>{' '}
            <Link to="/mobile/sort-by-category" className="switch-link">按分类分拣</Link>{' '}
            <Link to="/mobile/sort-comprehensive" className="switch-link">综合分拣</Link>{' '}
            <Link to="/today-price" className="switch-link" style={{ background: 'rgba(255,255,255,0.45)', fontWeight: 'bold' }}>💰 今日进价</Link>
          </div>
          <div className="stats-bar">
            <div className="stat-item">
              <div className="stat-value">{purchasers.length}</div>
              <div className="stat-label">采购单位</div>
            </div>
            <div className="stat-item">
              <div className="stat-value">{checkedIds.size}</div>
              <div className="stat-label">已采购商品</div>
            </div>
            <div className="stat-item">
              <div className="stat-value">{totalItems - checkedIds.size}</div>
              <div className="stat-label">待采购商品</div>
            </div>
          </div>
        </div>

        <div className="history-bar">
          <input
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
            title="选择日期检索该日期的历史分拣，留空显示当前待分拣"
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
        ) : sections.length === 0 ? (
          <div className="empty-state">
            <div className="empty-icon">🔍</div>
            <div>没有找到匹配的商品</div>
          </div>
        ) : (
          sections.map((s, idx) => (
            <div className="purchaser-section" key={`${s.purchaser.purchaser_name}_${idx}`}>
              <div className="purchaser-header">
                <h3>{s.purchaser.purchaser_name}</h3>
                <div className="purchaser-stats">{s.items.length}种商品</div>
              </div>
              <div className="section-body">
                {s.items.map((item) => {
                  const isChecked = checkedIds.has(item.id)
                  const isCorrected = corrections[item.id] !== undefined
                  const displayQty = isCorrected ? corrections[item.id] : item.quantity
                  return (
                    <div
                      className={'sort-card' + (isChecked ? ' checked' : '')}
                      key={item.id}
                      onClick={() => toggleCheck(item.id)}
                    >
                      <div className="checkbox-wrapper">
                        <div className={'checkbox-custom' + (isChecked ? ' checked' : '')}></div>
                      </div>
                      <div className="item-info">
                        <div className="item-name">{item.product_name}</div>
                        <div className="item-detail">
                          <span>{item.unit}</span>
                          {item.remark ? <span style={{ color: '#d97706' }}>备注: {item.remark}</span> : null}
                          {isCorrected ? (
                            <span className="corrected-tag">修正: {item.quantity}→{displayQty}</span>
                          ) : null}
                        </div>
                      </div>
                      <div className="quantity-badge">
                        <div className="quantity-value">{displayQty}</div>
                        <div className="quantity-unit">{item.unit}</div>
                        <input
                          type="number"
                          min="0"
                          step="any"
                          className="correction-input"
                          placeholder="修正"
                          key={epoch}
                          defaultValue={isCorrected ? corrections[item.id] : ''}
                          onChange={(e) => updateCorrectedQuantity(item.id, e.target.value)}
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
        <button className="btn-select-all" onClick={toggleSelectAll}>全选</button>
        <button className="btn-clear-all" onClick={clearSelection}>清空</button>
        <button className="btn-clear-all" onClick={clearCorrections}>清除修正</button>
        <button className="btn-print" onClick={() => void saveCorrectionsToServer()}>保存修正</button>
        <button className="btn-export" onClick={exportExcel}>导出XLSX</button>
      </div>
    </div>
  )
}
