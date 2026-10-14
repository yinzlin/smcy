import { useEffect, useRef, useState } from 'react'
import { App, AutoComplete, Button, Input, InputNumber, Popconfirm, Select } from 'antd'
import { getLastPurchasePrice, getProductUnits, searchProducts, type ProductSearchItem, type UnitOption } from '../../api/purchase'
import { lookupPriceScheduleBatch, type SchedulePriceEntry } from '../../api/sales'
import useIsMobile from '../../hooks/useIsMobile'

export interface SalesEditItem {
  key: number
  product_id: number
  product_name: string
  alias1: string
  alias2: string
  spec: string
  unit: string
  base_unit: string
  /** 商品编辑页的默认基础单价：非价格策略时段回退到该值 */
  default_base_price: number
  unit_price: number
  quantity: number
  pre_sale_quantity: number
  base_quantity: number
  ratio: number
  amount: number
  supplier_id: number
  supplier_name: string
  remark: string
  units: UnitOption[]
}

let keySeq = 1
export function newBlankSalesItem(): SalesEditItem {
  return {
    key: keySeq++,
    product_id: 0,
    product_name: '',
    alias1: '',
    alias2: '',
    spec: '',
    unit: '',
    base_unit: '',
    default_base_price: 0,
    unit_price: 0,
    quantity: 0,
    pre_sale_quantity: 0,
    base_quantity: 0,
    ratio: 1,
    amount: 0,
    supplier_id: 0,
    supplier_name: '',
    remark: '',
    units: [],
  }
}

const round2 = (v: number) => Math.round(v * 100) / 100

/** 价格策略取价规则（与 selectProduct/onOrderDateChange 一致）：政采价(>0) 优先，否则商超1/2/3/AI 非零平均价 */
export function scheduleUnitPrice(p: SchedulePriceEntry | null | undefined): number {
  if (!p) return 0
  if ((p.gov_procurement || 0) > 0) return round2(p.gov_procurement)
  const vals = [p.supermarket_1, p.supermarket_2, p.supermarket_3, p.ai_realtime].filter((v) => (v || 0) > 0)
  if (vals.length === 0) return 0
  return round2(vals.reduce((a, b) => a + b, 0) / vals.length)
}

/** 移动端卡片字段：label 在上的紧凑布局（chrome87 兼容的 flex 布局） */
function MobileField({
  label,
  children,
  style,
}: {
  label: string
  children: React.ReactNode
  style?: React.CSSProperties
}) {
  return (
    <div style={{ minWidth: 0, marginBottom: 8, ...style }}>
      <div style={{ fontSize: 12, color: '#888', marginBottom: 2 }}>{label}</div>
      {children}
    </div>
  )
}

interface Props {
  items: SalesEditItem[]
  onChange: (items: SalesEditItem[]) => void
  orderDate: string
  /** 明细行指定供应商（用于生成采购单），仅已审核供应商 */
  suppliers: { id: number; name: string }[]
}

