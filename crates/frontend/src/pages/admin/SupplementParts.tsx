import { useEffect, useMemo, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { Col, Row, Select, Spin, Table, Tag } from 'antd'
import type { TableProps } from 'antd'
import {
  listSupplementProducts,
  type CompareItem,
  type CompareResp,
  type SupplementProduct,
  type SupplementRecord,
} from '../../api/admin/supplement'

const fmt = (v: number) => (v || 0).toFixed(2)
const productName = (p: SupplementProduct) => `${p.name}${p.alias2 ? `(${p.alias2})` : ''}`
const productPrice = (p: SupplementProduct) => p.selling_price || p.base_price || 0

/**
 * 商品搜索选择（老页面 suppProductInput / replaceProductInput 的下拉搜索）：
 * 单击展开全量（过滤耗材类目），输入关键字服务端搜索 /api/product/list?page_size=50。
 */
export function ProductSelect({
  value,
  onChange,
  placeholder = '点击选择商品',
  disabled,
}: {
  value: SupplementProduct | null
  onChange: (p: SupplementProduct | null) => void
  placeholder?: string
  disabled?: boolean
}) {
  const [options, setOptions] = useState<SupplementProduct[]>([])
  const [fetching, setFetching] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const doSearch = (kw: string) => {
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(async () => {
      setFetching(true)
      try {
        const products = await listSupplementProducts(kw)
        // 与老页面一致：排除类目名含「耗材」的商品
        setOptions(products.filter((p) => !(p.category_name || '').includes('耗材')))
      } catch {
        setOptions([])
      } finally {
        setFetching(false)
      }
    }, 250)
  }

  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current)
  }, [])

  return (
    <Select
      style={{ width: '100%' }}
      showSearch
      filterOption={false}
      disabled={disabled}
      value={value ? productName(value) : undefined}
      placeholder={placeholder}
      notFoundContent={fetching ? <Spin size="small" /> : null}
      onSearch={(kw) => doSearch(kw)}
      onDropdownVisibleChange={(open) => {
        if (open) doSearch('')
      }}
      onSelect={(_v, opt) => onChange((opt as { product: SupplementProduct }).product)}
      options={options.map((p) => ({
        value: String(p.id),
        label: `${productName(p)} - ${productPrice(p).toFixed(2)}元/${p.unit || ''}`,
        product: p,
      }))}
    />
  )
}

interface PreviewItem extends CompareItem {
  id: number
}

/** 由 compareData + 未保存增项计算出的本地预览（照抄 renderLocalComparePreview） */
function buildPreview(compare: CompareResp, pendingUnsaved: SupplementRecord[]): CompareResp {
  const preview: CompareResp = JSON.parse(JSON.stringify(compare))
  const itemMap: Record<number, PreviewItem> = {}
  preview.items.forEach((item) => {
    if (item.id > 0) itemMap[item.id] = item as PreviewItem
  })

  pendingUnsaved.forEach((s) => {
    if (s.operation_type === 'increase_quantity' && s.target_order_item_id && itemMap[s.target_order_item_id]) {
      const it = itemMap[s.target_order_item_id]
      it.supplement_quantity += s.quantity
      it.supplement_amount += s.amount
      it.total_quantity += s.quantity
      it.total_amount += s.amount
      it.is_increase = true
    } else if (s.operation_type === 'replace_remove' && s.target_order_item_id && itemMap[s.target_order_item_id]) {
      const it = itemMap[s.target_order_item_id]
      it.supplement_quantity += s.quantity
      it.supplement_amount += s.amount
      it.total_quantity += s.quantity
      it.total_amount += s.amount
      it.is_replaced = true
    } else if (s.operation_type === 'new_item' || s.operation_type === 'replace_add') {
      preview.items.push({
        id: -Math.random(),
        product_id: s.product_id,
        product_name: s.product_name,
        display_name: s.product_name + (s.alias2 ? `(${s.alias2})` : ''),
        alias1: s.alias1,
        alias2: s.alias2,
        spec: s.spec,
        unit: s.unit,
        unit_price: s.unit_price,
        quantity: 0,
        amount: 0,
        is_new: true,
        is_increase: false,
        is_replaced: false,
        supplement_quantity: s.quantity,
        supplement_amount: s.amount,
        total_quantity: s.quantity,
        total_amount: s.amount,
      })
    }
  })

  const pendingAmount = pendingUnsaved.reduce((sum, s) => sum + s.amount, 0)
  preview.supplement_total = compare.supplement_total + pendingAmount
  preview.allocation_total = compare.allocation_total + pendingAmount
  return preview
}

