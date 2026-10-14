import { useEffect, useMemo, useState } from 'react'
import {
  App,
  Button,
  Card,
  Col,
  Form,
  Input,
  Modal,
  Row,
  Select,
  Space,
  Table,
  Tag,
  Typography,
  Upload,
} from 'antd'
import type { TableProps } from 'antd'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useSearchParams } from 'react-router-dom'
import {
  PURCHASER_EXPORT_URL,
  approvePurchaser,
  createPurchaser,
  deletePurchaser,
  importPurchaser,
  listCategoryTree,
  listPurchasers,
  unapprovePurchaser,
  updatePurchaser,
  type CategoryTreeNode,
  type Purchaser,
} from '../../api/admin/basic'
import { useAuth } from '../../stores/auth'

interface CreateFormValues {
  name: string
  contact?: string
  phone?: string
  address?: string
  category_id?: number
  business_scope?: string
  remark?: string
}

interface EditFormValues {
  name: string
  contact?: string
  phone?: string
  address?: string
  business_scope?: string
  remark?: string
  category_id?: number
}

function truncateText(text: string | null, maxLen: number): string {
  if (!text) return ''
  return text.length > maxLen ? text.slice(0, maxLen) + '...' : text
}

export default function PurchaserPage() {
  const { message, modal } = App.useApp()
  const { user } = useAuth()
  const isSuperAdmin = user?.role === 'super_admin'
  const queryClient = useQueryClient()
  const [searchParams] = useSearchParams()

  const [categoryId, setCategoryId] = useState<number | null>(() => {
    const cid = searchParams.get('category_id')
    return cid ? Number(cid) : null
  })
  const [keywordInput, setKeywordInput] = useState('')
  const [keyword, setKeyword] = useState('')
  const [editing, setEditing] = useState<Purchaser | null>(null)

  const [createForm] = Form.useForm<CreateFormValues>()
  const [editForm] = Form.useForm<EditFormValues>()

  const { data: catTree = [] } = useQuery({
    queryKey: ['categoryTree', 'purchaser'],
    queryFn: () => listCategoryTree('purchaser'),
  })

  const { data: purchasers = [], isLoading } = useQuery({
    queryKey: ['purchasers', { categoryId, keyword }],
    queryFn: () => listPurchasers(categoryId, keyword),
  })

  const categoryOptions = useMemo(() => collectOptions(catTree), [catTree])

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['purchasers'] })

  // 分类筛选由全局侧边栏导航驱动（/purchaser?category_id=N），URL 变化时同步
  useEffect(() => {
    const cid = searchParams.get('category_id')
    setCategoryId(cid ? Number(cid) : null)
  }, [searchParams])

  // 当前筛选分类的名称（用于列表标题展示），从分类树推导
  const categoryName = useMemo(() => {
    if (!categoryId) return ''
    return findNode(catTree, categoryId)?.name ?? ''
  }, [catTree, categoryId])

  // 点击分类后，新增表单的分类默认跟随当前筛选分类（照老页面 setFormCategory）
  useEffect(() => {
    createForm.setFieldsValue({ category_id: categoryId ?? undefined })
  }, [categoryId, createForm])

  const runAction = async (
    action: () => Promise<{ ok: boolean; message: string }>,
    successText: string,
    failPrefix: string,
  ) => {
    const res = await action()
    if (res.ok) {
      message.success(successText)
      refresh()
    } else {
      message.error(`${failPrefix}：${res.message || '操作失败'}`)
    }
  }

  const confirmThen = (title: string, action: () => void) => {
    modal.confirm({
      title,
      okText: '确定',
      cancelText: '取消',
      onOk: action,
    })
  }

  const handleCreate = async () => {
    const values = await createForm.validateFields()
    const res = await createPurchaser({
      name: values.name,
      contact: values.contact || null,
      phone: values.phone || null,
      address: values.address || null,
      business_scope: values.business_scope || null,
      remark: values.remark || null,
      category_id: values.category_id ?? null,
    })
    if (res.ok) {
      message.success(res.message || '创建成功')
      createForm.resetFields()
      createForm.setFieldsValue({ category_id: categoryId ?? undefined })
      refresh()
    } else {
      message.error(res.message || '创建失败')
    }
  }

  const openEdit = (row: Purchaser) => {
    if (row.audit_status === 'confirmed') {
      message.warning('该采购方已审核，如需修改请先反审核')
      return
    }
    setEditing(row)
  }

  useEffect(() => {
    if (!editing) return
    editForm.setFieldsValue({
      name: editing.name,
      contact: editing.contact ?? '',
      phone: editing.phone ?? '',
      address: editing.address ?? '',
      business_scope: editing.business_scope ?? '',
      remark: editing.remark ?? '',
      category_id: editing.category_id ?? undefined,
    })
  }, [editing, editForm])

  const submitEdit = async () => {
    if (!editing) return
    const values = await editForm.validateFields()
    const res = await updatePurchaser({
      id: editing.id,
      name: values.name,
      contact: values.contact || null,
      phone: values.phone || null,
      address: values.address || null,
      business_scope: values.business_scope || null,
      remark: values.remark || null,
      category_id: values.category_id ?? null,
    })
    if (res.ok) {
      message.success(res.message || '保存成功')
      setEditing(null)
      refresh()
    } else {
      message.error(res.message || '保存失败')
    }
  }

  const handleDelete = (row: Purchaser) => {
    if (row.audit_status === 'confirmed') {
      message.warning('该采购方已审核，如需删除请先反审核')
      return
    }
    confirmThen(`确定要删除采购方「${row.name}」吗？`, () =>
      runAction(() => deletePurchaser(row.id), '删除成功', '删除失败'),
    )
  }

  const resetSearch = () => {
    setKeywordInput('')
    setKeyword('')
    setCategoryId(null)
  }

  const columns: TableProps<Purchaser>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 60 },
    { title: '名称', dataIndex: 'name', width: 180 },
    { title: '联系人', dataIndex: 'contact', width: 90, render: (v: string | null) => v || '' },
    { title: '电话', dataIndex: 'phone', width: 110, render: (v: string | null) => v || '' },
    { title: '地址', dataIndex: 'address', width: 180, render: (v: string | null) => v || '' },
    {
      title: '经营范围',
      dataIndex: 'business_scope',
      width: 170,
      render: (v: string | null) => <span title={v || ''}>{truncateText(v, 20)}</span>,
    },
    {
      title: '备注',
      dataIndex: 'remark',
      width: 160,
      render: (v: string | null) => <span title={v || ''}>{truncateText(v, 20)}</span>,
    },
    {
      title: '分类',
      dataIndex: 'category_name',
      width: 100,
      render: (v: string | null) => v || '无分类',
    },
    {
      title: '审核状态',
      dataIndex: 'audit_status',
      width: 90,
      render: (v: string | null) =>
        v === 'confirmed' ? <Tag color="success">已审核</Tag> : <Tag color="warning">待审核</Tag>,
    },
    {
      title: '操作',
      key: 'action',
      width: 240,
      render: (_v, row) => (
        <Space size={4} wrap>
          {isSuperAdmin &&
            (row.audit_status === 'confirmed' ? (
              <Button
                size="small"
                onClick={() =>
                  confirmThen('确定反审核该采购方吗？反审核后需重新审核。', () =>
                    runAction(() => unapprovePurchaser(row.id), '反审核成功', '反审核失败'),
                  )
                }
              >
                反审核
              </Button>
            ) : (
              <Button
                size="small"
                type="primary"
                ghost
                onClick={() =>
                  confirmThen('确定审核通过该采购方吗？', () =>
                    runAction(() => approvePurchaser(row.id), '审核成功', '审核失败'),
                  )
                }
              >
                审核
              </Button>
            ))}
          <Button size="small" onClick={() => openEdit(row)}>
            编辑
          </Button>
          <Button size="small" danger onClick={() => handleDelete(row)}>
            删除
          </Button>
        </Space>
      ),
    },
  ]

  return (
    <>
      <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
          <Card size="small" title="新增采购单位">
            <Form form={createForm} onFinish={handleCreate} layout="vertical">
              <Row gutter={[8, 8]}>
                <Col xs={24} sm={12} lg={3}>
                  <Form.Item
                    name="name"
                    rules={[{ required: true, message: '请输入单位名称' }]}
                    style={{ marginBottom: 8 }}
                  >
                    <Input placeholder="单位名称" />
                  </Form.Item>
                </Col>
                <Col xs={24} sm={12} lg={2}>
                  <Form.Item name="contact" style={{ marginBottom: 8 }}>
                    <Input placeholder="联系人" />
                  </Form.Item>
                </Col>
                <Col xs={24} sm={12} lg={2}>
                  <Form.Item name="phone" style={{ marginBottom: 8 }}>
                    <Input placeholder="电话" />
                  </Form.Item>
                </Col>
                <Col xs={24} sm={12} lg={3}>
                  <Form.Item name="address" style={{ marginBottom: 8 }}>
                    <Input placeholder="地址" />
                  </Form.Item>
                </Col>
                <Col xs={24} sm={12} lg={2}>
                  <Form.Item name="category_id" style={{ marginBottom: 8 }}>
                    <Select placeholder="无分类" allowClear options={categoryOptions} />
                  </Form.Item>
                </Col>
                <Col xs={24} sm={12} lg={4}>
                  <Form.Item name="business_scope" style={{ marginBottom: 8 }}>
                    <Input placeholder="经营范围" />
                  </Form.Item>
                </Col>
                <Col xs={24} sm={12} lg={4}>
                  <Form.Item name="remark" style={{ marginBottom: 8 }}>
                    <Input placeholder="备注" />
                  </Form.Item>
                </Col>
                <Col xs={24} sm={12} lg={2}>
                  <Button type="primary" htmlType="submit" block>
                    新增
                  </Button>
                </Col>
              </Row>
            </Form>
          </Card>

          <Card size="small">
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                flexWrap: 'wrap',
                gap: 8,
              }}
            >
              <Typography.Title level={5} style={{ margin: 0 }}>
                {categoryId ? `分类采购方 - ${categoryName}` : '全部采购方'}
              </Typography.Title>
              <Space wrap>
                <Input
                  placeholder="搜索采购方名称"
                  style={{ width: 200 }}
                  value={keywordInput}
                  onChange={(e) => setKeywordInput(e.target.value)}
                  onPressEnter={() => setKeyword(keywordInput.trim())}
                />
                <Button type="primary" onClick={() => setKeyword(keywordInput.trim())}>
                  搜索
                </Button>
                <Button onClick={resetSearch}>显示全部</Button>
                <Button type="primary" ghost href={PURCHASER_EXPORT_URL}>
                  导出
                </Button>
                <Upload
                  accept=".xlsx,.csv"
                  showUploadList={false}
                  beforeUpload={(file) => {
                    void (async () => {
                      const res = await importPurchaser(file)
                      if (res.ok) {
                        message.success(res.message || '导入完成')
                        refresh()
                      } else {
                        message.error(res.message || '导入失败')
                      }
                    })()
                    return false
                  }}
                >
                  <Button>导入</Button>
                </Upload>
              </Space>
            </div>
          </Card>

          <Table<Purchaser>
            rowKey="id"
            size="small"
            bordered
            loading={isLoading}
            dataSource={purchasers}
            columns={columns}
            pagination={false}
            scroll={{ x: 1100 }}
            locale={{ emptyText: '暂无采购方数据' }}
          />
        </Space>

      <Modal
        open={!!editing}
        title="编辑采购方"
        okText="保存"
        cancelText="取消"
        onOk={submitEdit}
        onCancel={() => setEditing(null)}
      >
        <Form form={editForm} layout="vertical">
          <Form.Item
            label="单位名称"
            name="name"
            rules={[{ required: true, message: '请输入单位名称' }]}
          >
            <Input />
          </Form.Item>
          <Form.Item label="联系人" name="contact">
            <Input />
          </Form.Item>
          <Form.Item label="电话" name="phone">
            <Input />
          </Form.Item>
          <Form.Item label="地址" name="address">
            <Input />
          </Form.Item>
          <Form.Item label="经营范围" name="business_scope">
            <Input.TextArea rows={2} />
          </Form.Item>
          <Form.Item label="备注" name="remark">
            <Input.TextArea rows={2} />
          </Form.Item>
          <Form.Item label="分类" name="category_id">
            <Select placeholder="无分类" allowClear options={categoryOptions} />
          </Form.Item>
        </Form>
      </Modal>
    </>
  )
}

/** 在分类树中按 id 查找节点（用于筛选分类名称展示） */
function findNode(nodes: CategoryTreeNode[], id: number): CategoryTreeNode | null {
  for (const n of nodes) {
    if (n.id === id) return n
    const hit = findNode(n.children ?? [], id)
    if (hit) return hit
  }
  return null
}

/** 拍平分类树供 Select 使用（新增/编辑表单的分类下拉） */
function collectOptions(nodes: CategoryTreeNode[]): { value: number; label: string }[] {
  const out: { value: number; label: string }[] = []
  const walk = (list: CategoryTreeNode[], prefix: string) => {
    for (const n of list) {
      const label = prefix ? `${prefix} / ${n.name}` : n.name
      out.push({ value: n.id, label })
      walk(n.children ?? [], label)
    }
  }
  walk(nodes, '')
  return out
}
