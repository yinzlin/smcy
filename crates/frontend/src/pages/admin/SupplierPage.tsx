import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import {
  App,
  Button,
  Card,
  Checkbox,
  Col,
  DatePicker,
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
import type { Dayjs } from 'dayjs'
import dayjs from 'dayjs'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useSearchParams } from 'react-router-dom'
import {
  SUPPLIER_EXPORT_URL,
  approveSupplier,
  createSupplier,
  deleteSupplier,
  importSupplier,
  listCategoryTree,
  listSuppliers,
  unapproveSupplier,
  updateSupplier,
  type CategoryTreeNode,
  type Supplier,
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
  is_designated?: boolean
  designated_no?: string
}

interface EditFormValues {
  name: string
  category_id?: number
  is_designated?: boolean
  designated_no?: string
  legal_person?: string
  phone?: string
  contact?: string
  credit_code?: string
  address?: string
  license_no?: string
  license_expire?: Dayjs | null
  contract_no?: string
  contract_start?: Dayjs | null
  contract_end?: Dayjs | null
  business_scope?: string
  remark?: string
}

/** 证照/合同到期状态：已过期红色，30天内黄色，否则绿色；未填显示 — */
function expireBadge(label: string, dateStr: string): ReactNode {
  const today = dayjs().startOf('day')
  const d = dayjs(dateStr).startOf('day')
  const days = d.diff(today, 'day')
  if (days < 0) {
    return (
      <div style={{ color: '#dc3545', fontWeight: 600 }}>
        {label} {dateStr} 已过期
      </div>
    )
  }
  if (days <= 30) {
    return (
      <div style={{ color: '#d48806', fontWeight: 600 }}>
        {label} {dateStr} 剩{days}天
      </div>
    )
  }
  return (
    <div style={{ color: '#198754' }}>
      {label} {dateStr}
    </div>
  )
}

function truncateText(text: string | null, maxLen: number): string {
  if (!text) return ''
  return text.length > maxLen ? text.slice(0, maxLen) + '...' : text
}