const cellStyle = (bg: string, color: string): CSSProperties => ({ backgroundColor: bg, color })

/** 真实账套 vs 分摊账套 对比（照抄 renderCompareTables / renderLocalComparePreview 的行样式） */
export function CompareTables({ compare, pendingUnsaved }: { compare: CompareResp; pendingUnsaved: SupplementRecord[] }) {
  const preview = useMemo(() => buildPreview(compare, pendingUnsaved), [compare, pendingUnsaved])
  const diff = preview.allocation_total - preview.real_total

  const columns: TableProps<CompareItem>['columns'] = [
    {
      title: '商品',
      dataIndex: 'display_name',
      render: (v: string | undefined, o) => {
        const name = v || o.product_name
        if (o.is_new) return <strong>[增项] {name}</strong>
        if (o.is_replaced)
          return (
            <span>
              <del>{name}</del> <Tag color="error">已替换</Tag>
            </span>
          )
        if (o.is_increase)
          return (
            <span>
              {name} <Tag color="success">+{fmt(o.supplement_quantity)}</Tag>
            </span>
          )
        return name
      },
      onCell: (o) => (o.is_new ? { colSpan: 3, style: { textAlign: 'center', color: '#6c757d', fontSize: 12 } } : {}),
    },
    {
      title: '数量',
      dataIndex: 'total_quantity',
      align: 'right',
      render: (v: number) => fmt(v),
      onCell: (o) => (o.is_new ? { colSpan: 0 } : {}),
    },
    {
      title: '金额',
      dataIndex: 'total_amount',
      align: 'right',
      render: (v: number) => fmt(v),
      onCell: (o) => (o.is_new ? { colSpan: 0 } : {}),
    },
  ]

  const allocRowStyle = (o: CompareItem): CSSProperties => {
    if (o.is_new) return cellStyle('#fff3cd', '#856404')
    if (o.is_replaced) return cellStyle('#f8d7da', '#721c24')
    if (o.is_increase) return cellStyle('#d4edda', '#155724')
    return {}
  }

  return (
    <Row gutter={8}>
      <Col md={12} xs={24}>
        <div style={{ border: '1px solid #eee', borderRadius: 4 }}>
          <div style={{ padding: '4px 8px', background: '#f8f9fa', borderBottom: '1px solid #eee', fontSize: 12 }}>
            <strong>真实账套</strong> - {fmt(preview.real_total)} 元
          </div>
          <div style={{ maxHeight: 300, overflowY: 'auto', padding: 4 }}>
            <Table<CompareItem>
              size="small"
              bordered
              rowKey={(_, i) => String(i)}
              pagination={false}
              dataSource={preview.items}
              columns={columns}
            />
          </div>
        </div>
      </Col>
      <Col md={12} xs={24}>
        <div style={{ border: '1px solid #198754', borderRadius: 4 }}>
          <div
            style={{
              padding: '4px 8px',
              background: '#198754',
              color: '#fff',
              borderBottom: '1px solid #198754',
              fontSize: 12,
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              flexWrap: 'wrap',
            }}
          >
            <span>
              <strong>分摊账套</strong> - {fmt(preview.allocation_total)} 元
            </span>
            <Tag color="warning" style={{ marginRight: 0 }}>
              差异 +{fmt(diff)}
            </Tag>
          </div>
          <div style={{ maxHeight: 300, overflowY: 'auto', padding: 4 }}>
            <Table<CompareItem>
              size="small"
              bordered
              rowKey={(_, i) => String(i)}
              pagination={false}
              dataSource={preview.items}
              columns={columns}
              onRow={(o) => ({ style: allocRowStyle(o) })}
            />
          </div>
        </div>
      </Col>
    </Row>
  )
}
