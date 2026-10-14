import { useMemo, useState } from 'react'
import { App, Button, Card, Form, Input, Modal, Select, Space, Table, Tag, Typography } from 'antd'
import type { TableProps } from 'antd'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  approveWarehouse,
  createWarehouse,
  deleteWarehouse,
  listWarehouses,
  unapproveWarehouse,
  updateWarehouse,
  type Warehouse,
} from '../../api/admin/basic'
import { useAuth } from '../../stores/auth'

interface WarehouseFormValues {
  name: string
  code?: string
  contact?: string
  phone?: string
  address?: string
  status: number
}

function truncateText(text: string | null, maxLen: number): string {
  if (!text) return ''
  return text.length > maxLen ? text.slice(0, maxLen) + '...' : text
}

export default function WarehousePage() {
  const { message, modal } = App.useApp()
  const { user } = useAuth()
  const isSuperAdmin = user?.role === 'super_admin'
  const queryClient = useQueryClient()

  const [keyword, setKeyword] = useState('')
  const [modalOpen, setModalOpen] = useState(false)
  const [editing, setEditing] = useState<Warehouse | null>(null)

  const [form] = Form.useForm<WarehouseFormValues>()

  const { data: warehouses = [], isLoading } = useQuery({
    queryKey: ['warehouses'],
    queryFn: listWarehouses,
  })

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['warehouses'] })

  // 老页面为前端本地过滤（名称/编号包含关键字，忽略大小写）
  const filtered = useMemo(() => {
    const kw = keyword.toLowerCase().trim()
    if (!kw) return warehouses
    return warehouses.filter(
      (w) => w.name.toLowerCase().includes(kw) || (w.code ? w.code.toLowerCase().includes(kw) : false),
    )
  }, [warehouses, keyword])

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

  const openCreate = () => {
    setEditing(null)
    form.resetFields()
    form.setFieldsValue({ status: 1 })
    setModalOpen(true)
  }

  const openEdit = (row: Warehouse) => {
    if (row.audit_status === 'confirmed') {
      message.warning('该仓库已审核，如需修改请先反审核')
      return
    }
    setEditing(row)
    form.setFieldsValue({
      name: row.name,
      code: row.code ?? '',
      contact: row.contact ?? '',
      phone: row.phone ?? '',
      address: row.address ?? '',
      status: row.status,
    })
    setModalOpen(true)
  }

  const submitWarehouse = async () => {
    const values = await form.validateFields()
    const payload = {
      name: values.name,
      code: values.code || null,
      contact: values.contact || null,
      phone: values.phone || null,
      address: values.address || null,
      status: values.status,
      sort_order: 0,
    }
    const res = editing ? await updateWarehouse({ ...payload, id: editing.id }) : await createWarehouse(payload)
    if (res.ok) {
      message.success(res.message || '保存成功')
      setModalOpen(false)
      refresh()
    } else {
      message.error(res.message || (editing ? '保存失败' : '创建失败'))
    }
  }

  const handleDelete = (row: Warehouse) => {
    if (row.audit_status === 'confirmed') {
      message.warning('该仓库已审核，如需删除请先反审核')
      return
    }
    confirmThen('确定要删除该仓库吗？删除后无法恢复！', () =>
      runAction(() => deleteWarehouse(row.id), '删除成功', '删除失败'),
    )
  }

  const columns: TableProps<Warehouse>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 60 },
    { title: '编号', dataIndex: 'code', width: 100, render: (v: string | null) => v || '' },
    { title: '名称', dataIndex: 'name', width: 160 },
    { title: '联系人', dataIndex: 'contact', width: 90, render: (v: string | null) => v || '' },
    { title: '电话', dataIndex: 'phone', width: 110, render: (v: string | null) => v || '' },
    {
      title: '地址',
      dataIndex: 'address',
      width: 200,
      render: (v: string | null) => <span title={v || ''}>{truncateText(v, 20)}</span>,
    },
    {
      title: '状态',
      dataIndex: 'status',
      width: 80,
      render: (v: number) => (v === 1 ? <Tag color="success">启用</Tag> : <Tag color="default">停用</Tag>),
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
                  confirmThen('确定反审核该仓库吗？反审核后需重新审核。', () =>
                    runAction(() => unapproveWarehouse(row.id), '反审核成功', '反审核失败'),
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
                  confirmThen('确定审核通过该仓库吗？', () =>
                    runAction(() => approveWarehouse(row.id), '审核成功', '审核失败'),
                  )
                }
              >
                审核
              </Button>
            ))}
          <Button size="small" onClick={() => openEdit(row)}>
            编辑
          </Button>
          {/* 老页面：内置仓库 id=1 不提供删除按钮 */}
          {row.id !== 1 && (
            <Button size="small" danger onClick={() => handleDelete(row)}>
              删除
            </Button>
          )}
        </Space>
      ),
    },
  ]

  return (
    <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
      <Card>
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginBottom: 16,
          }}
        >
          <Typography.Title level={4} style={{ margin: 0 }}>
            仓库管理
          </Typography.Title>
          <Button type="primary" onClick={openCreate}>
            新建仓库
          </Button>
        </div>
        <div style={{ marginBottom: 12 }}>
          <Input
            placeholder="搜索仓库名称或编号..."
            style={{ maxWidth: 320 }}
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            allowClear
          />
        </div>
        <Table<Warehouse>
          rowKey="id"
          size="small"
          bordered
          loading={isLoading}
          dataSource={filtered}
          columns={columns}
          pagination={false}
          scroll={{ x: 900 }}
          locale={{ emptyText: '暂无仓库数据' }}
        />
      </Card>

      <Modal
        open={modalOpen}
        title={editing ? '编辑仓库' : '新建仓库'}
        okText="保存"
        cancelText="取消"
        onOk={submitWarehouse}
        onCancel={() => setModalOpen(false)}
      >
        <Form form={form} layout="vertical" initialValues={{ status: 1 }}>
          <Form.Item
            label="仓库名称"
            name="name"
            rules={[{ required: true, message: '请输入仓库名称' }]}
          >
            <Input />
          </Form.Item>
          <Form.Item label="仓库编号" name="code">
            <Input placeholder="如 WH002" />
          </Form.Item>
          <Form.Item label="联系人" name="contact">
            <Input />
          </Form.Item>
          <Form.Item label="电话" name="phone">
            <Input />
          </Form.Item>
          <Form.Item label="地址" name="address">
            <Input.TextArea rows={2} />
          </Form.Item>
          <Form.Item label="状态" name="status">
            <Select
              options={[
                { value: 1, label: '启用' },
                { value: 0, label: '停用' },
              ]}
            />
          </Form.Item>
        </Form>
      </Modal>
    </Space>
  )
}
