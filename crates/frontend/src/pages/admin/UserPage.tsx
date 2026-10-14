import { useMemo, useState } from 'react'
import { App, Button, Card, Input, Modal, Select, Space, Table, Tag } from 'antd'
import type { TableProps } from 'antd'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  createUser,
  deleteUser,
  getUserDetail,
  listAdminUsers,
  updateUser,
  updateUserStatus,
  type AdminUser,
  type UserSavePayload,
} from '../../api/admin/system'
import { listSuppliers } from '../../api/purchase'
import { listPurchasers } from '../../api/sales'

/** 角色文案（照抄老页面 role_label 映射） */
const ROLE_LABELS: Record<string, string> = {
  super_admin: '超级管理员',
  admin: '管理员',
  supplier: '供应商',
  purchaser: '采购方',
}
const roleLabel = (role: string) => ROLE_LABELS[role] ?? '普通用户'

/** 弹窗角色选项（照抄老页面 select，无 super_admin 项） */
const ROLE_OPTIONS = [
  { value: 'admin', label: '管理员' },
  { value: 'supplier', label: '供应商' },
  { value: 'purchaser', label: '采购方' },
  { value: 'user', label: '普通用户' },
]

interface EditState {
  open: boolean
  id: number | null
  username: string
  nickname: string
  password: string
  role: string
  supplierId: number
  purchaserId: number
}

const EMPTY_EDIT: EditState = {
  open: false,
  id: null,
  username: '',
  nickname: '',
  password: '',
  role: 'admin',
  supplierId: 0,
  purchaserId: 0,
}

/**
 * 旧页面 /user：添加用户按钮 + 表格（ID/用户名/昵称/角色/数据权限/状态/最后登录/创建时间/操作）
 * + 添加/编辑弹窗（角色与数据权限绑定 supplier_id/purchaser_id）+ 启用/禁用 + 删除。
 */
