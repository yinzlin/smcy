import { useEffect, useMemo, useState } from 'react'
import type { Key, ReactNode } from 'react'
import {
  App,
  Button,
  Card,
  Col,
  Dropdown,
  Form,
  Input,
  Modal,
  Row,
  Select,
  Space,
  Table,
  Tag,
  Tree,
  Typography,
  Upload,
} from 'antd'
import type { MenuProps, TableProps, TreeDataNode } from 'antd'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useSearchParams } from 'react-router-dom'
import {
  PURCHASER_EXPORT_URL,
  approvePurchaser,
  createCategory,
  createPurchaser,
  deleteCategory,
  deletePurchaser,
  importPurchaser,
  listCategoryTree,
  listPurchasers,
  renameCategory,
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

type CatModalMode = 'root' | 'sub' | 'sibling' | 'rename'

const ROOT_KEY = 'all'

function truncateText(text: string | null, maxLen: number): string {
  if (!text) return ''
  return text.length > maxLen ? text.slice(0, maxLen) + '...' : text
}

const CAT_MENU_ITEMS: MenuProps['items'] = [
  { key: 'sub', label: '新增子分类' },
  { key: 'sibling', label: '新增同级分类' },
  { type: 'divider' },
  { key: 'rename', label: '重命名' },
  { key: 'delete', label: '删除', danger: true },
]

export default function PurchaserPage() {
  const { message, modal } = App.useApp()
  const { user } = useAuth()
  const isSuperAdmin = user?.role === 'super_admin'
  const queryClient = useQueryClient()
  const [searchParams] = useSearchParams()

  const initialCatId = searchParams.get('category_id')

  const [categoryId, setCategoryId] = useState<number | null>(
    initialCatId ? Number(initialCatId) : null,
  )
  const [categoryName, setCategoryName] = useState(initialCatId ? '分类采购方' : '')
  const [keywordInput, setKeywordInput] = useState('')
  const [keyword, setKeyword] = useState('')
  const [editing, setEditing] = useState<Purchaser | null>(null)

  const [catModal, setCatModal] = useState<{ mode: CatModalMode; target: CategoryTreeNode | null } | null>(null)
  const [catName, setCatName] = useState('')
  const [expanded, setExpanded] = useState<Key[]>([])

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
  const invalidateTree = () => queryClient.invalidateQueries({ queryKey: ['categoryTree', 'purchaser'] })

  const selectCategory = (id: number | null, name: string) => {
    setCategoryId(id)
    setCategoryName(name)
  }

  // 点击分类后，新增表单的分类默认跟随当前筛选分类（照老页面 setFormCategory）
  useEffect(() => {
    createForm.setFieldsValue({ category_id: categoryId ?? undefined })
  }, [categoryId, createForm])

  const findNode = (nodes: CategoryTreeNode[], id: number): CategoryTreeNode | null => {
    for (const n of nodes) {
      if (n.id === id) return n
      const hit = findNode(n.children ?? [], id)
      if (hit) return hit
    }
    return null
  }

  const collectKeys = (nodes: CategoryTreeNode[]): Key[] =>
    nodes.flatMap((n) => [n.id as Key, ...collectKeys(n.children ?? [])])

  const allTreeKeys = useMemo(() => collectKeys(catTree), [catTree])

  useEffect(() => {
    setExpanded(allTreeKeys)
  }, [allTreeKeys])

  const renderNodeTitle = (node: CategoryTreeNode): ReactNode => (
    <Dropdown
      trigger={['contextMenu']}
      menu={{
        items: CAT_MENU_ITEMS,
        onClick: ({ key, domEvent }) => {
          domEvent.stopPropagation()
          if (key === 'sub') openCatModal('sub', node)
          else if (key === 'sibling') openCatModal('sibling', node)
          else if (key === 'rename') openCatModal('rename', node)
          else if (key === 'delete') handleDeleteCategory(node)
        },
      }}
    >
      <span style={{ userSelect: 'none' }}>📂 {node.name}</span>
    </Dropdown>
  )

  const toTreeData = (nodes: CategoryTreeNode[]): TreeDataNode[] =>
    nodes.map((n) => ({
      key: n.id,
      title: renderNodeTitle(n),
      children: toTreeData(n.children ?? []),
    }))

  const treeData: TreeDataNode[] = [{ key: ROOT_KEY, title: '全部采购方' }, ...toTreeData(catTree)]

  const openCatModal = (mode: CatModalMode, target: CategoryTreeNode | null = null) => {
    setCatName(mode === 'rename' && target ? target.name : '')
    setCatModal({ mode, target })
  }

  const catModalTitle =
    catModal?.mode === 'root'
      ? '请输入新的顶级分类名称'
      : catModal?.mode === 'sub'
        ? '请输入新的子分类名称'
        : catModal?.mode === 'sibling'
          ? '请输入新的同级分类名称'
          : '请输入新的分类名称'

  const submitCatModal = async () => {
    if (!catModal) return
    const name = catName.trim()
    if (!name) return
    const { mode, target } = catModal
    if (mode === 'rename') {
      if (!target || name === target.name) {
        setCatModal(null)
        return
      }
      const res = await renameCategory(target.id, name)
      if (res.ok) {
        message.success(res.message || '重命名成功')
        setCatModal(null)
        invalidateTree()
      } else {
        message.error(res.message || '重命名失败')
      }
      return
    }
    const parent_id =
      mode === 'root' ? null : mode === 'sub' ? (target?.id ?? null) : (target?.parent_id ?? null)
    const res = await createCategory({ name, parent_id, entity_type: 'purchaser', sort_order: 0 })
    if (res.ok) {
      message.success(res.message || '创建成功')
      setCatModal(null)
      invalidateTree()
    } else {
      message.error(res.message || '创建失败')
    }
  }

  const handleDeleteCategory = (node: CategoryTreeNode) => {
    modal.confirm({
      title: `确定要删除分类"${node.name}"吗？`,
      content: '注意：有子分类或已被引用的分类无法删除。',
      okText: '确定',
      cancelText: '取消',
      onOk: async () => {
        const res = await deleteCategory(node.id)
        if (res.ok) {
          message.success(res.message || '删除成功')
          if (categoryId === node.id) selectCategory(null, '')
          invalidateTree()
        } else {
          message.error(res.message || '删除失败')
        }
      },
    })
  }

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
    selectCategory(null, '')
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
    <Row gutter={16}>
      <Col flex="240px" style={{ minWidth: 200 }}>
        <Card
          size="small"
          title="采购方分类"
          extra={
            <Space size={4}>
              <Button size="small" type="link" style={{ padding: 0 }} onClick={() => openCatModal('root')}>
                新增顶级分类
              </Button>
              <Button size="small" type="link" style={{ padding: 0 }} onClick={invalidateTree}>
                刷新分类树
              </Button>
            </Space>
          }
        >
          <Tree
            blockNode
            treeData={treeData}
            selectedKeys={categoryId ? [categoryId] : [ROOT_KEY]}
            expandedKeys={expanded}
            onExpand={(keys) => setExpanded(keys)}
            onSelect={(keys) => {
              const key = keys[0]
              if (key === undefined || key === ROOT_KEY) {
                selectCategory(null, '')
                return
              }
              const node = findNode(catTree, Number(key))
              if (node) selectCategory(node.id, node.name)
            }}
          />
        </Card>
      </Col>
      <Col flex="auto">
        <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
          <Card size="small" title="新增采购单位">
            <Form form={createForm} onFinish={handleCreate} layout="vertical">
              <Row gutter={8}>
                <Col span={3}>
                  <Form.Item
                    name="name"
                    rules={[{ required: true, message: '请输入单位名称' }]}
                    style={{ marginBottom: 8 }}
                  >
                    <Input placeholder="单位名称" />
                  </Form.Item>
                </Col>
                <Col span={2}>
                  <Form.Item name="contact" style={{ marginBottom: 8 }}>
                    <Input placeholder="联系人" />
                  </Form.Item>
                </Col>
                <Col span={2}>
                  <Form.Item name="phone" style={{ marginBottom: 8 }}>
                    <Input placeholder="电话" />
                  </Form.Item>
                </Col>
                <Col span={3}>
                  <Form.Item name="address" style={{ marginBottom: 8 }}>
                    <Input placeholder="地址" />
                  </Form.Item>
                </Col>
                <Col span={2}>
                  <Form.Item name="category_id" style={{ marginBottom: 8 }}>
                    <Select placeholder="无分类" allowClear options={categoryOptions} />
                  </Form.Item>
                </Col>
                <Col span={4}>
                  <Form.Item name="business_scope" style={{ marginBottom: 8 }}>
                    <Input placeholder="经营范围" />
                  </Form.Item>
                </Col>
                <Col span={4}>
                  <Form.Item name="remark" style={{ marginBottom: 8 }}>
                    <Input placeholder="备注" />
                  </Form.Item>
                </Col>
                <Col span={2}>
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
      </Col>

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

      <Modal
        open={!!catModal}
        title={catModalTitle}
        okText="确定"
        cancelText="取消"
        onOk={submitCatModal}
        onCancel={() => setCatModal(null)}
      >
        <Input
          value={catName}
          placeholder="分类名称"
          onChange={(e) => setCatName(e.target.value)}
          onPressEnter={submitCatModal}
        />
      </Modal>
    </Row>
  )
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
