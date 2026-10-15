// 移动端分拣页共享外壳：样式、布局组件与勾选/修正状态 hook
// 1:1 复刻老 page_mobile_sort* 内嵌 CSS（作用域限定在 .msr 下，避免污染全局）
import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import type { CSSProperties, ReactNode } from 'react'
import { saveCorrections } from '../../api/mobile/sort'

const SCOPED_CSS = `
.msr, .msr * { margin: 0; padding: 0; box-sizing: border-box; }
.msr { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif; background: #f5f7fa; min-height: 100vh; }
.msr .sticky-header { position: sticky; top: 0; z-index: 100; }
.msr .page-header { color: white; padding: 16px 20px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }
.msr .page-header h1 { font-size: 18px; margin: 0; font-weight: 600; }
.msr .header-info { font-size: 13px; opacity: 0.9; margin-top: 4px; }
.msr .switch-links { display: flex; gap: 8px; margin-top: 8px; flex-wrap: wrap; }
.msr .switch-link { display: inline-block; margin-top: 8px; padding: 6px 12px; background: rgba(255,255,255,0.2); border-radius: 6px; font-size: 13px; text-decoration: none; color: white; }
.msr .switch-links .switch-link { margin-top: 0; }
.msr .switch-link:hover { background: rgba(255,255,255,0.3); }
.msr .switch-link-highlight { background: rgba(255,255,255,0.45); font-weight: bold; }
.msr .stats-bar { display: flex; gap: 12px; margin-top: 12px; }
.msr .stat-item { background: rgba(255,255,255,0.2); padding: 8px 12px; border-radius: 8px; flex: 1; text-align: center; }
.msr .stat-value { font-size: 16px; font-weight: bold; }
.msr .stat-label { font-size: 11px; opacity: 0.8; }
.msr .content-area { padding: 12px; }
.msr .content-area-padded { padding-bottom: 80px; }
.msr .sort-card { background: white; border-radius: 12px; padding: 16px; margin-bottom: 12px; box-shadow: 0 2px 6px rgba(0,0,0,0.05); display: flex; align-items: center; gap: 14px; transition: all 0.2s; }
.msr .sort-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.1); }
.msr .sort-card.checked { background: #ecfdf5; border: 1px solid #10b981; }
.msr .sort-card-flat { border-radius: 0; margin-bottom: 0; box-shadow: none; padding: 14px; border-bottom: 1px solid #eee; gap: 12px; }
.msr .sort-card-flat:hover { background: #f9fafb; box-shadow: none; }
.msr .sort-card-flat.checked { background: #ecfdf5; border: none; border-bottom: 1px solid #eee; }
.msr .sort-card-list { display: flex; align-items: center; gap: 14px; padding: 14px 16px; border-bottom: 1px solid #f0f0f0; transition: background 0.2s; background: white; }
.msr .sort-card-list:last-child { border-bottom: none; }
.msr .sort-card-list:hover { background: #f9fafb; }
.msr .sort-card-list.checked { background: #f0fdf4; }
.msr .sort-card-compact { display: flex; align-items: center; gap: 12px; padding: 12px 14px; background: white; margin: 4px; border-radius: 8px; border-bottom: 1px solid #f5f5f5; transition: background 0.2s; }
.msr .sort-card-compact:last-child { border-bottom: none; }
.msr .sort-card-compact:hover { background: #f9fafb; }
.msr .sort-card-compact.checked { background: #f0fdf4; }
.msr .checkbox-wrapper { flex-shrink: 0; }
.msr .checkbox-custom { width: 28px; height: 28px; border-radius: 8px; border: 2px solid #ddd; display: flex; align-items: center; justify-content: center; cursor: pointer; transition: all 0.2s; }
.msr .checkbox-custom.checked { background: #10b981; border-color: #10b981; }
.msr .checkbox-custom.checked::after { content: '✓'; color: white; font-size: 18px; font-weight: bold; }
.msr .checkbox-26 { width: 26px; height: 26px; border-radius: 6px; }
.msr .checkbox-26.checked::after { font-size: 16px; }
.msr .checkbox-24 { width: 24px; height: 24px; border-radius: 6px; }
.msr .checkbox-24.checked::after { font-size: 14px; }
.msr .item-info { flex: 1; min-width: 0; }
.msr .item-name { font-size: 16px; font-weight: 600; color: #333; margin-bottom: 4px; }
.msr .item-name-15 { font-size: 15px; margin-bottom: 2px; }
.msr .item-name-14 { font-size: 14px; margin-bottom: 2px; }
.msr .item-detail { font-size: 13px; color: #666; display: flex; gap: 16px; flex-wrap: wrap; }
.msr .item-detail-12 { font-size: 12px; gap: 12px; }
.msr .item-detail-11 { font-size: 11px; gap: 10px; }
.msr .item-detail span { background: #f3f4f6; padding: 3px 8px; border-radius: 4px; }
.msr .item-detail-12 span { padding: 2px 6px; border-radius: 4px; }
.msr .item-detail-11 span { padding: 2px 5px; border-radius: 3px; }
.msr .quantity-badge { flex-shrink: 0; text-align: right; }
.msr .quantity-value { font-size: 20px; font-weight: bold; color: #3b82f6; }
.msr .quantity-value-18 { font-size: 18px; }
.msr .quantity-value-16 { font-size: 16px; }
.msr .quantity-unit { font-size: 12px; color: #666; }
.msr .quantity-unit-11 { font-size: 11px; }
.msr .quantity-unit-10 { font-size: 10px; }
.msr .filter-bar { background: white; padding: 12px; border-bottom: 1px solid #eee; display: flex; gap: 8px; }
.msr .filter-bar input { flex: 1; padding: 10px 14px; border: 1px solid #ddd; border-radius: 8px; font-size: 14px; }
.msr .filter-bar button { padding: 10px 16px; border: none; border-radius: 8px; background: #3b82f6; color: white; font-size: 14px; }
.msr .filter-bar button.clear { background: #f3f4f6; color: #666; }
.msr .history-bar { background: #ecfdf5; padding: 12px; border-bottom: 1px solid #eee; display: flex; gap: 8px; }
.msr .history-bar input { flex: 1; padding: 10px 14px; border: 1px solid #ddd; border-radius: 8px; font-size: 14px; }
.msr .history-bar button { padding: 10px 16px; border: none; border-radius: 8px; background: #10b981; color: white; font-size: 14px; white-space: nowrap; }
.msr .history-bar button.clear { background: #f3f4f6; color: #666; }
.msr .bottom-bar { background: white; padding: 6px 12px; position: fixed; bottom: 0; left: 0; right: 0; display: flex; gap: 6px; box-shadow: 0 -2px 8px rgba(0,0,0,0.05); }
.msr .bottom-bar button { flex: 1; padding: 6px; border: none; border-radius: 6px; font-size: 11px; font-weight: 600; }
.msr .btn-select-all { background: #f3f4f6; color: #333; }
.msr .btn-clear-all { background: #fee2e2; color: #dc2626; }
.msr .btn-print { background: #10b981; color: white; }
.msr .btn-export-green { background: #10b981; color: white; }
.msr .btn-export-purple { background: #8b5cf6; color: white; }
.msr .btn-export-cyan { background: #06b6d4; color: white; }
.msr .empty-state { text-align: center; padding: 60px 20px; color: #999; }
.msr .empty-icon { font-size: 48px; margin-bottom: 16px; }
.msr .correction-input { width: 60px; padding: 6px; border: 1px solid #ddd; border-radius: 4px; font-size: 14px; text-align: center; }
.msr .correction-input-wide { width: 92px; }
.msr .correction-input:focus { outline: none; border-color: #3b82f6; }
.msr .corrected-tag { background: #fef3c7; color: #d97706; padding: 2px 5px; border-radius: 3px; font-size: 11px; }
.msr .purchaser-section { margin-bottom: 16px; }
.msr .purchaser-header-blue { background: #3b82f6; color: white; padding: 12px 16px; border-radius: 10px 10px 0 0; display: flex; justify-content: space-between; align-items: center; }
.msr .purchaser-header-blue h3 { font-size: 16px; margin: 0; font-weight: 600; }
.msr .purchaser-stats { font-size: 13px; opacity: 0.9; }
.msr .section-body { background: #fff; border-radius: 0 0 10px 10px; overflow: hidden; }
.msr .category-section { border-radius: 12px; margin-bottom: 16px; overflow: hidden; box-shadow: 0 2px 8px rgba(0,0,0,0.08); }
.msr .category-header { padding: 14px 16px; display: flex; align-items: center; justify-content: space-between; }
.msr .category-header h3 { font-size: 16px; margin: 0; color: white; font-weight: 600; }
.msr .category-stats { font-size: 13px; opacity: 0.9; color: white; }
.msr .category-body { background: white; padding: 0; }
.msr .purchaser-subsection { margin: 0 12px; border-bottom: 1px solid #f0f0f0; padding: 10px 0; }
.msr .purchaser-subsection:last-child { border-bottom: none; }
.msr .purchaser-subheader { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; padding: 8px 12px; background: #f8fafc; border-radius: 6px; }
.msr .purchaser-name { font-size: 14px; font-weight: 600; color: #333; }
.msr .purchaser-qty { font-size: 12px; color: #666; }
.msr .expandable-section { margin-bottom: 16px; border-radius: 12px; overflow: hidden; box-shadow: 0 2px 8px rgba(0,0,0,0.08); }
.msr .expandable-header { background: #fff; padding: 14px 16px; display: flex; align-items: center; justify-content: space-between; cursor: pointer; border-bottom: 1px solid #f0f0f0; }
.msr .expandable-header h3 { font-size: 16px; margin: 0; font-weight: 600; color: #333; }
.msr .expandable-stats { font-size: 13px; color: #666; }
.msr .expand-icon { font-size: 18px; color: #999; transition: transform 0.2s; }
.msr .expand-icon.expanded { transform: rotate(180deg); }
.msr .expandable-body { background: #fafafa; }
.msr .category-row { padding: 0 12px; }
.msr .category-title { padding: 10px 14px; border-radius: 8px; margin: 8px 4px; color: white; font-size: 14px; font-weight: 600; display: flex; align-items: center; justify-content: space-between; }
.msr .cat-hunxian { background: linear-gradient(135deg, #dc2626 0%, #ef4444 100%); }
.msr .cat-xianshu { background: linear-gradient(135deg, #16a34a 0%, #22c55e 100%); }
.msr .cat-liangyou { background: linear-gradient(135deg, #1d4ed8 0%, #3b82f6 100%); }
.msr .cat-douzhi { background: linear-gradient(135deg, #ca8a04 0%, #eab308 100%); }
.msr .cat-fenmian { background: linear-gradient(135deg, #64748b 0%, #94a3b8 100%); }
.msr .cat-shuiguo { background: linear-gradient(135deg, #ea580c 0%, #f97316 100%); }
.msr .cat-other { background: linear-gradient(135deg, #6b7280 0%, #9ca3af 100%); }
.msr .cat-supplier { background: linear-gradient(135deg, #10b981 0%, #34d399 100%); }
`

