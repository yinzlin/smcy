import { useState } from 'react'
import { App, Button, Card, Col, Form, Input, InputNumber, Modal, Row, Select, Table } from 'antd'
import type { TableProps } from 'antd'
import { checkProductName, createProduct, type Category, type Product } from '../../api/product'
import { scrollX } from '../../utils/tableScroll'

interface Props {
  categories: Category[]
  isSuperAdmin: boolean
  onCreated: () => void
}

/** 顶部快速新增表单：先做同名检测，命中时弹窗前预先查看再决定是否继续 */
export default function CreateProductCard({ categories, isSuperAdmin, onCreated }: Props) {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [submitting, setSubmitting] = useState(false)
  const [duplicates, setDuplicates] = useState<Product[] | null>(null)
  const [pending, setPending] = useState<{ values: Record<string, unknown> } | null>(null)

  const submit = async () => {
    const values = await form.validateFields()
    setSubmitting(true)
    try {
      const dup = await checkProductName(values.name)
      if (dup && dup.length > 0) {
        setPending({ values })
        setDuplicates(dup)
        return
      }
      await doCreate(values)
    } finally {
      setSubmitting(false)
    }
  }

  const doCreate = async (values: Record<string, unknown>) => {
    const payload = {
      name: String(values.name ?? ''),
      spec: (values.spec as string) || null,
      unit: (values.unit as string) || null,
      base_unit: (values.base_unit as string) || null,
      base_price: (values.base_price as number) ?? null,
      purchase_price: isSuperAdmin ? ((values.purchase_price as number) ?? null) : null,
      category_id: (values.category_id as number) ?? null,
      shelf_life: (values.shelf_life as string) || null,
      gov_status: 0,
    }
    const res = await createProduct(payload)
    if (res.ok) {
      form.resetFields()
      form.setFieldValue('base_unit', '斤')
      message.success('新增成功')
      onCreated()
    } else {
      message.error(res.message || '新增失败')
    }
  }

  const duplicateColumns: TableProps<Product>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    { title: '名称', dataIndex: 'name' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '' },
    { title: '单位', dataIndex: 'unit', width: 90 },
    { title: '单价', dataIndex: 'base_price', width: 90, render: (v: number) => (v || 0).toFixed(2) },
    { title: '分类', dataIndex: 'category_name', render: (v: string | null) => v || '无分类' },
  ]

  return (
    <Card title="新增商品" size="small">
      <Form form={form} layout="vertical" initialValues={{ base_unit: '斤' }}>
        <Row gutter={[12, 8]}>
          <Col xs={24} sm={12} lg={4}>
            <Form.Item name="name" rules={[{ required: true, message: '请输入商品名称' }]}>
              <Input placeholder="商品名称" />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={4}>
            <Form.Item name="spec">
              <Input placeholder="规格" />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={2}>
            <Form.Item name="unit">
              <Input placeholder="显示单位" />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={2}>
            <Form.Item name="base_unit">
              <Input placeholder="基础单位" />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={3}>
            <Form.Item name="base_price">
              <InputNumber placeholder="基础单价(售价)" step={0.01} style={{ width: '100%' }} />
            </Form.Item>
          </Col>
          {isSuperAdmin && (
            <Col xs={24} sm={12} lg={3}>
              <Form.Item name="purchase_price">
                <InputNumber placeholder="进价" step={0.01} style={{ width: '100%' }} />
              </Form.Item>
            </Col>
          )}
          <Col xs={24} sm={12} lg={3}>
            <Form.Item name="category_id">
              <Select
                placeholder="分类"
                allowClear
                options={categories.map((c) => ({ value: c.id, label: c.name }))}
              />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={2}>
            <Form.Item name="shelf_life">
              <Input placeholder="保质期" />
            </Form.Item>
          </Col>
          <Col xs={24} sm={12} lg={2}>
            <Form.Item>
              <Button type="primary" loading={submitting} onClick={submit} block>
                新增
              </Button>
            </Form.Item>
          </Col>
        </Row>
      </Form>

      <Modal
        title="提示：存在同名商品"
        open={!!duplicates}
        onCancel={() => {
          setDuplicates(null)
          setPending(null)
        }}
        okText="继续新增"
        cancelText="取消"
        width={720}
        onOk={async () => {
          const values = pending?.values
          setDuplicates(null)
          setPending(null)
          if (values) await doCreate(values)
        }}
      >
        <p>发现以下同名商品，请确认是否继续新增：</p>
        <Table<Product>
          rowKey="id"
          size="small"
          pagination={false}
          dataSource={duplicates ?? []}
          columns={duplicateColumns}
          scroll={scrollX('max-content')}
        />
      </Modal>
    </Card>
  )
}
