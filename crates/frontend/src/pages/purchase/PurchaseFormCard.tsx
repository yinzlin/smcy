import { useCallback, useEffect, useMemo, useState } from 'react'
import { App, AutoComplete, Button, Card, Col, DatePicker, Input, InputNumber, Row, Select, Space, Tag } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import {
  createPurchaseOrder,
  generateOrderNo,
  getProductById,
  getProductUnits,
  listSuppliers,
  listUsers,
  listWarehouses,
  updatePurchaseOrder,
  type PurchaseOrderDetail,
  type Supplier,
} from '../../api/purchase'
import ItemsEditor, { newBlankItem, type EditItem } from './ItemsEditor'

const round2 = (v: number) => Math.round(v * 100) / 100
const today = () => dayjs().format('YYYY-MM-DD')

interface Props {
  editing: PurchaseOrderDetail | null
  onSaved: () => void
  onCancelEdit: () => void
}

export default function PurchaseFormCard({ editing, onSaved, onCancelEdit }: Props) {
  const { message } = App.useApp()
  const [supplierId, setSupplierId] = useState<number | null>(null)
  const [supplierName, setSupplierName] = useState('')
  const [orderNo, setOrderNo] = useState('')
  const [orderDate, setOrderDate] = useState(today())
  const [remark, setRemark] = useState('')
  const [handlerId, setHandlerId] = useState<number | null>(null)
  const [discountRate, setDiscountRate] = useState(0)
  const [items, setItems] = useState<EditItem[]>([])
  const [saving, setSaving] = useState(false)

  const { data: suppliers = [] } = useQuery({ queryKey: ['suppliers'], queryFn: listSuppliers })
  const { data: warehouses = [] } = useQuery({ queryKey: ['warehouses'], queryFn: listWarehouses })
  const { data: users = [] } = useQuery({ queryKey: ['users'], queryFn: listUsers })

  // 新建订单仅可选已审核供应商
  const auditedSuppliers = useMemo(
    () => suppliers.filter((s) => s.audit_status === 'confirmed'),
    [suppliers],
  )

  const regenOrderNo = useCallback(async (date: string) => {
    try {
      const d = await generateOrderNo(date)
      setOrderNo(d.order_no)
    } catch {
      // 单号生成失败留空，保存时由后端兜底
    }
  }, [])

  // 记住/恢复上一次采购单的供应商、经手人与下浮率
  const applyLastDefaults = useCallback(() => {
    try {
      const sid = localStorage.getItem('po_last_supplier_id')
      const sname = localStorage.getItem('po_last_supplier_name')
      if (sid) {
        setSupplierId(Number(sid))
        setSupplierName(sname ?? '')
      }
      const hid = localStorage.getItem('po_last_handler_id')
      if (hid) setHandlerId(Number(hid))
      const dr = localStorage.getItem('po_last_discount_rate')
      if (dr !== null) setDiscountRate(Number(dr) || 0)
    } catch {
      // localStorage 不可用时忽略
    }
  }, [])

  const rememberDefaults = () => {
    try {
      localStorage.setItem('po_last_supplier_id', supplierId ? String(supplierId) : '')
      localStorage.setItem('po_last_supplier_name', supplierName)
      localStorage.setItem('po_last_handler_id', handlerId ? String(handlerId) : '')
      localStorage.setItem('po_last_discount_rate', String(discountRate))
    } catch {
      // 忽略
    }
  }

  const resetForm = useCallback(() => {
    setSupplierId(null)
    setSupplierName('')
    setOrderDate(today())
    setRemark('')
    setHandlerId(null)
    setDiscountRate(0)
    setItems([])
    void regenOrderNo(today())
    applyLastDefaults()
  }, [regenOrderNo, applyLastDefaults])

  // 编辑模式：把详情灌入表单；明细需补充基础单位/多单位信息
  useEffect(() => {
    if (!editing) {
      resetForm()
      return
    }
    setSupplierId(editing.supplier_id)
    setSupplierName(editing.supplier_name)
    setOrderNo(editing.order_no)
    setOrderDate(editing.order_date)
    setRemark(editing.remark ?? '')
    setDiscountRate(editing.discount_rate || 0)
    setHandlerId(editing.user_id && editing.user_id > 0 ? editing.user_id : null)

    void (async () => {
      const enriched: EditItem[] = await Promise.all(
        editing.items.map(async (it) => {
          let baseUnit = it.unit ?? ''
          let basePrice = it.unit_price || 0
          try {
            const p = await getProductById(it.product_id)
            if (p.id) {
              baseUnit = p.base_unit || baseUnit
              basePrice = p.base_price || basePrice
            }
          } catch {
            // 商品已删除时保留明细原值
          }
          let units: EditItem['units'] = []
          try {
            units = await getProductUnits(it.product_id)
          } catch {
            // 忽略
          }
          return {
            key: Math.random(),
            id: it.id,
            product_id: it.product_id,
            product_name: it.product_name,
            alias1: it.alias1 ?? '',
            alias2: it.alias2 ?? '',
            spec: it.spec ?? '',
            unit: it.unit ?? '',
            base_unit: baseUnit,
            base_price: basePrice,
            purchase_price: 0,
            unit_price: it.unit_price || 0,
            quantity: it.quantity || 0,
            ordered_quantity: it.ordered_quantity || 0,
            base_quantity: it.base_quantity || 0,
            ratio: 1,
            amount: it.amount || 0,
            remark: it.remark ?? '',
            warehouse_id: it.warehouse_id ?? 0,
            warehouse_name: it.warehouse_name ?? '',
            production_date: it.production_date ?? '',
            batch_no: it.batch_no ?? '',
            units,
          }
        }),
      )
      setItems(enriched)
    })()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editing])

  const totalAmount = useMemo(() => round2(items.reduce((s, it) => s + (it.amount || 0), 0)), [items])
  const discountAmount = round2(totalAmount * (1 - (discountRate || 0) / 100))
  const amountReduction = round2(discountAmount - Math.floor(discountAmount))
  const finalAmount = round2(discountAmount - amountReduction)

  const save = async () => {
    if (!supplierId) {
      message.warning('请选择供应商')
      return
    }
    const validItems = items.filter((it) => it.product_id > 0 && it.product_name.trim() !== '')
    if (validItems.length === 0) {
      message.warning('请添加商品明细')
      return
    }
    setSaving(true)
    try {
      const payload = {
        id: editing?.id ?? null,
        supplier_id: supplierId,
        order_no: orderNo,
        order_date: orderDate,
        total_amount: totalAmount,
        discount_rate: discountRate || 0,
        amount_reduction: amountReduction,
        final_amount: finalAmount,
        warehouse_id: 0,
        warehouse_name: '',
        user_id: handlerId,
        items: validItems.map((it) => ({
          id: it.id,
          product_id: it.product_id,
          product_name: it.product_name,
          alias1: it.alias1,
          alias2: it.alias2,
          spec: it.spec,
          unit: it.unit,
          unit_price: it.unit_price,
          quantity: it.quantity,
          base_quantity: it.base_quantity,
          amount: it.amount,
          ordered_quantity: it.ordered_quantity,
          remark: it.remark,
          warehouse_id: it.warehouse_id,
          warehouse_name: it.warehouse_name,
          production_date: it.production_date,
          batch_no: it.batch_no,
        })),
        remark: remark || null,
        version: editing?.version ?? null,
      }
      const res = editing ? await updatePurchaseOrder(payload) : await createPurchaseOrder(payload)
      if (res.ok) {
        message.success(editing ? '保存成功' : '创建成功')
        rememberDefaults()
        resetForm()
        onSaved()
      } else {
        message.error(res.message || '保存失败')
      }
    } finally {
      setSaving(false)
    }
  }

  const supplierProfile = (s: Supplier) => {
    if (s.is_designated === 1) {
      let warn: React.ReactNode = null
      if (s.license_expire) {
        const days = dayjs(s.license_expire).startOf('day').diff(dayjs().startOf('day'), 'day')
        if (days < 0) warn = <Tag color="error">许可证已过期</Tag>
        else if (days <= 30) warn = <Tag color="warning">许可证剩{days}天</Tag>
      }
      return (
        <div style={{ border: '1px solid #e5e7eb', borderRadius: 4, padding: 8, fontSize: 12, background: '#f8f9fa', marginTop: 4 }}>
          <Tag color="processing">定点</Tag>
          {s.designated_no ? `${s.designated_no}｜` : ''}
          {s.license_no ? `许可证 ${s.license_no}｜` : ''}
          {s.license_expire ? `有效期至 ${s.license_expire}` : ''}
          {warn}
          {(s.phone || s.address) && (
            <div>
              {s.phone ? `电话 ${s.phone}` : ''}
              {s.phone && s.address ? '｜' : ''}
              {s.address ?? ''}
            </div>
          )}
        </div>
      )
    }
    const parts = [s.phone ? `电话 ${s.phone}` : '', s.address ?? ''].filter(Boolean)
    return parts.length > 0 ? <div style={{ fontSize: 12, color: '#888', marginTop: 4 }}>{parts.join('｜')}</div> : null
  }

  const selectedSupplier = supplierId ? suppliers.find((s) => s.id === supplierId) : undefined

  return (
    <Card title={editing ? `编辑采购订单（${editing.order_no}）` : '新建采购订单'} style={{ marginBottom: 16 }}>
      <Row gutter={[8, 8]} style={{ marginBottom: 12 }}>
        <Col xs={24} sm={12} lg={6}>
          <div style={{ marginBottom: 4 }}>供应商：</div>
          <AutoComplete
            style={{ width: '100%', maxWidth: 240 }}
            placeholder="输入关键字搜索（仅已审核）"
            value={supplierName}
            options={auditedSuppliers.map((s) => ({ value: String(s.id), label: s.name }))}
            filterOption={(input, opt) => (opt?.label ?? '').toLowerCase().includes(input.toLowerCase())}
            onChange={(v) => {
              // 选中选项时 onChange 会以 option.value（供应商ID）触发，文本由 onSelect 处理
              if (auditedSuppliers.some((s) => String(s.id) === v)) return
              setSupplierName(v)
              // 手改文本即视为未选择，保存时需重新从下拉选择
              setSupplierId(null)
            }}
            onSelect={(v) => {
              setSupplierId(Number(v))
              const s = auditedSuppliers.find((x) => String(x.id) === v)
              if (s) setSupplierName(s.name)
            }}
          />
          {selectedSupplier && supplierProfile(selectedSupplier)}
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <div style={{ marginBottom: 4 }}>订单号：</div>
          <Input style={{ width: '100%', maxWidth: 200 }} value={orderNo} readOnly />
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <div style={{ marginBottom: 4 }}>订单日期：</div>
          <DatePicker
            value={dayjs(orderDate)}
            onChange={(d) => {
              const date = d ? d.format('YYYY-MM-DD') : today()
              setOrderDate(date)
              void regenOrderNo(date)
            }}
          />
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <div style={{ marginBottom: 4 }}>备注：</div>
          <Input style={{ width: '100%', maxWidth: 200 }} value={remark} onChange={(e) => setRemark(e.target.value)} />
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <div style={{ marginBottom: 4 }}>经手人：</div>
          <Select
            style={{ width: '100%', maxWidth: 200 }}
            allowClear
            placeholder="请选择经手人"
            value={handlerId ?? undefined}
            onChange={(v) => setHandlerId(v ?? null)}
            options={users
              .filter((u) => (u.nickname || u.username) !== '')
              .map((u) => ({
                value: u.id,
                label: `${u.nickname || u.username}${u.phone ? ` (${u.phone})` : ''}`,
              }))}
          />
        </Col>
      </Row>

      <ItemsEditor items={items} onChange={setItems} supplierId={supplierId} orderDate={orderDate} warehouses={warehouses} />

      <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 12, flexWrap: 'wrap', gap: 8 }}>
        <Button type="primary" onClick={() => setItems((arr) => [...arr, newBlankItem()])}>
          新增商品行
        </Button>
        <Space size="large" wrap>
          <span>
            合计：<b>¥{totalAmount.toFixed(2)}</b>
          </span>
          <span>
            下浮率：
            <InputNumber
              size="small"
              style={{ width: 80 }}
              min={0}
              step={0.1}
              value={discountRate}
              onChange={(v) => setDiscountRate(v ?? 0)}
            />
            %
          </span>
          <span>
            下浮后：<b>¥{discountAmount.toFixed(2)}</b>
          </span>
          <span title="自动扣减下浮后金额的小数部分，使整单金额为整数">
            金额折减：
            <Input style={{ width: 80, background: '#f0f0f0' }} size="small" readOnly value={amountReduction.toFixed(2)} />
          </span>
          <span>
            最终合计：<b style={{ color: '#cf1322' }}>¥{finalAmount.toFixed(2)}</b>
          </span>
        </Space>
      </div>

      <Space style={{ marginTop: 12 }}>
        <Button type="primary" loading={saving} onClick={() => void save()}>
          {editing ? '保存修改' : '保存采购订单'}
        </Button>
        <Button
          onClick={() => {
            if (editing) onCancelEdit()
            resetForm()
          }}
        >
          新建订单
        </Button>
      </Space>
    </Card>
  )
}