/** 分类名 → 渐变样式类（与老页 getCategoryClass 一致） */
export function getCategoryClass(name: string): string {
  if (name.includes('荤鲜')) return 'cat-hunxian'
  if (name.includes('鲜蔬')) return 'cat-xianshu'
  if (name.includes('粮油') || name.includes('干调')) return 'cat-liangyou'
  if (name.includes('豆制品')) return 'cat-douzhi'
  if (name.includes('粉面')) return 'cat-fenmian'
  if (name.includes('水果')) return 'cat-shuiguo'
  return 'cat-other'
}

/** 当天日期 YYYY-MM-DD（本地时区，照抄老页逻辑） */
export function todayStr(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

export function SortStyles() {
  return <style>{SCOPED_CSS}</style>
}

export interface SwitchLink {
  label: string
  to: string
  highlight?: boolean
}

/** 页头：渐变背景 + 标题 + 切换链接 + 统计栏 */
export function PageHeader(props: {
  gradient: string
  title: string
  info: string
  links: SwitchLink[]
  stats: { value: number; label: string }[]
  /** 非导航的动作链接（如"生成采购订单"），渲染在切换链接之后 */
  actionLinks?: { label: string; onClick: () => void }[]
}) {
  return (
    <div className="page-header" style={{ background: props.gradient }}>
      <h1>{props.title}</h1>
      <div className="header-info">{props.info}</div>
      <div className="switch-links">
        {props.links.map((l) => (
          <Link key={l.to + l.label} to={l.to} className={l.highlight ? 'switch-link switch-link-highlight' : 'switch-link'}>
            {l.label}
          </Link>
        ))}
        {props.actionLinks?.map((a) => (
          <a key={a.label} className="switch-link" style={{ cursor: 'pointer' }} onClick={a.onClick}>
            {a.label}
          </a>
        ))}
      </div>
      <div className="stats-bar">
        {props.stats.map((s) => (
          <div className="stat-item" key={s.label}>
            <div className="stat-value">{s.value}</div>
            <div className="stat-label">{s.label}</div>
          </div>
        ))}
      </div>
    </div>
  )
}

/** 历史检索栏（绿色背景） */
export function HistoryBar(props: { date: string; onDateChange: (v: string) => void; onSearch: () => void; onClear: () => void }) {
  return (
    <div className="history-bar">
      <input
        type="date"
        title="选择日期检索该日期的历史分拣，留空显示当前待分拣"
        value={props.date}
        onChange={(e) => props.onDateChange(e.target.value)}
      />
      <button onClick={props.onSearch}>检索历史分拣</button>
      <button className="clear" onClick={props.onClear}>清除</button>
    </div>
  )
}

/** 商品搜索栏 */
export function FilterBar(props: { keyword: string; onKeywordChange: (v: string) => void; onClear: () => void }) {
  return (
    <div className="filter-bar">
      <input
        type="text"
        placeholder="搜索商品名称..."
        value={props.keyword}
        onChange={(e) => props.onKeywordChange(e.target.value)}
      />
      <button className="clear" onClick={props.onClear}>清除</button>
    </div>
  )
}

/** 底部固定按钮栏 */
export function BottomBar(props: { children: ReactNode }) {
  return <div className="bottom-bar">{props.children}</div>
}

export function EmptyState(props: { icon: string; text: string }) {
  return (
    <div className="empty-state">
      <div className="empty-icon">{props.icon}</div>
      <div>{props.text}</div>
    </div>
  )
}

/** 勾选框 */
export function Checkbox(props: { checked: boolean; size?: 28 | 26 | 24 }) {
  const sizeCls = props.size === 26 ? ' checkbox-26' : props.size === 24 ? ' checkbox-24' : ''
  return (
    <div className="checkbox-wrapper">
      <div className={`checkbox-custom${sizeCls}${props.checked ? ' checked' : ''}`}></div>
    </div>
  )
}

/** 修正数量输入框（非受控，与老页 onchange 行为一致） */
export function CorrectionInput(props: {
  wide?: boolean
  text?: boolean
  value: number | undefined
  onCommit: (v: string) => void
}) {
  const cls = `correction-input${props.wide ? ' correction-input-wide' : ''}`
  return (
    <input
      type={props.text ? 'text' : 'number'}
      inputMode={props.text ? 'decimal' : undefined}
      min={props.text ? undefined : 0}
      step={props.text ? undefined : 'any'}
      className={cls}
      placeholder={props.wide ? '修正数量' : '修正'}
      defaultValue={props.value !== undefined ? String(props.value) : ''}
      onChange={(e) => props.onCommit(e.target.value)}
      onClick={(e) => e.stopPropagation()}
    />
  )
}

/** 数量角标（大数字 + 单位 + 修正输入框） */
export function QuantityBadge(props: {
  displayQty: number
  unit: string
  size?: 20 | 18 | 16
  corrected: boolean
  correctedValue: number | undefined
  wide?: boolean
  text?: boolean
  onCommit: (v: string) => void
}) {
  const vCls = props.size === 18 ? 'quantity-value quantity-value-18' : props.size === 16 ? 'quantity-value quantity-value-16' : 'quantity-value'
  const uCls = props.size === 16 ? 'quantity-unit quantity-unit-10' : props.size === 18 ? 'quantity-unit quantity-unit-11' : 'quantity-unit'
  return (
    <div className="quantity-badge">
      <div className={vCls}>{props.displayQty}</div>
      <div className={uCls}>{props.unit}</div>
      <CorrectionInput wide={props.wide} text={props.text} value={props.correctedValue} onCommit={props.onCommit} />
    </div>
  )
}

/** 勾选/修正状态管理（localStorage 持久化，key 与老页一致） */
export function useSortState(checkedKey: string, correctionsKey: string, reload: () => void) {
  const [checkedIds, setCheckedIds] = useState<Set<number>>(() => {
    const saved = localStorage.getItem(checkedKey)
    return saved ? new Set<number>(JSON.parse(saved) as number[]) : new Set<number>()
  })
  const [corrections, setCorrections] = useState<Record<number, number>>(() => {
    const saved = localStorage.getItem(correctionsKey)
    return saved ? (JSON.parse(saved) as Record<number, number>) : {}
  })

  // 数据重载后老页会重新 loadCheckedState/loadCorrectedQuantities（追加合并，保持一致）
  useEffect(() => {
    const savedChecked = localStorage.getItem(checkedKey)
    if (savedChecked) {
      setCheckedIds((prev) => {
        const next = new Set(prev)
        ;(JSON.parse(savedChecked) as number[]).forEach((id) => next.add(id))
        return next
      })
    }
    const savedCorr = localStorage.getItem(correctionsKey)
    if (savedCorr) {
      setCorrections((prev) => ({ ...prev, ...(JSON.parse(savedCorr) as Record<number, number>) }))
    }
    // 仅在挂载时执行（老页 loadItems 内重复加载是幂等合并）
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const persistChecked = (next: Set<number>) => {
    setCheckedIds(next)
    localStorage.setItem(checkedKey, JSON.stringify([...next]))
  }
  const persistCorrections = (next: Record<number, number>) => {
    setCorrections(next)
    localStorage.setItem(correctionsKey, JSON.stringify(next))
  }

  const toggleCheck = (id: number) => {
    const next = new Set(checkedIds)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    persistChecked(next)
  }

  const toggleSelectAll = (allIds: number[]) => {
    if (checkedIds.size === allIds.length) persistChecked(new Set())
    else persistChecked(new Set(allIds))
  }

  const clearSelection = () => persistChecked(new Set())

  const updateCorrection = (id: number, value: string) => {
    const num = parseFloat(value)
    const next = { ...corrections }
    if (num && num > 0) next[id] = num
    else delete next[id]
    persistCorrections(next)
  }

  const clearCorrections = () => persistCorrections({})

  const saveCorrectionsToServer = async () => {
    const entries = Object.entries(corrections)
    if (entries.length === 0) {
      window.alert('没有需要保存的修正')
      return
    }
    const payload = entries.map(([id, quantity]) => ({ id: parseInt(id, 10), quantity }))
    try {
      const text = await saveCorrections(payload)
      window.alert(text)
      clearCorrections()
      reload()
    } catch (e) {
      console.error('保存失败:', e)
      window.alert('保存失败，请重试')
    }
  }

  return { checkedIds, corrections, toggleCheck, toggleSelectAll, clearSelection, updateCorrection, clearCorrections, saveCorrectionsToServer }
}

export const quantityBadgeStyle: CSSProperties = {}
