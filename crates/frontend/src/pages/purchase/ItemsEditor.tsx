import { useEffect, useRef, useState } from 'react'
import { App, AutoComplete, Button, DatePicker, Drawer, Input, InputNumber, Popconfirm, Select, Table, Tag } from 'antd'
import dayjs from 'dayjs'
import {
  getLastPurchasePrice,
  getPriceCompare,
  getProductUnits,
  lookupContractPrice,
  searchProducts,
  type CompareSupplier,
  type ProductSearchItem,
  type UnitOption,
  type Warehouse,
} from '../../api/purchase'

export interface EditItem {
  key: number
  id: number | null
  product_id: number
  product_name: string
  alias1: string
  alias2: string
  spec: string
  unit: string
  base_unit: string
  base_price: number
  purchase_price: number
  unit_price: number
  quantity: number
  ordered_quantity: number
  base_quantity: number
  ratio: number
  amount: number
  remark: string
  warehouse_id: number
  warehouse_name: string
  production_date: string
  batch_no: string
  units: UnitOption[]
}

let keySeq = 1
export function newBlankItem(): EditItem {
  return {
    key: keySeq++,
    id: null,
    product_id: 0,
    product_name: '',
    alias1: '',
    alias2: '',
    spec: '',
    unit: '',
    base_unit: '',
    base_price: 0,
    purchase_price: 0,
    unit_price: 0,
    quantity: 0,
    ordered_quantity: 0,
    base_quantity: 0,
    ratio: 1,
    amount: 0,
    remark: '',
    warehouse_id: 0,
    warehouse_name: '',
    production_date: '',
    batch_no: '',
    units: [],
  }
}

const round2 = (v: number) => Math.round(v * 100) / 100

interface CompareState {
  open: boolean
  index: number
  productName: string
  baseUnit: string
  contractPrice: number
  contractEffectiveDate: string
  contractSource: string
  suppliers: CompareSupplier[]
}

interface Props {
  items: EditItem[]
  onChange: (items: EditItem[]) => void
  supplierId: number | null
  orderDate: string
  warehouses: Warehouse[]
}

