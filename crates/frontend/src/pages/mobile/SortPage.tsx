// 统筹分拣（对应老 page_mobile_sort）
// 逻辑层：api / localStorage 键名 / 统计口径 / 过滤规则与老页一致；表现层改用 AntD
import { useCallback, useEffect, useState } from 'react'
import { Button } from 'antd'
import { fetchSortItems, saveCorrections, sortItemsExcelUrl, type SortItem } from '../../api/mobile/sort'
import {
  MOBILE_ACCENT,
  MobileBottomBar,
  MobileDateBar,
  MobileEmpty,
  MobileHeader,
  MobileSearchBar,
  MobileSortRow,
} from './mobileKit'
import { useSortSelection } from './useSortSelection'
import { todayStr } from '../../utils/format'

export default function SortPage() {
  const [items, setItems] = useState<SortItem[]>([])
  const [date, setDate] = useState(todayStr())
  const [keyword, setKeyword] = useState('')

  const loadItems = useCallback(async (d: string) => {
    try {
      setItems(await fetchSortItems(d))
    } catch (e) {
      console.error('加载失败:', e)
    }
  }, [])

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
    checkedKey: 'sort_checked_ids',
    correctionsKey: 'sort_corrections',
    saveFn: saveCorrections,
    reload: () => void loadItems(date),
  })

  // 默认加载当天数据，并并入本地保存的勾选/修正（老页 loadCheckedState/loadCorrectedQuantities）
  useEffect(() => {
    document.title = '统筹分拣'
    void loadItems(todayStr())
    reloadFromStorage()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const kw = keyword.trim().toLowerCase()
  const filtered = items.filter((it) => it.product_name.toLowerCase().includes(kw))
  const allIds = items.map((it) => it.product_id)

  return (
    <>
      <MobileHeader
        color={MOBILE_ACCENT.sort}
        title="📦 统筹分拣"
        info="根据销售订单汇总采购清单"
        stats={[
          { value: items.length, label: '商品种类' },
          { value: checkedIds.size, label: '已采购' },
          { value: items.length - checkedIds.size, label: '待采购' },
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
        {filtered.length === 0 ? (
          <MobileEmpty text="没有找到匹配的商品" />
        ) : (
          filtered.map((item) => {
            const corrected = corrections[item.product_id]
            const isCorrected = corrected !== undefined
            return (
              <MobileSortRow
                key={item.product_id}
                id={item.product_id}
                checked={checkedIds.has(item.product_id)}
                title={item.product_name}
                unit={item.unit}
                quantity={isCorrected ? corrected : item.total_quantity}
                extraText={`采购单位: ${item.purchaser_names}`}
                remark={item.remarks ?? undefined}
                correctionText={isCorrected ? `修正: ${item.total_quantity}→${corrected}` : undefined}
                corrected={isCorrected}
                correctedRaw={corrected}
                correctionsVersion={correctionsVersion}
                onToggle={toggleCheck}
                onCorrectionCommit={updateCorrection}
              />
            )
          })
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
            window.location.href = sortItemsExcelUrl(date)
          }}
        >
          导出XLSX
        </Button>
      </MobileBottomBar>
    </>
  )
}