export default function UserPage() {
  const { message, modal } = App.useApp()
  const queryClient = useQueryClient()
  const [edit, setEdit] = useState<EditState>(EMPTY_EDIT)
  const [saving, setSaving] = useState(false)

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['adminUsers'] })

  const { data: users = [], isLoading } = useQuery({
    queryKey: ['adminUsers'],
    queryFn: listAdminUsers,
  })

  // 绑定下拉选项：供应商 / 采购方（含"未绑定"，照抄老页面）
  const { data: suppliers = [] } = useQuery({ queryKey: ['suppliers', 'userPage'], queryFn: listSuppliers })
  const { data: purchasers = [] } = useQuery({ queryKey: ['purchasers', 'userPage'], queryFn: listPurchasers })

  const supplierNameMap = useMemo(
    () => new Map(suppliers.map((s) => [s.id, s.name])),
    [suppliers],
  )
  const purchaserNameMap = useMemo(
    () => new Map(purchasers.map((p) => [p.id, p.name])),
    [purchasers],
  )

  const handleEdit = async (id: number) => {
    const data = await getUserDetail(id)
    if (data.success && data.user) {
      setEdit({
        open: true,
        id: data.user.id,
        username: data.user.username,
        nickname: data.user.nickname || '',
        password: '',
        role: data.user.role,
        supplierId: data.user.supplier_id || 0,
        purchaserId: data.user.purchaser_id || 0,
      })
    }
  }

  const handleSave = async () => {
    if (!edit.username.trim()) {
      message.error('用户名不能为空')
      return
    }
    const payload: UserSavePayload = {
      username: edit.username,
      nickname: edit.nickname,
      password: edit.password,
      role: edit.role,
      supplier_id: edit.supplierId,
      purchaser_id: edit.purchaserId,
    }
    setSaving(true)
    try {
      const result = edit.id ? await updateUser(edit.id, payload) : await createUser(payload)
      if (result.success) {
        setEdit(EMPTY_EDIT)
        refresh()
      } else {
        message.error(result.message || '保存失败')
      }
    } finally {
      setSaving(false)
    }
  }

  const handleToggleStatus = async (row: AdminUser) => {
    const res = await updateUserStatus(row.id, row.status === 1 ? 0 : 1)
    if (res.success) {
      refresh()
    } else {
      message.error('操作失败')
    }
  }

  const handleDelete = (row: AdminUser) => {
    modal.confirm({
      title: '确定删除该用户？',
      okText: '确定',
      cancelText: '取消',
      onOk: async () => {
        const res = await deleteUser(row.id)
        if (res.success) {
          refresh()
        } else {
          message.error('删除失败')
        }
      },
    })
  }

  const columns: TableProps<AdminUser>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    { title: '用户名', dataIndex: 'username', width: 130 },
    { title: '昵称', dataIndex: 'nickname', width: 130, render: (v: string | null) => v || '' },
    { title: '角色', dataIndex: 'role', width: 110, render: (v: string) => roleLabel(v) },
    {
      title: '数据权限',
      key: 'data_scope',
      width: 180,
      render: (_v, row) => {
        // 数据权限展示：供应商角色显示绑定的供应商，采购方角色显示绑定的采购单位（照抄老页面）
        if (row.role === 'supplier' && row.supplier_id > 0) {
          return `供应商：${supplierNameMap.get(row.supplier_id) ?? row.supplier_id}`
        }
        if (row.role === 'purchaser' && row.purchaser_id > 0) {
          return `采购方：${purchaserNameMap.get(row.purchaser_id) ?? row.purchaser_id}`
        }
        return '-'
      },
    },
    {
      title: '状态',
      dataIndex: 'status',
      width: 80,
      render: (v: number) =>
        v === 1 ? <Tag color="success">启用</Tag> : <Tag color="error">禁用</Tag>,
    },
    { title: '最后登录', dataIndex: 'last_login_time', width: 160, render: (v: string | undefined) => v || '-' },
    { title: '创建时间', dataIndex: 'create_at', width: 160, render: (v: string | undefined) => v || '-' },
    {
      title: '操作',
      key: 'action',
      width: 260,
      render: (_v, row) => (
        <Space size={4} wrap>
          <Button size="small" type="primary" onClick={() => handleEdit(row.id)}>
            编辑
          </Button>
          <Button
            size="small"
            type="primary"
            style={{ background: '#faad14', borderColor: '#faad14' }}
            onClick={() => handleToggleStatus(row)}
          >
            {row.status === 1 ? '禁用' : '启用'}
          </Button>
          {row.username !== 'super_admin' && (
            <Button size="small" danger onClick={() => handleDelete(row)}>
              删除
            </Button>
          )}
        </Space>
      ),
    },
  ]

  return (
    <Card size="small" title="用户管理">
      <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
        <Button
          type="primary"
          style={{ background: '#52c41a', borderColor: '#52c41a' }}
          onClick={() => setEdit({ ...EMPTY_EDIT, open: true })}
        >
          添加用户
        </Button>

        <Table<AdminUser>
          rowKey="id"
          size="small"
          loading={isLoading}
          dataSource={users}
          columns={columns}
          scroll={{ x: 1100 }}
          pagination={false}
        />
      </Space>

      <Modal
        open={edit.open}
        title={edit.id ? '编辑用户' : '添加用户'}
        okText="保存"
        cancelText="取消"
        confirmLoading={saving}
        onOk={handleSave}
        onCancel={() => setEdit(EMPTY_EDIT)}
      >
        <Space direction="vertical" size="middle" style={{ display: 'flex', marginTop: 12 }}>
          <label style={{ display: 'block' }}>
            用户名
            <Input value={edit.username} onChange={(e) => setEdit((s) => ({ ...s, username: e.target.value }))} />
          </label>
          <label style={{ display: 'block' }}>
            昵称
            <Input value={edit.nickname} onChange={(e) => setEdit((s) => ({ ...s, nickname: e.target.value }))} />
          </label>
          <label style={{ display: 'block' }}>
            密码
            <Input.Password
              value={edit.password}
              onChange={(e) => setEdit((s) => ({ ...s, password: e.target.value }))}
            />
            <small style={{ color: '#999' }}>编辑时不填则保持原密码</small>
          </label>
          <label style={{ display: 'block' }}>
            角色
            <Select
              style={{ width: '100%' }}
              value={edit.role}
              options={ROLE_OPTIONS}
              onChange={(v) => setEdit((s) => ({ ...s, role: v }))}
            />
          </label>
          {/* 根据角色显示/隐藏数据权限绑定行（照抄老页面 toggleDataScope） */}
          {edit.role === 'supplier' && (
            <label style={{ display: 'block' }}>
              绑定供应商（供应商角色数据权限）
              <Select
                style={{ width: '100%' }}
                value={edit.supplierId}
                onChange={(v) => setEdit((s) => ({ ...s, supplierId: v }))}
                options={[
                  { value: 0, label: '未绑定' },
                  ...suppliers.map((s) => ({ value: s.id, label: s.name })),
                ]}
              />
            </label>
          )}
          {edit.role === 'purchaser' && (
            <label style={{ display: 'block' }}>
              绑定采购单位（采购方角色数据权限）
              <Select
                style={{ width: '100%' }}
                value={edit.purchaserId}
                onChange={(v) => setEdit((s) => ({ ...s, purchaserId: v }))}
                options={[
                  { value: 0, label: '未绑定' },
                  ...purchasers.map((p) => ({ value: p.id, label: p.name })),
                ]}
              />
            </label>
          )}
        </Space>
      </Modal>
    </Card>
  )
}