export default function ItemsEditor({ items, onChange, supplierId, orderDate, warehouses }: Props) {
  const { message, modal } = App.useApp()
  const containerRef = useRef<HTMLDivElement>(null)
  const itemsRef = useRef(items)
  useEffect(() => {
    itemsRef.current = items
  }, [items])

  const [searchMap, setSearchMap] = useState<Record<number, ProductSearchItem[]>>({})
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const dropdownOpen = useRef<Record<number, boolean>>({})
  const [compare, setCompare] = useState<CompareState | null>(null)

  const patch = (index: number, partial: Partial<EditItem>) => {
    onChange(itemsRef.current.map((it, j) => (j === index ? { ...it, ...partial } : it)))
  }

  const removeItem = (index: number) => {
    onChange(itemsRef.current.filter((_, j) => j !== index))
  }

  // 同列上下移动焦点（WPS 风格）
  const focusCell = (rowIdx: number, cellIdx: number) => {
    const tbody = containerRef.current?.querySelector('tbody')
    const row = tbody?.rows[rowIdx]
    const target = row?.cells[cellIdx]?.querySelector('input')
    if (target) {
      target.focus()
      target.select?.()
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

  // 选中商品：基础信息 → 比价自动填价 → 最近采购价对比提示 → 加载多单位
  const applyProduct = async (index: number, p: ProductSearchItem) => {
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

    const price = round2(p.purchase_price || p.base_price || p.selling_price || 0)
    patch(index, {
      product_id: p.id,
      product_name: p.name,
      alias1: p.alias1 ?? '',
      alias2: p.alias2 ?? '',
      spec: p.spec ?? '',
      unit: p.base_unit,
      base_unit: p.base_unit,
      purchase_price: p.purchase_price || 0,
      unit_price: price,
      base_price: price,
      ratio: 1,
      amount: round2((itemsRef.current[index].quantity || 0) * price),
    })

    // 采购比价：约定采购价 > 当前供应商今日价 > 当前供应商最近价 > 其他供应商最低价
    let suppliers: CompareSupplier[] = []
    let contractPrice = 0
    let contractEffDate = ''
    let bestSource = ''
    try {
      const [c, k] = await Promise.all([
        getPriceCompare(p.id),
        supplierId ? lookupContractPrice(p.id, supplierId, orderDate) : Promise.resolve(null),
      ])
      suppliers = c.suppliers ?? []
      if (k?.matched && k.price > 0) {
        contractPrice = k.price
        contractEffDate = k.effective_date ?? ''
      }
    } catch {
      // 比价接口失败不阻塞开单
    }

    let best = 0
    if (contractPrice > 0) {
      best = contractPrice
      bestSource = `约定采购价（${contractEffDate} 生效）`
    } else if (suppliers.length > 0) {
      const cur = suppliers.find((s) => s.supplier_id === supplierId)
      if (cur && cur.today_price > 0) {
        best = cur.today_price
        bestSource = `当前供应商「${cur.supplier_name}」今日采集价`
      } else if (cur && cur.last_base_unit_price > 0) {
        best = cur.last_base_unit_price
        bestSource = `当前供应商「${cur.supplier_name}」最近采购价`
      } else {
        let min = Infinity
        let minS: CompareSupplier | null = null
        for (const s of suppliers) {
          const pr = s.today_price > 0 ? s.today_price : s.last_base_unit_price
          if (pr > 0 && pr < min) {
            min = pr
            minS = s
          }
        }
        if (minS) {
          best = min
          bestSource =
            minS.today_price > 0
              ? `供应商「${minS.supplier_name}」今日采集价`
              : `供应商「${minS.supplier_name}」最近最低采购价`
        }
      }
    }

    if (best > 0) {
      const bp = round2(best)
      patch(index, {
        unit_price: bp,
        base_price: bp,
        amount: round2((itemsRef.current[index].quantity || 0) * bp),
      })
    }

    if (suppliers.length > 0 || contractPrice > 0) {
      setCompare({
        open: true,
        index,
        productName: p.name,
        baseUnit: p.base_unit,
        contractPrice,
        contractEffectiveDate: contractEffDate,
        contractSource: bestSource,
        suppliers,
      })
    }

    // 最近采购价对比提示（同基础单位）
    const curPrice = itemsRef.current[index].unit_price
    try {
      const lp = await getLastPurchasePrice(p.id)
      const lastPrice = lp.purchase_price || 0
      if (lastPrice > 0 && lp.base_unit === p.base_unit && Math.abs(curPrice - lastPrice) >= 0.01) {
        const diff = curPrice - lastPrice
        await new Promise<void>((resolve) => {
          modal.confirm({
            title: '价格提示',
            content: (
              <div style={{ whiteSpace: 'pre-line' }}>
                {`商品：${p.name}\n最近采购价（基础单位 ${lp.base_unit}）：${lastPrice.toFixed(2)}\n本次采购价：${curPrice.toFixed(2)}\n${diff > 0 ? '上涨' : '下降'} ${Math.abs(diff).toFixed(2)}（${Math.abs((diff / lastPrice) * 100).toFixed(1)}%）\n\n是否继续？`}
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

    // WPS 风格：选中后跳到下一行商品名称，末行则自动新增明细
    if (index >= itemsRef.current.length - 1) {
      onChange([...itemsRef.current, newBlankItem()])
    }
    focusProductInput(index + 1)
  }

  const updateUnit = (index: number, unitName: string) => {
    const item = itemsRef.current[index]
    const options = [
      {
        name: item.base_unit,
        ratio: 1,
        purchase_price: item.purchase_price || item.base_price || item.unit_price,
        base_price: item.base_price || item.unit_price,
      },
      ...item.units.map((u) => ({
        name: u.name,
        ratio: u.ratio,
        purchase_price: u.purchase_price,
        base_price: item.base_price || item.unit_price,
      })),
    ]
    const opt = options.find((o) => o.name === unitName)
    if (!opt) return
    const unitPrice = opt.purchase_price > 0 ? round2(opt.purchase_price) : round2(opt.base_price * opt.ratio)
    patch(index, {
      unit: opt.name,
      ratio: opt.ratio,
      unit_price: unitPrice,
      base_quantity: round2(item.quantity * opt.ratio),
      amount: round2(unitPrice * item.quantity),
    })
  }

  const selectCompareSupplier = (s: CompareSupplier) => {
    if (!compare) return
    const price = s.today_price > 0 ? s.today_price : s.last_base_unit_price
    if (price > 0) {
      const bp = round2(price)
      patch(compare.index, {
        unit_price: bp,
        base_price: bp,
        amount: round2((itemsRef.current[compare.index].quantity || 0) * bp),
      })
      message.success(`已采用「${s.supplier_name}」价格 ¥${bp.toFixed(2)}`)
    }
  }

  const cellStyle: React.CSSProperties = { border: '1px solid #f0f0f0', padding: 4, verticalAlign: 'middle' }

  return (
    <div ref={containerRef} style={{ overflowX: 'auto' }}>
      <table style={{ width: '100%', borderCollapse: 'collapse', minWidth: 1320 }}>
        <thead>
          <tr style={{ background: '#fafafa' }}>
            {['商品名称', '规格', '单位', '订购数量', '数量', '单价', '金额', '仓库', '生产日期', '批号', '备注', '操作'].map(
              (h, i) => (
                <th key={h} style={{ ...cellStyle, minWidth: [140, 140, 90, 90, 80, 90, 90, 110, 130, 90, 110, 60][i] }}>
                  {h}
                </th>
              ),
            )}
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
                    options={(searchMap[index] ?? []).map((p) => ({
                      value: String(p.id),
                      label: (
                        <div>
                          <strong>{p.name}</strong>
                          {p.spec ? ` (${p.spec})` : ''}
                          {(p.alias1 || p.alias2) && (
                            <div style={{ fontSize: 12, color: '#888' }}>
                              {[p.alias1 && `别称1: ${p.alias1}`, p.alias2 && `别称2: ${p.alias2}`]
                                .filter(Boolean)
                                .join(', ')}
                            </div>
                          )}
                          {p.category_name && <div style={{ fontSize: 12, color: '#aaa' }}>分类: {p.category_name}</div>}
                        </div>
                      ),
                    }))}
                    onChange={(v) => {
                      // 选中选项时 onChange 会以 option.value（商品ID）触发，文本由 onSelect 处理
                      if ((searchMap[index] ?? []).some((p) => String(p.id) === v)) return
                      const cur = itemsRef.current[index]
                      // 已选商品后手改文本：清空 product_id，强制重新从下拉选择，保证名称与ID一致
                      patch(index, {
                        product_name: v,
                        ...(cur.product_id > 0 && v !== cur.product_name ? { product_id: 0 } : {}),
                      })
                      handleProductSearch(index, v)
                    }}
                    onSelect={(v) => {
                      const p = (searchMap[index] ?? []).find((x) => String(x.id) === v)
                      if (p) void applyProduct(index, p)
                    }}
                    onDropdownVisibleChange={(open) => {
                      dropdownOpen.current[index] = open
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && !dropdownOpen.current[index]) {
                        e.preventDefault()
                        if (index >= itemsRef.current.length - 1) {
                          onChange([...itemsRef.current, newBlankItem()])
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
                  placeholder="订购数量"
                  value={item.ordered_quantity || null}
                  onChange={(v) => patch(index, { ordered_quantity: round2(v ?? 0) })}
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
                  onChange={(v) => {
                    const qty = round2(v ?? 0)
                    patch(index, {
                      quantity: qty,
                      base_quantity: round2(qty * (item.ratio || 1)),
                      amount: round2(item.unit_price * qty),
                    })
                  }}
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
                  onChange={(v) => {
                    const price = round2(v ?? 0)
                    patch(index, { unit_price: price, amount: round2(price * item.quantity) })
                  }}
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
                  placeholder="仓库"
                  optionFilterProp="label"
                  value={item.warehouse_id || undefined}
                  options={warehouses.map((w) => ({ value: w.id, label: w.name }))}
                  onChange={(v, opt) =>
                    patch(index, {
                      warehouse_id: v ?? 0,
                      warehouse_name: v ? ((opt as { label: string })?.label ?? '') : '',
                    })
                  }
                />
              </td>
              <td style={cellStyle}>
                <DatePicker
                  size="small"
                  style={{ width: '100%' }}
                  value={item.production_date ? dayjs(item.production_date) : null}
                  onChange={(d) => patch(index, { production_date: d ? d.format('YYYY-MM-DD') : '' })}
                />
              </td>
              <td style={cellStyle}>
                <Input
                  size="small"
                  placeholder="批号"
                  value={item.batch_no}
                  onChange={(e) => patch(index, { batch_no: e.target.value })}
                  onKeyDown={(e) => handleEnterNav(e, index)}
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

      <Drawer
        title={`采购比价 - ${compare?.productName ?? ''}${compare?.baseUnit ? `（基础单位：${compare.baseUnit}）` : ''}`}
        open={compare?.open ?? false}
        onClose={() => setCompare((c) => (c ? { ...c, open: false } : c))}
        width={860}
      >
        {compare?.contractPrice ? (
          <div
            style={{
              background: '#d1fae5',
              border: '1px solid #6ee7b7',
              borderRadius: 4,
              padding: '8px 12px',
              marginBottom: 12,
              fontSize: 13,
            }}
          >
            <Tag color="#065f46">约定采购价</Tag>
            <b>¥{compare.contractPrice.toFixed(2)}</b> / {compare.baseUnit}（{compare.contractSource}）
          </div>
        ) : null}
        <Table<CompareSupplier>
          size="small"
          rowKey="supplier_id"
          pagination={false}
          dataSource={compare?.suppliers ?? []}
          columns={[
            {
              title: '供应商',
              dataIndex: 'supplier_name',
              render: (v: string, s, i) => (
                <span>
                  {v}
                  {s.supplier_id === supplierId && <Tag color="processing">当前</Tag>}
                  {i === 0 && s.supplier_id !== supplierId && <Tag color="success">最低</Tag>}
                </span>
              ),
            },
            {
              title: '今日价',
              dataIndex: 'today_price',
              render: (v: number) =>
                v > 0 ? <b style={{ color: '#2563eb' }}>¥{v.toFixed(2)}</b> : '-',
            },
            { title: '最近采购', dataIndex: 'last_date', render: (v: string) => v || '-' },
            {
              title: '最近单价',
              key: 'last',
              render: (_v, s) => (s.last_unit_price > 0 ? `¥${s.last_unit_price.toFixed(2)}/${s.last_unit}` : '-'),
            },
            { title: '加权均价', dataIndex: 'avg_price', render: (v: number) => (v > 0 ? `¥${v.toFixed(2)}` : '-') },
            {
              title: '价格区间',
              key: 'range',
              render: (_v, s) => (s.min_price > 0 ? `¥${s.min_price.toFixed(2)}~¥${s.max_price.toFixed(2)}` : '-'),
            },
            { title: '次数', dataIndex: 'order_count', width: 60 },
            {
              title: '操作',
              key: 'action',
              width: 80,
              render: (_v, s) => (
                <Button size="small" type="link" onClick={() => selectCompareSupplier(s)}>
                  选用
                </Button>
              ),
            },
          ]}
        />
      </Drawer>
    </div>
  )
}