export default function SalesItemsEditor({ items, onChange, orderDate, suppliers }: Props) {
  const { modal } = App.useApp()
  const isMobile = useIsMobile()

  const containerRef = useRef<HTMLDivElement>(null)
  const itemsRef = useRef(items)
  useEffect(() => {
    itemsRef.current = items
  }, [items])

  const [searchMap, setSearchMap] = useState<Record<number, ProductSearchItem[]>>({})
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const dropdownOpen = useRef<Record<number, boolean>>({})

  const patch = (index: number, partial: Partial<SalesEditItem>) => {
    onChange(itemsRef.current.map((it, j) => (j === index ? { ...it, ...partial } : it)))
  }

  const removeItem = (index: number) => {
    onChange(itemsRef.current.filter((_, j) => j !== index))
  }

  const addRow = () => {
    onChange([...itemsRef.current, newBlankSalesItem()])
  }

  // 同列上下移动焦点（WPS 风格）
  const focusCell = (rowIdx: number, cellIdx: number) => {
    const tbody = containerRef.current?.querySelector('tbody')
    const row = tbody?.rows[rowIdx]
    const target = row?.cells[cellIdx]?.querySelector('input, select')
    if (target instanceof HTMLElement) {
      target.focus()
      if (target instanceof HTMLInputElement) target.select()
    }
  }

  const handleEnterNav = (e: React.KeyboardEvent, index: number) => {
    if (e.key !== 'Enter') return
    e.preventDefault()
    const td = (e.currentTarget as HTMLElement).closest('td')
    if (!td) return
    focusCell(index + 1, td.cellIndex)
  }

  const focusProductInput = (rowIdx: number) => {
    setTimeout(() => {
      const el = containerRef.current?.querySelector(`[data-product-input="${rowIdx}"] input`)
      if (el instanceof HTMLInputElement) {
        el.focus()
        el.select()
      }
    }, 0)
  }

  const handleProductSearch = (index: number, keyword: string) => {
    if (searchTimer.current) clearTimeout(searchTimer.current)
    const kw = keyword.trim()
    if (!kw) {
      setSearchMap((m) => ({ ...m, [index]: [] }))
      return
    }
    searchTimer.current = setTimeout(async () => {
      try {
        const products = await searchProducts(kw)
        setSearchMap((m) => ({ ...m, [index]: products }))
      } catch {
        // 搜索失败保持现状
      }
    }, 300)
  }

  // 商品下拉选项（桌面/移动共用）
  const productOptions = (index: number) =>
    (searchMap[index] ?? []).map((p) => ({
      value: String(p.id),
      label: (
        <div>
          <strong>{p.name}</strong>
          {p.spec ? ` (${p.spec})` : ''}
          {(p.alias1 || p.alias2) && (
            <div style={{ fontSize: 12, color: '#888' }}>
              {[p.alias1 && `别称1: ${p.alias1}`, p.alias2 && `别称2: ${p.alias2}`].filter(Boolean).join(', ')}
            </div>
          )}
          {p.category_name && <div style={{ fontSize: 12, color: '#aaa' }}>分类: {p.category_name}</div>}
        </div>
      ),
    }))

  const handleProductNameChange = (index: number, v: string) => {
    if ((searchMap[index] ?? []).some((p) => String(p.id) === v)) return
    const cur = itemsRef.current[index]
    patch(index, {
      product_name: v,
      ...(cur.product_id > 0 && v !== cur.product_name ? { product_id: 0 } : {}),
    })
    handleProductSearch(index, v)
  }

  const handleProductSelect = (index: number, v: string) => {
    const p = (searchMap[index] ?? []).find((x) => String(x.id) === v)
    if (!p) return
    // 移动端只做数据更新；桌面 wrapper 额外做焦点跳转
    if (isMobile) void applyProductCore(index, p)
    else void applyProduct(index, p)
  }

  // 选中商品：默认基础单价 → 按订单日期查时段价 → 零售价低于最近采购价提醒 → 多单位
  // （纯数据更新部分，桌面/移动共用；焦点跳转拆分到桌面 wrapper applyProduct）
  const applyProductCore = async (index: number, p: ProductSearchItem) => {
    // 同商品重复提示：订单中已有该商品时，确认后才能再增加一条记录
    const dupIdx = itemsRef.current.findIndex((it, j) => j !== index && it.product_id === p.id)
    if (dupIdx >= 0) {
      const ok = await new Promise<boolean>((resolve) => {
        modal.confirm({
          title: `该订单已有商品【${p.name}】（第${dupIdx + 1}行），是否确认增加记录？`,
          okText: '确定',
          cancelText: '取消',
          onOk: () => resolve(true),
          onCancel: () => resolve(false),
        })
      })
      if (!ok) return
    }

    // 销售订单售价取基础单位对应的基础单价（基础单价=计算售价，在商品管理同步）
    const defaultBasePrice = round2(p.base_price || p.selling_price || 0)
    patch(index, {
      product_id: p.id,
      product_name: p.name,
      alias1: p.alias1 ?? '',
      alias2: p.alias2 ?? '',
      spec: p.spec ?? '',
      unit: p.base_unit,
      base_unit: p.base_unit,
      default_base_price: defaultBasePrice,
      unit_price: defaultBasePrice,
      ratio: 1,
      amount: round2((itemsRef.current[index].quantity || 0) * defaultBasePrice),
    })

    // 按订单日期拉取该商品的价格策略时段价，命中时用该时段策略价作为售价
    if (orderDate) {
      try {
        const jd = await lookupPriceScheduleBatch(orderDate, [p.id])
        if (jd.success) {
          const sp = scheduleUnitPrice(jd.prices[String(p.id)])
          if (sp > 0) {
            patch(index, {
              unit_price: sp,
              amount: round2((itemsRef.current[index].quantity || 0) * sp),
            })
          }
        }
      } catch {
        // 时段价查询失败不阻塞开单
      }
    }

    // 销售单：本次零售价对比最近采购价（同基础单位），低于则提醒
    const curPrice = itemsRef.current[index].unit_price
    try {
      const lp = await getLastPurchasePrice(p.id)
      const lastPrice = lp.purchase_price || 0
      if (lastPrice > 0 && lp.base_unit === p.base_unit && curPrice < lastPrice) {
        await new Promise<void>((resolve) => {
          modal.confirm({
            title: '价格提示',
            content: (
              <div style={{ whiteSpace: 'pre-line' }}>
                {`商品：${p.name}\n最近采购价（基础单位 ${lp.base_unit}）：${lastPrice.toFixed(2)}\n本次零售价：${curPrice.toFixed(2)}\n零售价低于采购价 ${(lastPrice - curPrice).toFixed(2)}\n\n是否继续？`}
              </div>
            ),
            okText: '确定',
            cancelText: '取消',
            onOk: () => resolve(),
            onCancel: () => resolve(),
          })
        })
      }
    } catch {
      // 价格对比失败不阻塞
    }

    // 多单位
    try {
      const units = await getProductUnits(p.id)
      patch(index, { units })
    } catch {
      patch(index, { units: [] })
    }

    // 末行则自动新增明细（数据行为，两分支共用）
    if (index >= itemsRef.current.length - 1) {
      addRow()
    }
  }

  // 桌面专用 wrapper：选中商品后跳到下一行商品名称（WPS 风格焦点跳转，仅桌面分支调用）
  const applyProduct = (index: number, p: ProductSearchItem) => {
    void applyProductCore(index, p).then(() => focusProductInput(index + 1))
  }

  const updateUnit = (index: number, unitName: string) => {
    const item = itemsRef.current[index]
    const options = [
      { name: item.base_unit, ratio: 1, unit_price: item.default_base_price || item.unit_price },
      ...item.units.map((u) => ({ name: u.name, ratio: u.ratio, unit_price: u.unit_price || 0 })),
    ]
    const opt = options.find((o) => o.name === unitName)
    if (!opt) return
    const basePrice = item.default_base_price || item.unit_price
    const unitPrice = opt.unit_price > 0 ? round2(opt.unit_price) : round2(basePrice * opt.ratio)
    patch(index, {
      unit: opt.name,
      ratio: opt.ratio,
      unit_price: unitPrice,
      base_quantity: round2(item.quantity * opt.ratio),
      amount: round2(unitPrice * item.quantity),
    })
  }

  const updatePreSaleQty = (index: number, value: number) => {
    const pre = round2(value || 0)
    if (pre > 0) {
      const item = itemsRef.current[index]
      patch(index, {
        pre_sale_quantity: pre,
        quantity: pre,
        base_quantity: round2(pre * (item.ratio || 1)),
        amount: round2(item.unit_price * pre),
      })
    } else {
      patch(index, { pre_sale_quantity: 0 })
    }
  }

  const updateQuantity = (index: number, v: number | null) => {
    const qty = round2(v ?? 0)
    const item = itemsRef.current[index]
    patch(index, {
      quantity: qty,
      base_quantity: round2(qty * (item.ratio || 1)),
      amount: round2(item.unit_price * qty),
    })
  }

  const updateUnitPrice = (index: number, v: number | null) => {
    const price = round2(v ?? 0)
    const item = itemsRef.current[index]
    patch(index, { unit_price: price, amount: round2(price * item.quantity) })
  }

  const cellStyle: React.CSSProperties = { border: '1px solid #f0f0f0', padding: 4, verticalAlign: 'middle' }

  // 移动端：逐行卡片布局（不渲染桌面 table，无键盘导航；业务 handler 与桌面共用）
  if (isMobile) {
    return (
      <div style={{ padding: 4 }}>
        {items.map((item, index) => (
          <div
            key={item.key}
            style={{ border: '1px solid #f0f0f0', borderRadius: 8, padding: 12, marginBottom: 12, background: '#fff' }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
              <div
                style={{
                  flex: 1,
                  minWidth: 0,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  fontWeight: 600,
                  marginRight: 8,
                }}
              >
                #{index + 1} {item.product_name || <span style={{ color: '#bbb' }}>未选择商品</span>}
              </div>
              <Popconfirm title="确定删除该商品行？" onConfirm={() => removeItem(index)}>
                <Button size="small" danger>
                  删除
                </Button>
              </Popconfirm>
            </div>
            <MobileField label="商品名称">
              <AutoComplete
                style={{ width: '100%' }}
                value={item.product_name}
                placeholder="输入商品名称搜索"
                options={productOptions(index)}
                onChange={(v) => handleProductNameChange(index, v)}
                onSelect={(v) => handleProductSelect(index, v)}
              />
            </MobileField>
            <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
              <MobileField label="规格" style={{ flex: 1, marginBottom: 0 }}>
                <Input value={item.spec} onChange={(e) => patch(index, { spec: e.target.value })} />
              </MobileField>
              <MobileField label="单位" style={{ flex: 1, marginBottom: 0 }}>
                <Select
                  style={{ width: '100%' }}
                  value={item.unit || undefined}
                  onChange={(v) => updateUnit(index, v)}
                  options={[
                    ...(item.base_unit ? [{ value: item.base_unit, label: `${item.base_unit}(基础单位)` }] : []),
                    ...item.units.map((u) => ({ value: u.name, label: u.name })),
                  ]}
                />
              </MobileField>
            </div>
            <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
              <MobileField label="预售数量" style={{ flex: 1, marginBottom: 0 }}>
                <InputNumber
                  style={{ width: '100%' }}
                  controls={false}
                  min={0}
                  precision={2}
                  placeholder="预售数量"
                  value={item.pre_sale_quantity || null}
                  onChange={(v) => updatePreSaleQty(index, v ?? 0)}
                />
              </MobileField>
              <MobileField label="数量" style={{ flex: 1, marginBottom: 0 }}>
                <InputNumber
                  style={{ width: '100%' }}
                  controls={false}
                  min={0}
                  precision={2}
                  value={item.quantity || null}
                  onChange={(v) => updateQuantity(index, v)}
                />
              </MobileField>
              <MobileField label="单价" style={{ flex: 1, marginBottom: 0 }}>
                <InputNumber
                  style={{ width: '100%' }}
                  controls={false}
                  min={0}
                  precision={2}
                  value={item.unit_price}
                  onChange={(v) => updateUnitPrice(index, v)}
                />
              </MobileField>
            </div>
            <MobileField label="金额">
              <div style={{ textAlign: 'right', fontSize: 16, fontWeight: 600, padding: '2px 0' }}>
                {item.amount.toFixed(2)}
              </div>
            </MobileField>
            <MobileField label="供应商">
              <Select
                style={{ width: '100%' }}
                showSearch
                allowClear
                placeholder="请选择供应商"
                optionFilterProp="label"
                value={item.supplier_id || undefined}
                options={suppliers.map((s) => ({ value: s.id, label: s.name }))}
                onChange={(v, opt) =>
                  patch(index, {
                    supplier_id: v ?? 0,
                    supplier_name: v ? ((opt as { label: string })?.label ?? '') : '',
                  })
                }
              />
            </MobileField>
            <MobileField label="备注" style={{ marginBottom: 0 }}>
              <Input placeholder="单品备注" value={item.remark} onChange={(e) => patch(index, { remark: e.target.value })} />
            </MobileField>
          </div>
        ))}
        <Button block type="dashed" onClick={() => addRow()}>
          新增商品行
        </Button>
      </div>
    )
  }

  return (
    <div ref={containerRef} style={{ overflowX: 'auto' }}>
      <table style={{ width: '100%', borderCollapse: 'collapse', minWidth: 1180 }}>
        <thead>
          <tr style={{ background: '#fafafa' }}>
            {['商品名称', '规格', '单位', '预售数量', '数量', '单价', '金额', '供应商', '备注', '操作'].map((h, i) => (
              <th key={h} style={{ ...cellStyle, minWidth: [140, 150, 90, 90, 80, 90, 90, 140, 110, 60][i] }}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {items.map((item, index) => (
            <tr key={item.key}>
              <td style={cellStyle}>
                <div data-product-input={index}>
                  <AutoComplete
                    size="small"
                    style={{ width: '100%' }}
                    value={item.product_name}
                    placeholder="输入商品名称搜索"
                    options={productOptions(index)}
                    onChange={(v) => handleProductNameChange(index, v)}
                    onSelect={(v) => handleProductSelect(index, v)}
                    onDropdownVisibleChange={(open) => {
                      dropdownOpen.current[index] = open
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && !dropdownOpen.current[index]) {
                        e.preventDefault()
                        if (index >= itemsRef.current.length - 1) {
                          addRow()
                        }
                        focusProductInput(index + 1)
                      }
                    }}
                  />
                </div>
              </td>
              <td style={cellStyle}>
                <Input
                  size="small"
                  value={item.spec}
                  onChange={(e) => patch(index, { spec: e.target.value })}
                  onKeyDown={(e) => handleEnterNav(e, index)}
                />
              </td>
              <td style={cellStyle}>
                <Select
                  size="small"
                  style={{ width: '100%' }}
                  value={item.unit || undefined}
                  onChange={(v) => updateUnit(index, v)}
                  options={[
                    ...(item.base_unit ? [{ value: item.base_unit, label: `${item.base_unit}(基础单位)` }] : []),
                    ...item.units.map((u) => ({ value: u.name, label: u.name })),
                  ]}
                />
              </td>
              <td style={cellStyle}>
                <InputNumber
                  size="small"
                  style={{ width: '100%' }}
                  controls={false}
                  min={0}
                  precision={2}
                  placeholder="预售数量"
                  value={item.pre_sale_quantity || null}
                  onChange={(v) => updatePreSaleQty(index, v ?? 0)}
                  onKeyDown={(e) => handleEnterNav(e, index)}
                />
              </td>
              <td style={cellStyle}>
                <InputNumber
                  size="small"
                  style={{ width: '100%' }}
                  controls={false}
                  min={0}
                  precision={2}
                  value={item.quantity || null}
                  onChange={(v) => updateQuantity(index, v)}
                  onKeyDown={(e) => handleEnterNav(e, index)}
                />
              </td>
              <td style={cellStyle}>
                <InputNumber
                  size="small"
                  style={{ width: '100%' }}
                  controls={false}
                  min={0}
                  precision={2}
                  value={item.unit_price}
                  onChange={(v) => updateUnitPrice(index, v)}
                  onKeyDown={(e) => handleEnterNav(e, index)}
                />
              </td>
              <td style={{ ...cellStyle, textAlign: 'right' }}>{item.amount.toFixed(2)}</td>
              <td style={cellStyle}>
                <Select
                  size="small"
                  style={{ width: '100%' }}
                  showSearch
                  allowClear
                  placeholder="请选择供应商"
                  optionFilterProp="label"
                  value={item.supplier_id || undefined}
                  options={suppliers.map((s) => ({ value: s.id, label: s.name }))}
                  onChange={(v, opt) =>
                    patch(index, {
                      supplier_id: v ?? 0,
                      supplier_name: v ? ((opt as { label: string })?.label ?? '') : '',
                    })
                  }
                />
              </td>
              <td style={cellStyle}>
                <Input
                  size="small"
                  placeholder="单品备注"
                  value={item.remark}
                  onChange={(e) => patch(index, { remark: e.target.value })}
                  onKeyDown={(e) => handleEnterNav(e, index)}
                />
              </td>
              <td style={cellStyle}>
                <Popconfirm title="确定删除该商品行？" onConfirm={() => removeItem(index)}>
                  <Button size="small" danger>
                    删除
                  </Button>
                </Popconfirm>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
