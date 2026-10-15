// 按分类分拣（对应老 page_mobile_sort_by_category，原路由 /mobile/sort_by_category）
import { useEffect, useMemo, useState } from 'react'
import { Button } from 'antd'
import {
  exportSortByCategoryUrl,
  fetchSortItemsByCategory,
  saveSortCorrections,
  type SortCategory,
} from '../../api/mobile/sortByCategory'
import {
  MOBILE_ACCENT,
  MobileBottomBar,
  MobileDateBar,
  MobileEmpty,
  MobileHeader,
  MobileSearchBar,
  MobileSection,
  MobileSortRow,
  MobileSubHeader,
  categoryColor,
} from './mobileKit'
import { useSortSelection } from './useSortSelection'
import { todayStr } from '../../utils/format'

const CHECKED_KEY = 'sort_by_category_checked_ids'
const CORRECTIONS_KEY = 'sort_by_category_corrections'

export default function SortByCategoryPage() {
  const [categories, setCategories] = useState<SortCategory[]>([])
  const [loaded, setLoaded] = useState(false)
  const [date, setDate] = useState(todayStr)
  const [keyword, setKeyword] = useState('')

  const loadItems = async (d: string) => {
    try {
      setCategories(await fetchSortItemsByCategory(d))
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
    saveFn: saveSortCorrections,
    reload: () => void loadItems(date),
  })

  useEffect(() => {
    document.title = '按分类分拣'
    void loadItems(todayStr())
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const stats = useMemo(() => {
    const allIds: number[] = []
    categories.forEach((cat) => {
      cat.purchasers?.forEach((p) => p.items.forEach((item) => allIds.push(item.item_id)))
    })
    return { totalCount: allIds.length, allIds, checkedCount: checkedIds.size, uncheckedCount: allIds.length - checkedIds.size }
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
    <>
      <MobileHeader
        color={MOBILE_ACCENT.category}
        title="🏷️ 按分类分拣"
        info="按商品分类汇总采购清单，便于分发给不同供应商"
        stats={[
          { value: stats.totalCount, label: '商品种类' },
          { value: stats.checkedCount, label: '已采购' },
          { value: stats.uncheckedCount, label: '待采购' },
        ]}
      >
        <MobileDateBar
          value={date}
          onChange={setDate}
          onSearch={() => void loadItems(date)}
          onClear={() => {
            setDate('')
            setKeyword('')
            void loadItems('')
          }}
        />
        <MobileSearchBar value={keyword} onChange={setKeyword} onClear={() => setKeyword('')} />
      </MobileHeader>

      <div className="mobile-content">
        {!loaded ? (
          <MobileEmpty text="暂无采购订单" />
        ) : !hasItems ? (
          <MobileEmpty text="没有找到匹配的商品" />
        ) : (
          sections.map(({ cat, filteredPurchasers, totalQty }, catIdx) => (
            <MobileSection
              key={`${cat.category_name}-${catIdx}`}
              color={categoryColor(cat.category_name)}
              title={cat.category_name}
              extra={`共 ${totalQty.toFixed(0)} 件`}
            >
              {filteredPurchasers.map(({ purchaser, items }, pIdx) => (
                <div key={`${purchaser.purchaser_name}-${pIdx}`}>
                  <MobileSubHeader title={`📍 ${purchaser.purchaser_name}`} extra={`共 ${purchaser.total_quantity.toFixed(0)} 件`} />
                  {items.map((item) => {
                    const corrected = corrections[item.item_id]
                    const isCorrected = corrected !== undefined
                    return (
                      <MobileSortRow
                        key={item.item_id}
                        id={item.item_id}
                        checked={checkedIds.has(item.item_id)}
                        title={item.product_name}
                        unit={item.unit}
                        quantity={isCorrected ? corrected : item.quantity}
                        remark={item.remark && item.remark.trim() ? item.remark : undefined}
                        correctionText={isCorrected ? `修正: ${item.quantity}→${corrected}` : undefined}
                        corrected={isCorrected}
                        correctedRaw={corrected}
                        correctionsVersion={correctionsVersion}
                        onToggle={toggleCheck}
                        onCorrectionCommit={updateCorrection}
                      />
                    )
                  })}
                </div>
              ))}
            </MobileSection>
          ))
        )}
      </div>

      <MobileBottomBar>
        <Button onClick={() => toggleSelectAll(stats.allIds)}>全选</Button>
        <Button onClick={clearSelection}>清空</Button>
        <Button onClick={clearCorrections}>清除修正</Button>
        <Button type="primary" onClick={() => void save()}>
          保存修正
        </Button>
        <Button
          onClick={() => {
            window.location.href = exportSortByCategoryUrl(date)
          }}
        >
          导出XLSX
        </Button>
      </MobileBottomBar>
    </>
  )
}
