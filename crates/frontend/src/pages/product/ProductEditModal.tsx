import { useEffect, useState } from 'react'
import {
  App,
  Button,
  Col,
  Divider,
  Form,
  Input,
  InputNumber,
  Modal,
  Row,
  Select,
  Space,
  Table,
  Upload,
} from 'antd'
import type { TableProps, UploadProps } from 'antd'
import {
  applyProductImageUrl,
  createProductUnit,
  deleteProductImage,
  deleteProductPrices,
  deleteProductUnits,
  syncBasePrice,
  updateProduct,
  upsertProductPrice,
  uploadProductImage,
  type Category,
  type ProductUnit,
  type Product,
} from '../../api/product'
import { scrollX } from '../../utils/tableScroll'

interface Props {
  open: boolean
  product: Product | null
  categories: Category[]
  isSuperAdmin: boolean
  onClose: () => void
  onSaved: () => void
}

const PRICE_TYPE_BY_FIELD: Record<string, string> = {
  gov_price: 'gov_procurement',
  supermarket_1: 'supermarket_1',
  supermarket_2: 'supermarket_2',
  supermarket_3: 'supermarket_3',
  ai_realtime: 'ai_realtime',
}

export default function ProductEditModal({ open, product, categories, isSuperAdmin, onClose, onSaved }: Props) {
  const { message, modal } = App.useApp()
  const [form] = Form.useForm()
  const [units, setUnits] = useState<ProductUnit[]>([])
  const [imageUrl, setImageUrl] = useState<string | null>(null)
  const [imageUrlInput, setImageUrlInput] = useState('')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (open && product) {
      setImageUrl(product.image_url)
      setImageUrlInput('')
      setUnits((product.units ?? []).map((u) => ({ ...u })))
      const priceOf = (t: string) => {
        const hit = (product.prices ?? []).find((p) => p.price_type === t)
        return hit ? hit.price : undefined
      }
      form.setFieldsValue({
        name: product.name,
        alias1: product.alias1,
        alias2: product.alias2,
        unit: product.unit,
        spec: product.spec,
        base_unit: product.base_unit,
        base_price: product.base_price || 0,
        purchase_price: product.purchase_price || 0,
        max_purchase_price: product.max_purchase_price || 0,
        min_purchase_price: product.min_purchase_price || 0,
        category_id: product.category_id ?? undefined,
        markup_rate: product.markup_rate ?? 0.5,
        auto_update_price: product.auto_update_price ?? 0,
        shelf_life: product.shelf_life,
        gov_status: product.gov_status ?? 0,
        gov_price: priceOf('gov_procurement'),
        supermarket_1: priceOf('supermarket_1'),
        supermarket_2: priceOf('supermarket_2'),
        supermarket_3: priceOf('supermarket_3'),
        ai_realtime: priceOf('ai_realtime'),
        selling_price: product.selling_price || 0,
      })
    }
  }, [open, product, form])

  /** 计算售价：政采价优先，其次商超1/2/3/AI 非零均价（四舍五入 2 位），最后回退基础单价 */
  const computeSellingPrice = (v: Record<string, unknown>) => {
    const gov = Number(v.gov_price) || 0
    if (gov > 0) return gov
    const vals = [v.supermarket_1, v.supermarket_2, v.supermarket_3, v.ai_realtime]
      .map((x) => Number(x) || 0)
      .filter((x) => x > 0)
    if (vals.length > 0) {
      return Math.round((vals.reduce((a, b) => a + b, 0) / vals.length) * 100) / 100
    }
    return Number(v.base_price) || 0
  }

  /** 政采/商超/AI 价变动：联动政采状态，并同步计算售价与基础单价 */
  const onPriceChange = () => {
    const v = form.getFieldsValue()
    const gov = Number(v.gov_price) || 0
    let gs = Number(v.gov_status) || 0
    if (gov > 0) gs = 1
    else if (gs === 1) gs = 0
    const sp = computeSellingPrice(v)
    const rounded = Math.round(sp * 100) / 100
    form.setFieldsValue({
      gov_status: gs,
      selling_price: rounded,
      base_price: rounded > 0 ? rounded : v.base_price,
    })
  }

  /** 加成率/进价变动：自动更新开启且有进价时预览售价 */
  const onMarkupChange = () => {
    const v = form.getFieldsValue()
    if (Number(v.auto_update_price) !== 1) return
    const purchase = Number(v.purchase_price) || 0
    const markup = Number(v.markup_rate) || 0
    if (purchase > 0) {
      form.setFieldValue('selling_price', Number((purchase * (1 + markup)).toFixed(2)))
    }
  }

  const confirmModal = (content: string) =>
    new Promise<boolean>((resolve) => {
      modal.confirm({
        title: '价格提示',
        content: <div style={{ whiteSpace: 'pre-wrap' }}>{content}</div>,
        okText: '继续',
        cancelText: '取消',
        onOk: () => resolve(true),
        onCancel: () => resolve(false),
      })
    })

  const saveUnits = async (productId: number) => {
    await deleteProductUnits(productId)
    for (const u of units) {
      if (!u.unit_name) continue
      await createProductUnit({
        product_id: productId,
        unit_name: u.unit_name,
        ratio: Number(u.ratio) || 1,
        unit_price: Number(u.unit_price) || 0,
        purchase_price: Number(u.purchase_price) || 0,
        sort_order: Number(u.sort_order) || 0,
      })
    }
  }

  const savePrices = async (productId: number, values: Record<string, unknown>) => {
    await deleteProductPrices(productId)
    for (const [field, type] of Object.entries(PRICE_TYPE_BY_FIELD)) {
      const price = Number(values[field]) || 0
      if (price > 0) await upsertProductPrice(productId, type, price)
    }
    await syncBasePrice(productId)
  }

  const submit = async () => {
    if (!product) return
    const values = await form.validateFields()
    const purchase = Number(values.purchase_price) || 0
    const selling = Number(values.base_price) || 0
    const warnings: string[] = []
    if (purchase <= 0) warnings.push('当前进价为 0')
    if (selling <= 0) warnings.push('当前售价为 0')

    if (purchase > 0 && selling > 0 && purchase > selling) {
      const ok = await confirmModal(
        `【严重提醒】\n警告：当前进价（${purchase.toFixed(2)}）高于当前售价（${selling.toFixed(2)}），请确认是否倒置！\n\n是否仍要保存？`,
      )
      if (!ok) return
      if (warnings.length > 0) {
        const ok2 = await confirmModal(`【普通提醒】以下价格为零：${warnings.join('、')}\n\n是否仍要保存？`)
        if (!ok2) return
      }
    } else if (warnings.length > 0) {
      const ok = await confirmModal(`【普通提醒】以下价格为零：${warnings.join('、')}\n\n是否仍要保存？`)
      if (!ok) return
    }

    setSaving(true)
    try {
      const res = await updateProduct({
        id: product.id,
        name: String(values.name ?? ''),
        spec: (values.spec as string) || null,
        alias1: (values.alias1 as string) || null,
        alias2: (values.alias2 as string) || null,
        unit: (values.unit as string) || null,
        base_unit: (values.base_unit as string) || null,
        base_price: (values.base_price as number) ?? null,
        purchase_price: (values.purchase_price as number) ?? null,
        image_url: imageUrl,
        category_id: (values.category_id as number) ?? null,
        markup_rate: Number(values.markup_rate) || 0.5,
        auto_update_price: Number(values.auto_update_price) || 0,
        shelf_life: (values.shelf_life as string) || null,
        gov_status: Number(values.gov_status) || 0,
      })
      if (!res.ok) {
        message.error(res.message || '保存失败')
        return
      }
      await saveUnits(product.id)
      await savePrices(product.id, values)
      message.success('保存成功')
      onSaved()
    } finally {
      setSaving(false)
    }
  }

  const uploadProps: UploadProps = {
    accept: 'image/*',
    showUploadList: false,
    beforeUpload: (file) => {
      void (async () => {
        if (!product) return
        const res = await uploadProductImage(product.id, file)
        if (res.url) {
          setImageUrl(res.url)
          message.success('图片上传成功')
        } else {
          message.error(res.error || '上传失败')
        }
      })()
      return false
    },
  }

  const unitColumns = (): TableProps<ProductUnit>['columns'] => {
    const cols: TableProps<ProductUnit>['columns'] = [
      {
        title: '单位名称',
        dataIndex: 'unit_name',
        render: (_v, _r, i) => (
          <Input value={units[i].unit_name} onChange={(e) => patchUnit(i, { unit_name: e.target.value })} />
        ),
      },
      {
        title: '换算比例（1本单位=?基础单位）',
        dataIndex: 'ratio',
        width: 220,
        render: (_v, _r, i) => (
          <InputNumber
            value={units[i].ratio}
            step={0.01}
            style={{ width: '100%' }}
            onChange={(v) => patchUnit(i, { ratio: Number(v) || 0 })}
          />
        ),
      },
      {
        title: '单位售价（留0则按比例自动算）',
        dataIndex: 'unit_price',
        width: 200,
        render: (_v, _r, i) => (
          <InputNumber
            value={units[i].unit_price}
            step={0.01}
            style={{ width: '100%' }}
            onChange={(v) => patchUnit(i, { unit_price: Number(v) || 0 })}
          />
        ),
      },
    ]
    if (isSuperAdmin) {
      cols.push({
        title: '单位采购价（留0则按进价比例算）',
        dataIndex: 'purchase_price',
        width: 200,
        render: (_v, _r, i) => (
          <InputNumber
            value={units[i].purchase_price}
            step={0.01}
            style={{ width: '100%' }}
            onChange={(v) => patchUnit(i, { purchase_price: Number(v) || 0 })}
          />
        ),
      })
    }
    cols.push(
      {
        title: '排序',
        dataIndex: 'sort_order',
        width: 90,
        render: (_v, _r, i) => (
          <InputNumber
            value={units[i].sort_order}
            style={{ width: '100%' }}
            onChange={(v) => patchUnit(i, { sort_order: Number(v) || 0 })}
          />
        ),
      },
      {
        title: '操作',
        width: 90,
        render: (_v, _r, i) => (
          <Button size="small" danger onClick={() => setUnits((prev) => prev.filter((_, idx) => idx !== i))}>
            删除
          </Button>
        ),
      },
    )
    return cols
  }

  const patchUnit = (index: number, patch: Partial<ProductUnit>) => {
    setUnits((prev) => prev.map((u, i) => (i === index ? { ...u, ...patch } : u)))
  }

  return (
    <Modal
      title="编辑商品"
      open={open}
      onCancel={onClose}
      onOk={submit}
      okText="保存"
      cancelText="取消"
      confirmLoading={saving}
      width={960}
      destroyOnClose
    >
      <Form form={form} layout="vertical">
        <Row gutter={[12, 8]}>
          <Col xs={24} sm={12} lg={8}>
            <Form.Item label="商品名称（通用名）" name="name" rules={[{ required: true, message: '请输入名称' }]}>
              <Input />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={5}>
            <Form.Item label="别称1（如地域称呼）" name="alias1">
              <Input placeholder="如：甜蕉" />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={5}>
            <Form.Item label="别称2（如地域称呼）" name="alias2">
              <Input placeholder="如：甘蕉" />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={6}>
            <Form.Item label="显示单位" name="unit">
              <Input />
            </Form.Item>
          </Col>
        </Row>
        <Row gutter={[12, 8]}>
          <Col xs={24} sm={12} lg={4}>
            <Form.Item label="规格" name="spec">
              <Input />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={4}>
            <Form.Item label="基础单位" name="base_unit">
              <Input />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={6}>
            <Form.Item label="基础单价（每基础单位，售价）" name="base_price">
              <InputNumber step={0.01} style={{ width: '100%' }} />
            </Form.Item>
          </Col>
          {isSuperAdmin && (
            <Col xs={24} sm={12} lg={6}>
              <Form.Item label="当前进价（每基础单位，最近采购价）" name="purchase_price">
                <InputNumber step={0.01} style={{ width: '100%' }} onChange={onMarkupChange} />
              </Form.Item>
            </Col>
          )}
        </Row>
        <Row gutter={[12, 8]}>
          <Col xs={24} sm={12} lg={8}>
            <Form.Item label="分类" name="category_id">
              <Select
                allowClear
                placeholder="无分类"
                options={categories.map((c) => ({ value: c.id, label: c.name }))}
              />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={4}>
            <Form.Item label="加成率（毛利率）" name="markup_rate">
              <InputNumber step={0.01} min={0} max={5} style={{ width: '100%' }} onChange={onMarkupChange} />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={5}>
            <Form.Item label="售价自动更新" name="auto_update_price">
              <Select
                options={[
                  { value: 0, label: '关闭' },
                  { value: 1, label: '开启' },
                ]}
              />
            </Form.Item>
          </Col>
          {isSuperAdmin && (
            <>
              <Col xs={24} sm={12} lg={3}>
                <Form.Item label="历史最高进价（自动）" name="max_purchase_price">
                  <InputNumber readOnly style={{ width: '100%', background: '#f5f5f5' }} />
                </Form.Item>
              </Col>
              <Col xs={24} sm={12} lg={3}>
                <Form.Item label="历史最低进价（自动）" name="min_purchase_price">
                  <InputNumber readOnly style={{ width: '100%', background: '#f5f5f5' }} />
                </Form.Item>
              </Col>
            </>
          )}
        </Row>
        <Row gutter={[12, 8]}>
          <Col xs={24} sm={12} lg={7}>
            <Form.Item label="保质期（如：7天、180天）" name="shelf_life">
              <Input placeholder="如：7天" />
            </Form.Item>
          </Col>
        </Row>

        <Divider orientation="left" style={{ margin: '8px 0' }}>
          价格管理
        </Divider>
        <div style={{ background: '#e6f7ff', padding: '8px 12px', borderRadius: 4, fontSize: 12 }}>
          售价计算规则：若有政采平台价则以政采平台价为售价；否则取商超1/商超2/商超3/AI实时采集价的非零平均价（四舍五入保留2位）；若无任何价格则使用基础单价。
        </div>
        <Row gutter={[12, 8]} style={{ marginTop: 12 }}>
          <Col xs={24} sm={12} lg={8}>
            <Form.Item label="政采平台状态" name="gov_status">
              <Select
                options={[
                  { value: 0, label: '非政采商品' },
                  { value: 1, label: '政采商品（有指导价）' },
                  { value: 2, label: '政采商品（无指导价）' },
                ]}
              />
            </Form.Item>
          </Col>
        </Row>
        <Row gutter={[12, 8]}>
          <Col xs={24} sm={12} lg={6}>
            <Form.Item label="政采平台价" name="gov_price">
              <InputNumber step={0.01} style={{ width: '100%' }} onChange={onPriceChange} />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={6}>
            <Form.Item label="商超1零售价" name="supermarket_1">
              <InputNumber step={0.01} style={{ width: '100%' }} onChange={onPriceChange} />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={6}>
            <Form.Item label="商超2零售价" name="supermarket_2">
              <InputNumber step={0.01} style={{ width: '100%' }} onChange={onPriceChange} />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={6}>
            <Form.Item label="商超3零售价" name="supermarket_3">
              <InputNumber step={0.01} style={{ width: '100%' }} onChange={onPriceChange} />
            </Form.Item>
          </Col>
        </Row>
        <Row gutter={[12, 8]}>
          <Col xs={24} sm={12} lg={8}>
            <Form.Item label="AI实时采集价（预留）" name="ai_realtime">
              <InputNumber step={0.01} style={{ width: '100%' }} onChange={onPriceChange} />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={8}>
            <Form.Item label="计算售价（只读）" name="selling_price">
              <InputNumber readOnly style={{ width: '100%', background: '#f5f5f5' }} />
            </Form.Item>
          </Col>
        </Row>

        <Divider orientation="left" style={{ margin: '8px 0' }}>
          商品图片
        </Divider>
        <Space align="start" size="large">
          <div
            style={{
              width: 120,
              height: 120,
              background: '#f5f5f5',
              borderRadius: 8,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              overflow: 'hidden',
              border: imageUrl ? 'none' : '2px dashed #ddd',
              color: '#999',
            }}
          >
            {imageUrl ? (
              <img src={imageUrl} alt="商品图片" style={{ width: 120, height: 120, objectFit: 'cover' }} />
            ) : (
              <span>暂无图片</span>
            )}
          </div>
          <Space direction="vertical">
            <Space>
              <Upload {...uploadProps}>
                <Button size="small">上传图片</Button>
              </Upload>
              {imageUrl && (
                <Button
                  size="small"
                  danger
                  onClick={async () => {
                    if (!product) return
                    const res = await deleteProductImage(product.id)
                    if (res.ok) {
                      setImageUrl(null)
                      message.success('图片已删除')
                    } else {
                      message.error(res.message || '删除失败')
                    }
                  }}
                >
                  删除图片
                </Button>
              )}
            </Space>
            <span style={{ color: '#999', fontSize: 12 }}>支持 JPG、PNG、GIF、WebP 格式，最大 5MB</span>
            <Space.Compact style={{ width: 420 }}>
              <Input
                value={imageUrlInput}
                placeholder="粘贴网络图片URL（http/https），自动下载转存到本系统"
                onChange={(e) => setImageUrlInput(e.target.value)}
              />
              <Button
                onClick={async () => {
                  if (!product) return
                  const url = imageUrlInput.trim()
                  if (!url) {
                    message.warning('请先粘贴图片URL')
                    return
                  }
                  const res = await applyProductImageUrl(product.id, url)
                  if (res.url) {
                    setImageUrl(res.url)
                    setImageUrlInput('')
                    message.success('图片已应用')
                  } else {
                    message.error(res.error || '应用失败')
                  }
                }}
              >
                应用URL图片
              </Button>
            </Space.Compact>
          </Space>
        </Space>

        <Divider orientation="left" style={{ margin: '8px 0' }}>
          多单位设置
        </Divider>
        <div style={{ background: '#e6f7ff', padding: '8px 12px', borderRadius: 4, fontSize: 12, marginBottom: 8 }}>
          示例：基础单位为「斤」，新增「件」单位，1件=20斤，则换算比例填 20；若整件批发价 55 元（比按比例算的 60 元便宜），则在单位单价填 55，留 0 则自动按比例计算。单位采购价用于整采整卖场景，留 0 则使用进价按比例计算。
        </div>
        <Button
          size="small"
          type="primary"
          style={{ marginBottom: 8 }}
          onClick={() => setUnits((prev) => [...prev, { unit_name: '', ratio: 1, unit_price: 0, purchase_price: 0, sort_order: 0 }])}
        >
          + 添加单位
        </Button>
        <Table<ProductUnit>
          rowKey={(_r, i) => String(i)}
          size="small"
          pagination={false}
          dataSource={units}
          columns={unitColumns()}
          scroll={scrollX('max-content')}
        />
      </Form>
    </Modal>
  )
}
