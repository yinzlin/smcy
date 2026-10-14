import { useCallback, useEffect, useMemo, useState } from 'react'
import { App, AutoComplete, Button, Card, DatePicker, Input, InputNumber, Modal, Select, Space, Table, Upload } from 'antd'
import { PictureOutlined } from '@ant-design/icons'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import { getProductById, getProductUnits, listSuppliers, listWarehouses } from '../../api/purchase'
import {
  createSalesOrder,
  deleteSalesOrderImage,
  generateSalesOrderNo,
  getUserContactPhone,
  listPurchasers,
  lookupPriceScheduleBatch,
  setUserContactPhone,
  updateSalesOrder,
  updateSalesOrderPrices,
  uploadSalesOrderImage,
  type SalesOrderDetail,
  type SchedulePriceEntry,
} from '../../api/sales'
import SalesItemsEditor, { newBlankSalesItem, scheduleUnitPrice, type SalesEditItem } from './ItemsEditor'

const round2 = (v: number) => Math.round(v * 100) / 100
const today = () => dayjs().format('YYYY-MM-DD')
const DEFAULT_SUPPLIER_COMPANY = '湖南食全味美餐饮管理有限公司'
const DEFAULT_TRUCK_PLATE = '湘A·BE9312'

interface Props {
  editing: SalesOrderDetail | null
  onSaved: () => void
  onCancelEdit: () => void
}

interface PriceChange {
  name: string
  oldPrice: number
  newPrice: number
  diff: number
}

