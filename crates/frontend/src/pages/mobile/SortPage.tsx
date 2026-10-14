// 统筹分拣（对应老 page_mobile_sort）
import { useEffect, useState } from 'react'
import { fetchSortItems, sortItemsExcelUrl, type SortItem } from '../../api/mobile/sort'
import {
  BottomBar,
  Checkbox,
  EmptyState,
  FilterBar,
  HistoryBar,
  PageHeader,
  QuantityBadge,
  SortStyles,
  todayStr,
  useSortState,
} from './sortShared'

export default function SortPage() {
  const [items, setItems] = useState<SortItem[]>([])
  const [date, setDate] = useState(todayStr())
  const [keyword, setKeyword] = useState('')

  const loadItems = async (d: string) => {
    try {
      setItems(await fetchSortItems(d))
    } catch (e) {
      console.error('加载失败:', e)
    }
  }

  // 默认加载当天数据
  useEffect(() => {
    void loadItems(todayStr())
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const { checkedIds, corrections, toggleCheck, toggleSelectAll, clearSelection, updateCorrection, clearCorrections, saveCorrectionsToServer } =
    useSortState('sort_checked_ids', 'sort_corrections', () => void loadItems(date))

  const kw = keyword.trim().toLowerCase()
  const filtered = items.filter((it) => it.product_name.toLowerCase().includes(kw))

  return (
    <div className="msr">
      <SortStyles />
      <div className="sticky-header">
        <PageHeader
          gradient="linear-gradient(135deg, #1e3a8a 0%, #3b82f6 100%)"
          title="📦 统筹分拣"
          info="根据销售订单汇总采购清单"
          links={[
            { label: '按单位分拣', to: '/mobile/sort-by-purchaser' },
            { label: '按分类分拣', to: '/mobile/sort-by-category' },
            { label: '综合分拣', to: '/mobile/sort-comprehensive' },
            { label: '💰 今日进价', to: '/today-price', highlight: true },
          ]}
          stats={[
            { value: items.length, label: '商品种类' },
            { value: checkedIds.size, label: '已采购' },
            { value: items.length - checkedIds.size, label: '待采购' },
          ]}
        />
        <HistoryBar
          date={date}
          onDateChange={setDate}
          onSearch={() => void loadItems(date)}
          onClear={() => {
            setDate('')
            setKeyword('')
            void loadItems('')
          }}
        />
        <FilterBar keyword={keyword} onKeywordChange={setKeyword} onClear={() => setKeyword('')} />
      </div>

      <div className="content-area">
        {filtered.length === 0 ? (
          <EmptyState icon="🔍" text="没有找到匹配的商品" />
        ) : (
          filtered.map((item) => {
            const isChecked = checkedIds.has(item.product_id)
            const correctedValue = corrections[item.product_id]
            const isCorrected = correctedValue !== undefined
            const displayQty = isCorrected ? correctedValue : item.total_quantity
            return (
              <div key={item.product_id} className={`sort-card${isChecked ? ' checked' : ''}`} onClick={() => toggleCheck(item.product_id)}>
                <Checkbox checked={isChecked} />
                <div className="item-info">
                  <div className="item-name">{item.product_name}</div>
                  <div className="item-detail">
                    <span>{item.unit}</span>
                    <span>采购单位: {item.purchaser_names}</span>
                    {item.remarks ? <span style={{ color: '#d97706' }}>备注: {item.remarks}</span> : null}
                    {isCorrected ? (
                      <span className="corrected-tag">
                        修正: {item.total_quantity}→{displayQty}
                      </span>
                    ) : null}
                  </div>
                </div>
                <QuantityBadge
                  displayQty={displayQty}
                  unit={item.unit}
                  corrected={isCorrected}
                  correctedValue={correctedValue}
                  onCommit={(v) => updateCorrection(item.product_id, v)}
                />
              </div>
            )
          })
        )}
      </div>

      <BottomBar>
        <button className="btn-select-all" onClick={() => toggleSelectAll(items.map((it) => it.product_id))}>
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
        <button onClick={() => (window.location.href = sortItemsExcelUrl(date))}>导出XLSX</button>
      </BottomBar>
    </div>
  )
}
