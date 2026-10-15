// 综合分拣（对应老页函数 page_mobile_sort_comprehensive，原路由 /mobile/sort_comprehensive）
// 采购单位折叠区改用 antd Collapse，展开态仍持久化在 sort_comprehensive_expanded
import { useEffect, useState } from 'react'
import { Button, Collapse } from 'antd'
import { fetchSortComprehensive, saveCorrections, sortComprehensiveExcelUrl } from '../../api/mobile/sortComprehensive'
import type { SortComprehensiveItem, SortComprehensivePurchaser } from '../../api/mobile/sortComprehensive'
import {
  MOBILE_ACCENT,
  MobileBottomBar,
  MobileDateBar,
  MobileEmpty,
  MobileHeader,
  MobileSearchBar,
  MobileSection,
  MobileSortRow,
  categoryColor,
} from './mobileKit'
import { useSortSelection } from './useSortSelection'
import { todayStr } from '../../utils/format'

const CHECKED_KEY = 'sort_comprehensive_checked_ids'
const EXPANDED_KEY = 'sort_comprehensive_expanded'
const CORRECTIONS_KEY = 'sort_comprehensive_corrections'

function loadIdSet(key: string): Set<number> {
  try {
    const saved = localStorage.getItem(key)
    if (saved) return new Set(JSON.parse(saved) as number[])
  } catch {
    // localStorage 数据损坏时按空处理
  }
  return new Set<number>()
}

export default function SortComprehensivePage() {
  const [purchasers, setPurchasers] = useState<SortComprehensivePurchaser[]>([])
  const [loaded, setLoaded] = useState(false)
  const [historyDate, setHistoryDate] = useState(todayStr)
  const [searchInput, setSearchInput] = useState('')
  const [expandedIds, setExpandedIds] = useState<Set<number>>(() => loadIdSet(EXPANDED_KEY))

  const loadItems = async (date: string) => {
    try {
      setPurchasers(await fetchSortComprehensive(date))
      setLoaded(true)
    } catch (e) {
      console.error('加载失败:', e)
    }
  }

  const {
    checkedIds,
    corrections,
    correctionsVersion,
    toggleCheck,
    toggleSelectAll,
    clearSelection,
    updateCorrection,
    clearCorrections,
    saveCorrections: save,
  } = useSortSelection({
    checkedKey: CHECKED_KEY,
    correctionsKey: CORRECTIONS_KEY,
    saveFn: saveCorrections,
    reload: () => void loadItems(historyDate),
  })

  useEffect(() => {
    document.title = '综合分拣'
    void loadItems(todayStr())
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const persistExpanded = (ids: Set<number>) => {
    localStorage.setItem(EXPANDED_KEY, JSON.stringify([...ids]))
  }

  // ---- 统计（与老页 updateStats 一致：checkedIds.size 可能含历史残留 id，原样保留） ----
  let totalItems = 0
  const allIds: number[] = []
  purchasers.forEach((p) => {
    p.categories.forEach((c) => {
      c.items.forEach((item) => {
        totalItems += 1
        allIds.push(item.id)
      })
    })
  })

  // ---- 渲染（与老页 renderItems 一致：按关键字过滤，隐藏无可见分类的采购单位） ----
  const keyword = searchInput.trim().toLowerCase()
  const sections = purchasers
    .map((purchaser) => {
      const visibleCats = purchaser.categories
        .map((category) => ({
          category,
          filtered: category.items.filter((item) => item.product_name.toLowerCase().includes(keyword)),
        }))
        .filter((x) => x.filtered.length > 0)
      return { purchaser, visibleCats }
    })
    .filter((s) => s.visibleCats.length > 0)

  const renderItem = (item: SortComprehensiveItem) => {
    const corrected = corrections[item.id]
    const isCorrected = corrected !== undefined
    return (
      <MobileSortRow
        key={item.id}
        id={item.id}
        checked={checkedIds.has(item.id)}
        title={item.product_name}
        unit={item.unit}
        quantity={isCorrected ? corrected : item.quantity}
        remark={item.remarks && item.remarks.length > 0 ? item.remarks.join(', ') : undefined}
        correctionText={isCorrected ? `修正: ${item.quantity}→${corrected}` : undefined}
        corrected={isCorrected}
        correctedRaw={corrected}
        correctionsVersion={correctionsVersion}
        onToggle={toggleCheck}
        onCorrectionCommit={updateCorrection}
      />
    )
  }

  return (
    <>
      <MobileHeader
        color={MOBILE_ACCENT.comprehensive}
        title="🔄 综合分拣"
        info="先按采购单位，再按商品分类汇总采购清单"
        stats={[
          { value: purchasers.length, label: '采购单位' },
          { value: checkedIds.size, label: '已采购' },
          { value: totalItems - checkedIds.size, label: '待采购' },
        ]}
      >
        <MobileDateBar
          value={historyDate}
          onChange={setHistoryDate}
          onSearch={() => void loadItems(historyDate)}
          onClear={() => {
            setHistoryDate('')
            setSearchInput('')
            void loadItems('')
          }}
        />
        <MobileSearchBar value={searchInput} onChange={setSearchInput} onClear={() => setSearchInput('')} />
      </MobileHeader>

      <div className="mobile-content">
        {sections.length > 0 ? (
          <Collapse
            className="mobile-collapse"
            activeKey={[...expandedIds].map(String)}
            onChange={(keys) => {
              const arr = Array.isArray(keys) ? keys : [keys]
              const next = new Set(arr.map(Number))
              setExpandedIds(next)
              persistExpanded(next)
            }}
            items={sections.map(({ purchaser, visibleCats }) => ({
              key: String(purchaser.purchaser_id),
              label: (
                <div className="mobile-collapse-label">
                  <span className="mobile-collapse-title">{purchaser.purchaser_name}</span>
                  <span className="mobile-collapse-extra">{purchaser.categories.length}个分类</span>
                </div>
              ),
              children: visibleCats.map(({ category, filtered }) => {
                const totalQty = filtered.reduce((sum, item) => sum + item.quantity, 0)
                return (
                  <MobileSection
                    key={category.category_id}
                    color={categoryColor(category.category_name)}
                    title={category.category_name}
                    extra={`${filtered.length}种 / ${totalQty.toFixed(0)}`}
                  >
                    {filtered.map(renderItem)}
                  </MobileSection>
                )
              }),
            }))}
          />
        ) : loaded ? (
          <MobileEmpty text="没有找到匹配的商品" />
        ) : (
          <MobileEmpty text="暂无采购订单" />
        )}
      </div>

      <MobileBottomBar>
        <Button onClick={() => toggleSelectAll(allIds)}>全选</Button>
        <Button onClick={clearSelection}>清空</Button>
        <Button onClick={clearCorrections}>清除修正</Button>
        <Button type="primary" onClick={() => void save()}>
          保存修正
        </Button>
        <Button
          onClick={() => {
            window.location.href = sortComprehensiveExcelUrl(historyDate)
          }}
        >
          导出XLSX
        </Button>
      </MobileBottomBar>
    </>
  )
}