export default function SalesFormCard({ editing, onSaved, onCancelEdit }: Props) {
  const { message, modal } = App.useApp()
  const [purchaserId, setPurchaserId] = useState<number | null>(null)
  const [purchaserName, setPurchaserName] = useState('')
  const [warehouseId, setWarehouseId] = useState<number | null>(null)
  const [warehouseName, setWarehouseName] = useState('')
  const [orderNo, setOrderNo] = useState('')
  const [orderDate, setOrderDate] = useState(today())
  const [supplierCompany, setSupplierCompany] = useState(DEFAULT_SUPPLIER_COMPANY)
  const [truckPlate, setTruckPlate] = useState(DEFAULT_TRUCK_PLATE)
  const [contactPhone, setContactPhone] = useState('')
  const [remark, setRemark] = useState('')
  const [discountRate, setDiscountRate] = useState(20)
  const [amountReduction, setAmountReduction] = useState(0)
  const [items, setItems] = useState<SalesEditItem[]>([])
  const [saving, setSaving] = useState(false)
  const [priceChanges, setPriceChanges] = useState<PriceChange[] | null>(null)
  const [customerImage, setCustomerImage] = useState<string | null>(null)
  const [signedImage, setSignedImage] = useState<string | null>(null)

  const { data: purchasers = [] } = useQuery({ queryKey: ['purchasers'], queryFn: listPurchasers })
  const { data: warehouses = [] } = useQuery({ queryKey: ['warehouses'], queryFn: listWarehouses })
  const { data: suppliers = [] } = useQuery({ queryKey: ['suppliers'], queryFn: listSuppliers })

  // 新建销售单仅可选已审核采购单位；明细供应商仅已审核
  const auditedPurchasers = useMemo(() => purchasers.filter((p) => p.audit_status === 'confirmed'), [purchasers])
  const auditedSuppliers = useMemo(() => suppliers.filter((s) => s.audit_status === 'confirmed'), [suppliers])

  const regenOrderNo = useCallback(async (date: string) => {
    try {
      const d = await generateSalesOrderNo(date)
      setOrderNo(d.order_no)
    } catch {
      // 单号生成失败留空，保存时由后端兜底
    }
  }, [])

  // 记住/恢复上一次销售单的采购单位、出库仓库、供应商、供货车牌号、联系方式、下浮率
  const applyLastDefaults = useCallback(() => {
    try {
      const pid = localStorage.getItem('so_last_purchaser_id')
      const pname = localStorage.getItem('so_last_purchaser_name')
      if (pid) {
        setPurchaserId(Number(pid))
        setPurchaserName(pname ?? '')
      }
      const wid = localStorage.getItem('so_last_warehouse_id')
      const wname = localStorage.getItem('so_last_warehouse_name')
      if (wid) {
        setWarehouseId(Number(wid))
        setWarehouseName(wname ?? '')
      }
      const sc = localStorage.getItem('so_last_supplier_company')
      if (sc !== null) setSupplierCompany(sc)
      const tp = localStorage.getItem('so_last_truck_plate')
      if (tp !== null) setTruckPlate(tp)
      const cp = localStorage.getItem('so_last_contact_phone')
      if (cp) setContactPhone(cp)
      const dr = localStorage.getItem('so_last_discount_rate')
      if (dr !== null) setDiscountRate(Number(dr) || 0)
    } catch {
      // localStorage 不可用时忽略
    }
  }, [])

  const rememberDefaults = () => {
    try {
      localStorage.setItem('so_last_purchaser_id', purchaserId ? String(purchaserId) : '')
      localStorage.setItem('so_last_purchaser_name', purchaserName)
      localStorage.setItem('so_last_warehouse_id', warehouseId ? String(warehouseId) : '')
      localStorage.setItem('so_last_warehouse_name', warehouseName)
      localStorage.setItem('so_last_supplier_company', supplierCompany.trim())
      localStorage.setItem('so_last_truck_plate', truckPlate.trim())
      localStorage.setItem('so_last_contact_phone', contactPhone.trim())
      localStorage.setItem('so_last_discount_rate', String(discountRate))
    } catch {
      // 忽略
    }
  }

  const resetForm = useCallback(() => {
    setPurchaserId(null)
    setPurchaserName('')
    setWarehouseId(null)
    setWarehouseName('')
    setOrderDate(today())
    setRemark('')
    setSupplierCompany(DEFAULT_SUPPLIER_COMPANY)
    setTruckPlate(DEFAULT_TRUCK_PLATE)
    setDiscountRate(20)
    setAmountReduction(0)
    setItems([])
    setCustomerImage(null)
    setSignedImage(null)
    void regenOrderNo(today())
    applyLastDefaults()
  }, [regenOrderNo, applyLastDefaults])

  // 联系方式：localStorage 优先，无记录时回退当前用户最近保存值
  useEffect(() => {
    const local = localStorage.getItem('so_last_contact_phone')
    if (local) {
      setContactPhone(local)
      return
    }
    getUserContactPhone()
      .then((d) => {
        if (d.success) setContactPhone(d.contact_phone || '')
      })
      .catch(() => {
        // 拉取失败静默
      })
  }, [])

  // 编辑模式：把详情灌入表单；明细需补充基础单位/多单位信息（补 ratio 用于改数量重算 base_quantity）
  useEffect(() => {
    if (!editing) {
      resetForm()
      return
    }
    setPurchaserId(editing.purchaser_id)
    setPurchaserName(editing.purchaser_name)
    setWarehouseId(editing.warehouse_id || null)
    setWarehouseName(editing.warehouse_name || '')
    setOrderNo(editing.order_no)
    setOrderDate(editing.order_date)
    setRemark(editing.remark ?? '')
    setSupplierCompany(editing.supplier_company ?? '')
    setTruckPlate(editing.truck_plate ?? '')
    setDiscountRate(editing.discount_rate || 0)
    setAmountReduction(editing.amount_reduction || 0)
    setCustomerImage(editing.customer_order_image || null)
    setSignedImage(editing.signed_order_image || null)

    void (async () => {
      const enriched: SalesEditItem[] = await Promise.all(
        editing.items.map(async (it) => {
          let baseUnit = it.unit ?? ''
          let defaultBasePrice = 0
          try {
            const p = await getProductById(it.product_id)
            if (p.id) {
              baseUnit = p.base_unit || baseUnit
              defaultBasePrice = p.base_price || 0
            }
          } catch {
            // 商品已删除时保留明细原值
          }
          let units: SalesEditItem['units'] = []
          try {
            units = await getProductUnits(it.product_id)
          } catch {
            // 忽略
          }
          let ratio = 1
          if (it.unit && it.unit !== baseUnit) {
            const mu = units.find((x) => x.name === it.unit)
            ratio = mu && mu.ratio > 0 ? mu.ratio : 1
          }
          return {
            key: Math.random(),
            product_id: it.product_id,
            product_name: it.product_name,
            alias1: it.alias1 ?? '',
            alias2: it.alias2 ?? '',
            spec: it.spec ?? '',
            unit: it.unit ?? '',
            base_unit: baseUnit,
            default_base_price: defaultBasePrice,
            unit_price: it.unit_price || 0,
            quantity: it.quantity || 0,
            pre_sale_quantity: it.pre_sale_quantity || 0,
            base_quantity: it.base_quantity || 0,
            ratio,
            amount: it.amount || 0,
            supplier_id: it.supplier_id || 0,
            supplier_name: it.supplier_name ?? '',
            remark: it.remark ?? '',
            units,
          }
        }),
      )
      setItems(enriched)
    })()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editing])

  // 采购单位确定后：自动把同名仓库填到出库仓库
  const selectPurchaser = (id: number, name: string) => {
    setPurchaserId(id)
    setPurchaserName(name)
    const matched = warehouses.find((w) => w.name === name)
    if (matched) {
      setWarehouseId(matched.id)
      setWarehouseName(matched.name)
    }
  }

  const totalAmount = useMemo(() => round2(items.reduce((s, it) => s + (it.amount || 0), 0)), [items])
  const discountAmount = round2(totalAmount * (1 - (discountRate || 0) / 100))
  const finalAmount = Math.max(0, round2(discountAmount - (amountReduction || 0)))

  // 订单日期变化：重新取单号；已选明细按新日期重新拉政采价/超市比价覆盖
  const onOrderDateChange = async (date: string) => {
    setOrderDate(date)
    void regenOrderNo(date)
    const ids = items.map((i) => i.product_id).filter((id) => id > 0)
    if (ids.length === 0) return
    try {
      const jd = await lookupPriceScheduleBatch(date, ids)
      if (!jd.success) return
      let updated = 0
      const next = items.map((it) => {
        if (it.product_id <= 0) return it
        const sp = scheduleUnitPrice(jd.prices[String(it.product_id)])
        if (sp > 0) {
          updated += 1
          return { ...it, unit_price: sp, amount: round2((it.quantity || 0) * sp) }
        }
        if (it.default_base_price > 0 && it.unit_price !== it.default_base_price) {
          // 非价格策略时段（或无策略）：回退到商品编辑页面的默认基础单价
          updated += 1
          return { ...it, unit_price: it.default_base_price, amount: round2((it.quantity || 0) * it.default_base_price) }
        }
        return it
      })
      if (updated > 0) setItems(next)
    } catch {
      // 按日期加载价失败静默
    }
  }

  const save = async () => {
    if (!purchaserId) {
      message.warning('请选择采购单位')
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
        purchaser_id: purchaserId,
        order_no: orderNo,
        order_date: orderDate,
        total_amount: totalAmount,
        discount_rate: discountRate || 0,
        amount_reduction: amountReduction || 0,
        final_amount: finalAmount,
        warehouse_id: warehouseId ?? 0,
        warehouse_name: warehouseName,
        items: validItems.map((it) => ({
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
          pre_sale_quantity: it.pre_sale_quantity > 0 ? it.pre_sale_quantity : null,
          supplier_id: it.supplier_id,
          supplier_name: it.supplier_name,
          remark: it.remark || null,
        })),
        remark: remark || null,
        supplier_company: supplierCompany.trim() || null,
        truck_plate: truckPlate.trim() || null,
        version: editing?.version ?? 1,
      }
      const res = editing ? await updateSalesOrder(payload) : await createSalesOrder(payload)
      if (res.ok) {
        message.success(editing ? '订单保存成功' : '订单创建成功')
        rememberDefaults()
        // 非空才更新"上次输入"的联系方式，避免误清空
        if (contactPhone.trim()) {
          void setUserContactPhone(contactPhone.trim()).catch(() => undefined)
        }
        onSaved()
      } else {
        message.error(res.message || '保存失败')
      }
    } finally {
      setSaving(false)
    }
  }

  // 按订单日期批量加载政采价/超市比价，命中明细后自动填入单价
  const loadPriceScheduleByDate = async () => {
    if (!orderDate) {
      message.warning('请先选择订单日期')
      return
    }
    const ids = items.map((i) => i.product_id).filter((id) => id > 0)
    if (ids.length === 0) {
      message.warning('请先添加商品明细')
      return
    }
    try {
      const jd = await lookupPriceScheduleBatch(orderDate, ids)
      if (!jd.success) {
        message.error('加载价格策略失败：' + (jd as unknown as { message?: string }).message)
        return
      }
      let filled = 0
      const missingNames: string[] = []
      const next = items.map((it) => {
        if (it.product_id <= 0) return it
        const p: SchedulePriceEntry | undefined = jd.prices[String(it.product_id)]
        const sp = scheduleUnitPrice(p)
        if (sp > 0) {
          filled += 1
          return { ...it, unit_price: sp, amount: round2((it.quantity || 0) * sp) }
        }
        if (it.default_base_price > 0 && it.unit_price !== it.default_base_price) {
          // 无策略记录：回退到商品编辑页面的默认基础单价
          filled += 1
          return { ...it, unit_price: it.default_base_price, amount: round2((it.quantity || 0) * it.default_base_price) }
        }
        if (!p) missingNames.push(it.product_name || `ID ${it.product_id}`)
        return it
      })
      setItems(next)
      if (filled > 0 && missingNames.length === 0) {
        message.success(`已按 ${orderDate} 政采价/超市比价回填 ${filled} 条明细`)
      } else if (filled > 0) {
        message.warning(`已回填 ${filled} 条；${missingNames.length} 个商品当日无价策略，保持原单价：${missingNames.join('、')}`)
      } else {
        modal.warning({
          title: '未找到对应时段价',
          content: (
            <div style={{ whiteSpace: 'pre-line' }}>
              {`未找到 ${orderDate} 对应时段的政采价/超市比价，请先在【价格策略】页录入。\n\n涉及商品：${missingNames.join('、') || '无'}\n\n排查：\n1) 在【价格策略】页是否已为这些商品录入价格？\n2) 录入的 effective_date 是否 <= ${orderDate}？\n3) 是否被后续时段的 end_date 提前结束？`}
            </div>
          ),
        })
      }
    } catch (e) {
      message.error(`请求失败：${e}`)
    }
  }

  // 一键更新售价：后端按商品最新基础售价返回（基础单位口径），前端按行单位换算
  const applyUpdatePrices = async () => {
    if (!editing) {
      message.warning('请先选择要编辑的订单')
      return
    }
    const ok = await new Promise<boolean>((resolve) => {
      modal.confirm({
        title: '将自动填入商品最新基础售价，请核对后手动保存修改。确定继续？',
        okText: '确定',
        cancelText: '取消',
        onOk: () => resolve(true),
        onCancel: () => resolve(false),
      })
    })
    if (!ok) return
    try {
      const data = await updateSalesOrderPrices(editing.id)
      if (data.errors.length > 0) {
        message.warning('部分商品获取售价失败：\n' + data.errors.join('\n'))
      }
      const priceMap = new Map<number, (typeof data.items)[number]>()
      data.items.forEach((i) => priceMap.set(i.product_id, i))
      const changes: PriceChange[] = []
      const next = items.map((item) => {
        const newData = priceMap.get(item.product_id)
        if (!newData) return item
        // 后端返回基础单价；按行当前单位换算后重算金额（与切换单位逻辑一致）
        const basePrice = newData.unit_price
        let ratio = item.ratio
        if (!ratio || ratio <= 0) {
          ratio = 1
          if (item.unit !== item.base_unit) {
            const u = item.units.find((x) => x.name === item.unit)
            if (u && u.ratio > 0) ratio = u.ratio
          }
        }
        const isBaseUnit = item.unit === item.base_unit
        const newPrice = isBaseUnit ? basePrice : round2(basePrice * ratio)
        const diff = newPrice - item.unit_price
        if (Math.abs(diff) > 0.001) {
          changes.push({ name: item.product_name, oldPrice: item.unit_price, newPrice, diff })
        }
        return {
          ...item,
          unit_price: newPrice,
          ratio,
          base_quantity: round2((item.quantity || 0) * ratio),
          amount: round2(newPrice * (item.quantity || 0)),
        }
      })
      setItems(next)
      if (changes.length > 0) {
        setPriceChanges(changes)
      } else {
        message.success(`已获取 ${data.items.length} 项商品最新售价，无变动。`)
      }
    } catch (e) {
      message.error(`获取失败：${e instanceof Error ? e.message : e}`)
    }
  }

  const imageUploadProps = (type: 'customer' | 'signed') => ({
    accept: 'image/*',
    showUploadList: false,
    beforeUpload: (file: File) => {
      if (!editing) {
        message.warning('请先保存销售订单后再上传图片')
        return false
      }
      void uploadSalesOrderImage(editing.id, type, file)
        .then((r) => {
          if (type === 'customer') setCustomerImage(r.url)
          else setSignedImage(r.url)
        })
        .catch((err) => message.error(`图片上传失败：${err instanceof Error ? err.message : err}`))
      return false
    },
  })

  const removeImage = (type: 'customer' | 'signed') => {
    if (!editing) return
    modal.confirm({
      title: '确定删除该图片？',
      onOk: () =>
        void deleteSalesOrderImage(editing.id, type).then((r) => {
          if (r.ok) {
            if (type === 'customer') setCustomerImage(null)
            else setSignedImage(null)
          } else {
            message.error('删除失败')
          }
        }),
    })
  }

  const imageBlock = (type: 'customer' | 'signed') => {
    const url = type === 'customer' ? customerImage : signedImage
    const label = type === 'customer' ? '上传客户订单' : '上传已验收签字订单'
    return (
      <>
        <Upload {...imageUploadProps(type)}>
          <Button icon={<PictureOutlined />}>{label}</Button>
        </Upload>
        {url && (
          <>
            <a href={url} target="_blank" rel="noreferrer">
              <img src={url} style={{ height: 38, width: 38, objectFit: 'cover', borderRadius: 4, border: '1px solid #ddd' }} alt="" />
            </a>
            <Button size="small" danger onClick={() => removeImage(type)}>
              删除
            </Button>
          </>
        )}
      </>
    )
  }

  return (
    <Card title={editing ? `编辑销售订单（${editing.order_no}）` : '新建销售订单'} style={{ marginBottom: 16 }}>
      <Space wrap size="middle" style={{ marginBottom: 12 }}>
        <div>
          <div style={{ marginBottom: 4 }}>采购单位：</div>
          <AutoComplete
            style={{ width: 220 }}
            placeholder="输入关键字搜索（仅已审核）"
            value={purchaserName}
            options={auditedPurchasers.map((p) => ({ value: String(p.id), label: p.name }))}
            filterOption={(input, opt) => (opt?.label ?? '').toLowerCase().includes(input.toLowerCase())}
            onChange={(v) => {
              if (auditedPurchasers.some((p) => String(p.id) === v)) return
              setPurchaserName(v)
              setPurchaserId(null)
            }}
            onSelect={(v) => {
              const p = auditedPurchasers.find((x) => String(x.id) === v)
              if (p) selectPurchaser(p.id, p.name)
            }}
          />
        </div>
        <div>
          <div style={{ marginBottom: 4 }}>出库仓库：</div>
          <Select
            style={{ width: 200 }}
            showSearch
            allowClear
            placeholder="选择仓库"
            optionFilterProp="label"
            value={warehouseId ?? undefined}
            onChange={(v, opt) => {
              setWarehouseId(v ?? null)
              setWarehouseName(v ? ((opt as { label: string })?.label ?? '') : '')
            }}
            options={warehouses.map((w) => ({ value: w.id, label: w.name }))}
          />
        </div>
        <div>
          <div style={{ marginBottom: 4 }}>订单号：</div>
          <Input style={{ width: 180 }} value={orderNo} readOnly />
        </div>
        <div>
          <div style={{ marginBottom: 4 }}>订单日期：</div>
          <DatePicker
            value={dayjs(orderDate)}
            onChange={(d) => void onOrderDateChange(d ? d.format('YYYY-MM-DD') : today())}
          />
        </div>
        <div>
          <div style={{ marginBottom: 4 }}>供应商（验收单/报销单用）：</div>
          <Input style={{ width: 240 }} value={supplierCompany} onChange={(e) => setSupplierCompany(e.target.value)} placeholder="请输入供应商名称" />
        </div>
        <div>
          <div style={{ marginBottom: 4 }}>供货车牌号（验收单/报销单用）：</div>
          <Input style={{ width: 160 }} value={truckPlate} onChange={(e) => setTruckPlate(e.target.value)} placeholder="请输入供货车牌号" />
        </div>
        <div>
          <div style={{ marginBottom: 4 }}>联系方式（验收单/报销单用）：</div>
          <Input style={{ width: 160 }} value={contactPhone} onChange={(e) => setContactPhone(e.target.value)} placeholder="请输入联系方式" />
        </div>
        <div>
          <div style={{ marginBottom: 4 }}>备注：</div>
          <Input style={{ width: 200 }} value={remark} onChange={(e) => setRemark(e.target.value)} />
        </div>
      </Space>

      <SalesItemsEditor items={items} onChange={setItems} orderDate={orderDate} suppliers={auditedSuppliers} />

      <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 12, flexWrap: 'wrap', gap: 8 }}>
        <Button type="primary" onClick={() => setItems((arr) => [...arr, newBlankSalesItem()])}>
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
          <span>
            金额折减：
            <InputNumber
              size="small"
              style={{ width: 80 }}
              min={0}
              step={0.01}
              value={amountReduction}
              onChange={(v) => setAmountReduction(v ?? 0)}
            />
          </span>
          <span>
            最终合计：<b style={{ color: '#cf1322' }}>¥{finalAmount.toFixed(2)}</b>
          </span>
        </Space>
      </div>

      <Space wrap style={{ marginTop: 12 }}>
        <Button type="primary" loading={saving} onClick={() => void save()}>
          {editing ? '保存修改' : '保存销售订单'}
        </Button>
        <Button
          onClick={() => {
            if (editing) onCancelEdit()
            resetForm()
          }}
        >
          新建订单
        </Button>
        <Button type="primary" ghost onClick={() => void loadPriceScheduleByDate()} title="按订单日期加载政采价/超市比价">
          按日期加载价
        </Button>
        {editing && <Button onClick={() => void applyUpdatePrices()}>一键更新售价</Button>}
        {imageBlock('customer')}
        {imageBlock('signed')}
      </Space>

      <Modal title="售价变更明细" open={priceChanges !== null} footer={null} onCancel={() => setPriceChanges(null)}>
        <Table<PriceChange>
          size="small"
          rowKey="name"
          pagination={false}
          dataSource={priceChanges ?? []}
          columns={[
            { title: '商品名称', dataIndex: 'name' },
            { title: '原售价', dataIndex: 'oldPrice', render: (v: number) => `¥${v.toFixed(2)}` },
            { title: '新售价', dataIndex: 'newPrice', render: (v: number) => `¥${v.toFixed(2)}` },
            {
              title: '变动',
              dataIndex: 'diff',
              render: (v: number) => (
                <span style={{ color: v > 0 ? '#389e0d' : '#cf1322' }}>
                  {v > 0 ? '↑' : '↓'} ¥{Math.abs(v).toFixed(2)}
                </span>
              ),
            },
          ]}
        />
      </Modal>
    </Card>
  )
}