export default function SupplierPage() {
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
  const [editing, setEditing] = useState<Supplier | null>(null)

  const [createForm] = Form.useForm<CreateFormValues>()
  const [editForm] = Form.useForm<EditFormValues>()

  const { data: catTree = [] } = useQuery({
    queryKey: ['categoryTree', 'supplier'],
    queryFn: () => listCategoryTree('supplier'),
  })

  const { data: suppliers = [], isLoading } = useQuery({
    queryKey: ['suppliers', { categoryId, keyword }],
    queryFn: () => listSuppliers(categoryId, keyword),
  })

  const categoryOptions = useMemo(() => collectOptions(catTree), [catTree])

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['suppliers'] })

  // 分类筛选由全局侧边栏导航驱动（/supplier?category_id=N），URL 变化时同步
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
    const res = await createSupplier({
      name: values.name,
      contact: values.contact || null,
      phone: values.phone || null,
      address: values.address || null,
      business_scope: values.business_scope || null,
      remark: values.remark || null,
      category_id: values.category_id ?? null,
      is_designated: values.is_designated ? 1 : 0,
      designated_no: values.designated_no || null,
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

  const openEdit = (row: Supplier) => {
    if (row.audit_status === 'confirmed') {
      message.warning('该供应商已审核，如需修改请先反审核')
      return
    }
    setEditing(row)
  }

  useEffect(() => {
    if (!editing) return
    editForm.setFieldsValue({
      name: editing.name,
      category_id: editing.category_id ?? undefined,
      is_designated: editing.is_designated === 1,
      designated_no: editing.designated_no ?? '',
      legal_person: editing.legal_person ?? '',
      phone: editing.phone ?? '',
      contact: editing.contact ?? '',
      credit_code: editing.credit_code ?? '',
      address: editing.address ?? '',
      license_no: editing.license_no ?? '',
      license_expire: editing.license_expire ? dayjs(editing.license_expire) : null,
      contract_no: editing.contract_no ?? '',
      contract_start: editing.contract_start ? dayjs(editing.contract_start) : null,
      contract_end: editing.contract_end ? dayjs(editing.contract_end) : null,
      business_scope: editing.business_scope ?? '',
      remark: editing.remark ?? '',
    })
  }, [editing, editForm])

  const submitEdit = async () => {
    if (!editing) return
    const values = await editForm.validateFields()
    const res = await updateSupplier({
      id: editing.id,
      name: values.name,
      contact: values.contact || null,
      phone: values.phone || null,
      address: values.address || null,
      business_scope: values.business_scope || null,
      remark: values.remark || null,
      category_id: values.category_id ?? null,
      is_designated: values.is_designated ? 1 : 0,
      designated_no: values.designated_no || null,
      legal_person: values.legal_person || null,
      credit_code: values.credit_code || null,
      license_no: values.license_no || null,
      license_expire: values.license_expire ? values.license_expire.format('YYYY-MM-DD') : null,
      contract_no: values.contract_no || null,
      contract_start: values.contract_start ? values.contract_start.format('YYYY-MM-DD') : null,
      contract_end: values.contract_end ? values.contract_end.format('YYYY-MM-DD') : null,
    })
    if (res.ok) {
      message.success(res.message || '保存成功')
      setEditing(null)
      refresh()
    } else {
      message.error(res.message || '保存失败')
    }
  }

  const handleDelete = (row: Supplier) => {
    if (row.audit_status === 'confirmed') {
      message.warning('该供应商已审核，如需删除请先反审核')
      return
    }
    confirmThen(`确定要删除供应商「${row.name}」吗？`, () =>
      runAction(() => deleteSupplier(row.id), '删除成功', '删除失败'),
    )
  }

  const resetSearch = () => {
    setKeywordInput('')
    setKeyword('')
    setCategoryId(null)
  }

  const columns: TableProps<Supplier>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 60 },
    {
      title: '名称',
      dataIndex: 'name',
      width: 180,
      render: (_v, row) => (
        <span>
          {row.name}
          {row.is_designated === 1 && (
            <Tag color="cyan" style={{ marginLeft: 6 }}>
              定点{row.designated_no ? ` ${row.designated_no}` : ''}
            </Tag>
          )}
        </span>
      ),
    },
    { title: '联系人', dataIndex: 'contact', width: 90, render: (v: string | null) => v || '' },
    { title: '电话', dataIndex: 'phone', width: 110, render: (v: string | null) => v || '' },
    { title: '地址', dataIndex: 'address', width: 160, render: (v: string | null) => v || '' },
    {
      title: '经营范围',
      dataIndex: 'business_scope',
      width: 160,
      render: (v: string | null) => (
        <span title={v || ''}>{truncateText(v, 20)}</span>
      ),
    },
    {
      title: '备注',
      dataIndex: 'remark',
      width: 150,
      render: (v: string | null) => <span title={v || ''}>{truncateText(v, 20)}</span>,
    },
    {
      title: '分类',
      dataIndex: 'category_name',
      width: 100,
      render: (v: string | null) => v || '无分类',
    },
    {
      title: '证照/合同到期',
      key: 'expire',
      width: 190,
      render: (_v, row) => {
        const parts: ReactNode[] = []
        if (row.license_expire) parts.push(expireBadge('许可证', row.license_expire))
        if (row.contract_end) parts.push(expireBadge('合同', row.contract_end))
        if (parts.length === 0) return <span style={{ color: '#999' }}>—</span>
        return <div>{parts}</div>
      },
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
      width: 260,
      render: (_v, row) => (
        <Space size={4} wrap>
          {isSuperAdmin &&
            (row.audit_status === 'confirmed' ? (
              <Button
                size="small"
                onClick={() =>
                  confirmThen('确定反审核该供应商吗？反审核后需重新审核。', () =>
                    runAction(() => unapproveSupplier(row.id), '反审核成功', '反审核失败'),
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
                  confirmThen('确定审核通过该供应商吗？', () =>
                    runAction(() => approveSupplier(row.id), '审核成功', '审核失败'),
                  )
                }
              >
                审核
              </Button>
            ))}
          <Button size="small" onClick={() => openEdit(row)}>
            编辑档案
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
          <Card size="small" title="新增供应商">
            <Form form={createForm} onFinish={handleCreate} layout="vertical">
              <Row gutter={8}>
                <Col span={3}>
                  <Form.Item
                    name="name"
                    rules={[{ required: true, message: '请输入供应商名称' }]}
                    style={{ marginBottom: 8 }}
                  >
                    <Input placeholder="供应商名称" />
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
                <Col span={3}>
                  <Form.Item name="remark" style={{ marginBottom: 8 }}>
                    <Input placeholder="备注" />
                  </Form.Item>
                </Col>
                <Col span={2}>
                  <Form.Item name="is_designated" valuePropName="checked" style={{ marginBottom: 8 }}>
                    <Checkbox>定点供应商</Checkbox>
                  </Form.Item>
                </Col>
                <Col span={2}>
                  <Form.Item name="designated_no" style={{ marginBottom: 8 }}>
                    <Input placeholder="定点编号" />
                  </Form.Item>
                </Col>
                <Col span={2}>
                  <Button type="primary" htmlType="submit" block>
                    新增
                  </Button>
                </Col>
              </Row>
            </Form>
            <Typography.Text type="secondary">
              定点供应商的证照、合同等完整档案信息请在新增后点「编辑档案」维护。
            </Typography.Text>
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
                {categoryId ? `分类供应商 - ${categoryName}` : '全部供应商'}
              </Typography.Title>
              <Space wrap>
                <Input
                  placeholder="搜索供应商名称"
                  style={{ width: 200 }}
                  value={keywordInput}
                  onChange={(e) => setKeywordInput(e.target.value)}
                  onPressEnter={() => setKeyword(keywordInput.trim())}
                />
                <Button type="primary" onClick={() => setKeyword(keywordInput.trim())}>
                  搜索
                </Button>
                <Button onClick={resetSearch}>显示全部</Button>
                <Button type="primary" ghost href={SUPPLIER_EXPORT_URL}>
                  导出
                </Button>
                <Upload
                  accept=".xlsx,.csv"
                  showUploadList={false}
                  beforeUpload={(file) => {
                    void (async () => {
                      const res = await importSupplier(file)
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

          <Table<Supplier>
            rowKey="id"
            size="small"
            bordered
            loading={isLoading}
            dataSource={suppliers}
            columns={columns}
            pagination={false}
            scroll={{ x: 1200 }}
            locale={{ emptyText: '暂无供应商数据' }}
          />
        </Space>

      <Modal
        open={!!editing}
        title="供应商档案维护"
        width={800}
        okText="保存"
        cancelText="取消"
        onOk={submitEdit}
        onCancel={() => setEditing(null)}
      >
        <Form form={editForm} layout="vertical">
          <Row gutter={8}>
            <Col span={8}>
              <Form.Item
                label="供应商名称"
                name="name"
                rules={[{ required: true, message: '请输入供应商名称' }]}
              >
                <Input />
              </Form.Item>
            </Col>
            <Col span={4}>
              <Form.Item label="分类" name="category_id">
                <Select placeholder="无分类" allowClear options={categoryOptions} />
              </Form.Item>
            </Col>
            <Col span={3}>
              <Form.Item label="定点供应商" name="is_designated" valuePropName="checked">
                <Checkbox />
              </Form.Item>
            </Col>
            <Col span={3}>
              <Form.Item label="定点编号" name="designated_no">
                <Input />
              </Form.Item>
            </Col>
            <Col span={3}>
              <Form.Item label="负责人" name="legal_person">
                <Input />
              </Form.Item>
            </Col>
            <Col span={3}>
              <Form.Item label="电话" name="phone">
                <Input />
              </Form.Item>
            </Col>
            <Col span={6}>
              <Form.Item label="联系人" name="contact">
                <Input />
              </Form.Item>
            </Col>
            <Col span={6}>
              <Form.Item label="统一社会信用代码" name="credit_code">
                <Input />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item label="地址" name="address">
                <Input />
              </Form.Item>
            </Col>
            <Col span={6}>
              <Form.Item label="食品经营/生产许可证号" name="license_no">
                <Input />
              </Form.Item>
            </Col>
            <Col span={6}>
              <Form.Item label="许可证有效期至" name="license_expire">
                <DatePicker style={{ width: '100%' }} format="YYYY-MM-DD" />
              </Form.Item>
            </Col>
            <Col span={4}>
              <Form.Item label="供货合同编号" name="contract_no">
                <Input />
              </Form.Item>
            </Col>
            <Col span={4}>
              <Form.Item label="合同开始日期" name="contract_start">
                <DatePicker style={{ width: '100%' }} format="YYYY-MM-DD" />
              </Form.Item>
            </Col>
            <Col span={4}>
              <Form.Item label="合同结束日期" name="contract_end">
                <DatePicker style={{ width: '100%' }} format="YYYY-MM-DD" />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item label="经营范围" name="business_scope">
                <Input.TextArea rows={2} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item label="备注" name="remark">
                <Input.TextArea rows={2} />
              </Form.Item>
            </Col>
          </Row>
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
