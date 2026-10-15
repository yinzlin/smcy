// 按供应商分拣（对应老 page_mobile_sort_by_supplier，原路由 /mobile/sort_by_supplier）
import { useEffect, useState } from 'react'
import { Button } from 'antd'
import {
  exportSortBySupplierExcelUrl,
  fetchSortItemsBySupplier,
  saveSortCorrections,
  type SortBySupplierGroup,
} from '../../api/mobile/sortBySupplier'
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
} from './mobileKit'
import { useSortSelection } from './useSortSelection'
import { todayStr } from '../../utils/format'

const CHECKED_KEY = 'sort_by_supplier_checked_ids'
const CORRECTIONS_KEY = 'sort_by_supplier_corrections'

export default function SortBySupplierPage() {
  const [suppliers, setSuppliers] = useState<SortBySupplierGroup[]>([])
  const [loaded, setLoaded] = useState(false)
  const [historyDate, setHistoryDate] = useState(todayStr)
  const [keyword, setKeyword] = useState('')

  const loadItems = async (date: string) => {
    try {
      setSuppliers(await fetchSortItemsBySupplier(date))
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
    reload: () => void loadItems(historyDate),
  })

  useEffect(() => {
    document.title = '按供应商分拣'
    void loadItems(todayStr())
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const exportExcel = (withValues: boolean) => {
    window.location.href = exportSortBySupplierExcelUrl(historyDate, withValues)
  }

  // 统计（与老页 updateStats 一致：total 为全部条目数，不按搜索过滤）
  let totalCount = 0
  const allIds: number[] = []
  suppliers.forEach((supplier) => {
    supplier.purchasers?.forEach((purchaser) => {
      purchaser.items.forEach((item) => {
        totalCount += 1
        allIds.push(item.item_id)
      })
    })
  })

  // 渲染数据（与老页 renderItems 一致：按关键字过滤商品，无匹配的供应商整体不渲染）
  const kw = keyword.trim().toLowerCase()
  const sections = suppliers.map((supplier) => ({
    supplier,
    purchasers: (supplier.purchasers ?? [])
      .map((purchaser) => ({
        purchaser,
        filtered: purchaser.items.filter((item) => item.product_name.toLowerCase().includes(kw)),
      }))
      .filter((entry) => entry.filtered.length > 0),
  }))
  const hasItems = sections.some((s) => s.purchasers.length > 0)

  return (
    <>
      <MobileHeader
        color={MOBILE_ACCENT.supplier}
        title="🏭 按供应商分拣"
        info="按供应商分类汇总采购清单，便于分发给不同供应商"
        stats={[
          { value: totalCount, label: '商品种类' },
          { value: checkedIds.size, label: '已采购' },
          { value: totalCount - checkedIds.size, label: '待采购' },
        ]}
      >
        <MobileDateBar
          value={historyDate}
          onChange={setHistoryDate}
          onSearch={() => void loadItems(historyDate)}
          onClear={() => {
            setHistoryDate('')
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
          sections.map(({ supplier, purchasers }) => {
            if (purchasers.length === 0) return null
            const totalQty = purchasers.reduce((sum, entry) => sum + entry.purchaser.total_quantity, 0)
            return (
              <MobileSection
                key={`${supplier.supplier_name}-${supplier.supplier_id}`}
                color={MOBILE_ACCENT.supplier}
                title={supplier.supplier_name}
                extra={`共 ${totalQty.toFixed(0)} 件`}
              >
                {purchasers.map(({ purchaser, filtered }) => (
                  <div key={`${purchaser.purchaser_name}-${purchaser.purchaser_id}`}>
                    <MobileSubHeader title={`📍 ${purchaser.purchaser_name}`} extra={`共 ${purchaser.total_quantity.toFixed(0)} 件`} />
                    {filtered.map((item) => {
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
                          extraText={`单价 ¥${(item.unit_price || 0).toFixed(2)}`}
                          remark={item.remark && item.remark.trim() ? item.remark : undefined}
                          correctionText={isCorrected ? `数量修正: ${item.quantity}→${corrected}` : undefined}
                          corrected={isCorrected}
                          correctedRaw={corrected}
                          inputWide
                          inputText
                          correctionsVersion={correctionsVersion}
                          onToggle={toggleCheck}
                          onCorrectionCommit={updateCorrection}
                        />
                      )
                    })}
                  </div>
                ))}
              </MobileSection>
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
        <Button onClick={() => exportExcel(false)}>导出XLSX</Button>
        <Button onClick={() => exportExcel(true)}>导出(含数值)</Button>
      </MobileBottomBar>
    </>
  )
}
