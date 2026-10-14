import { useState } from 'react'
import { App, Button, Card, Space, Table } from 'antd'
import type { TableProps } from 'antd'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  backupDownloadUrl,
  deleteBackup,
  executeBackup,
  listBackups,
  type BackupRecord,
} from '../../api/admin/system'
import { scrollX } from '../../utils/tableScroll'

/**
 * 旧页面 /backup：执行备份按钮 + 记录表格（ID/备份时间/文件名/大小(字节)/操作：下载、删除）。
 * 记录列表老页面为服务端渲染，SPA 预留 GET /api/backup/list（后端补接口后自动生效）。
 */
export default function BackupPage() {
  const { message, modal } = App.useApp()
  const queryClient = useQueryClient()
  const [backingUp, setBackingUp] = useState(false)

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['backups'] })

  const { data: rows = [], isLoading } = useQuery({
    queryKey: ['backups'],
    queryFn: listBackups,
  })

  const handleBackup = async () => {
    setBackingUp(true)
    try {
      const res = await executeBackup()
      message.info(res.message)
      if (res.ok) refresh()
    } finally {
      setBackingUp(false)
    }
  }

  const handleDelete = (row: BackupRecord) => {
    modal.confirm({
      title: '确定删除此备份？',
      okText: '确定',
      cancelText: '取消',
      onOk: async () => {
        const res = await deleteBackup(row.id)
        if (res.ok) {
          refresh()
        } else {
          message.error('删除失败')
        }
      },
    })
  }

  const columns: TableProps<BackupRecord>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    { title: '备份时间', dataIndex: 'backup_time', width: 180 },
    { title: '文件名', dataIndex: 'file_name' },
    { title: '大小(字节)', dataIndex: 'size', width: 120, align: 'right' },
    {
      title: '操作',
      key: 'action',
      width: 150,
      render: (_v, row) => (
        <Space size={4} wrap>
          <Button
            size="small"
            type="primary"
            ghost
            onClick={() => {
              window.location.href = backupDownloadUrl(row.id)
            }}
          >
            下载
          </Button>
          <Button size="small" danger onClick={() => handleDelete(row)}>
            删除
          </Button>
        </Space>
      ),
    },
  ]

  return (
    <Card size="small" title="数据备份">
      <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
        <Button
          type="primary"
          style={{ background: '#52c41a', borderColor: '#52c41a' }}
          loading={backingUp}
          onClick={handleBackup}
        >
          执行备份
        </Button>

        <Table<BackupRecord>
          rowKey="id"
          size="small"
          loading={isLoading}
          dataSource={rows}
          columns={columns}
          scroll={scrollX('max-content')}
          pagination={false}
        />
      </Space>
    </Card>
  )
}
