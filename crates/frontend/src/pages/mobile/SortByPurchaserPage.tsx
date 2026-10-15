// 按单位分拣（对应老 page_mobile_sort_by_purchaser，原路由 /mobile/sort_by_purchaser）
import { useEffect, useState } from 'react'
import { Button } from 'antd'
import {
  exportSortByPurchaserUrl,
  fetchSortItemsByPurchaser,
  saveSortCorrections,
  type SortByPurchaserGroup,
} from '../../api/mobile/sortByPurchaser'
import {
  MOBILE_ACCENT,
  MobileBottomBar,
  MobileDateBar,
  MobileEmpty,
  MobileHeader,
  MobileSearchBar,
  MobileSection,
  MobileSortRow,
} from './mobileKit'
import { useSortSelection } from './useSortSelection'
import { todayStr } from '../../utils/format'

/** localStorage key 与老页完全一致 */
const CHECKED_KEY = 'sort_by_purchaser_checked_ids'
const CORRECTIONS_KEY = 'sort_by_purchaser_corrections'

export default function SortByPurchaserPage() {
  const [purchasers, setPurchasers] = useState<SortByPurchaserGroup[]>([])
  const [date, setDate] = useState(todayStr)
  const [keyword, setKeyword] = useState('')
  /** 老页首屏静态空态（📭），首次加载成功后被列表/空态替换 */
  const [loaded, setLoaded] = useState(false)

  const {
    checkedIds,
    corrections,
    correctionsVersion,
    toggleCheck,
    toggleSelectAll,
    clearSelection,
    updateCorrection,
    clearCorrections,
    reloadFromStorage,
    saveCorrections: save,
  } = useSortSelection({
    checkedKey: CHECKED_KEY,
    correctionsKey: CORRECTIONS_KEY,
    // 老页本页挂载时不读缓存，而是数据加载后再同步
    initialFromStorage: false,
    saveFn: saveSortCorrections,
    reload: () => void loadItems(date),
  })

  // 函数声明（提升）：本页 loadItems 依赖 hook 的 reloadFromStorage，故置于 hook 之后
  async function loadItems(dateStr: string) {
    try {
      setPurchasers(await fetchSortItemsByPurchaser(dateStr))
      // 老页 loadCheckedState（并入当前集合）/ loadCorrectedQuantities（整体替换）
      reloadFromStorage({ mergeChecked: true, replaceCorrections: true })
      setLoaded(true)
    } catch (e) {
      console.error('加载失败:', e)
    }
  }

  useEffect(() => {
    document.title = '按单位分拣'
    void loadItems(todayStr())
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // 统计逻辑与老页 updateStats 一致（待采购 = 总商品数 - 已勾选数，不做下限保护）
  let totalItems = 0
  const allIds: number[] = []
  purchasers.forEach((p) => {
    totalItems += p.items.length
    p.items.forEach((item) => allIds.push(item.id))
  })

  // 过滤渲染逻辑与老页 renderItems 一致：按商品名小写包含过滤，空组整体跳过
  const kw = keyword.trim().toLowerCase()
  const sections = purchasers
    .map((p) => ({ purchaser: p, items: p.items.filter((it) => it.product_name.toLowerCase().includes(kw)) }))
    .filter((s) => s.items.length > 0)

  return (
    <>
      <MobileHeader
        color={MOBILE_ACCENT.purchased}
        title="🏢 按单位分拣"
        info="按采购单位分组查看采购清单"
        stats={[
          { value: purchasers.length, label: '采购单位' },
          { value: checkedIds.size, label: '已采购商品' },
          { value: totalItems - checkedIds.size, label: '待采购商品' },
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
        ) : sections.length === 0 ? (
          <MobileEmpty text="没有找到匹配的商品" />
        ) : (
          sections.map((s, idx) => (
            <MobileSection
              key={`${s.purchaser.purchaser_name}_${idx}`}
              color={MOBILE_ACCENT.purchased}
              title={s.purchaser.purchaser_name}
              extra={`${s.items.length}种商品`}
            >
              {s.items.map((item) => {
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
                    remark={item.remark || undefined}
                    correctionText={isCorrected ? `修正: ${item.quantity}→${corrected}` : undefined}
                    corrected={isCorrected}
                    correctedRaw={corrected}
                    correctionsVersion={correctionsVersion}
                    onToggle={toggleCheck}
                    onCorrectionCommit={updateCorrection}
                  />
                )
              })}
            </MobileSection>
          ))
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
            window.location.href = exportSortByPurchaserUrl(date)
          }}
        >
          导出XLSX
        </Button>
      </MobileBottomBar>
    </>
  )
}
