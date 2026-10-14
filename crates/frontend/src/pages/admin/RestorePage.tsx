import { useState } from 'react'
import { Alert, App, Button, Card, Select, Space, Typography, Upload } from 'antd'
import type { UploadFile } from 'antd'
import { useQuery } from '@tanstack/react-query'
import { listBackups, restoreBackup, restoreFromFile } from '../../api/admin/system'

/**
 * 旧页面 /restore：危险操作警告 + 选择备份下拉 + 确认恢复 + 从文件恢复（上传 .db），
 * 警告与确认文案照抄老页面；备份列表老页面为服务端渲染（同备份页预留 /api/backup/list）。
 */
export default function RestorePage() {
  const { message, modal } = App.useApp()
  const [backupId, setBackupId] = useState<number | null>(null)
  const [file, setFile] = useState<File | null>(null)
  const [restoring, setRestoring] = useState(false)

  const { data: backups = [], isLoading } = useQuery({
    queryKey: ['backups'],
    queryFn: listBackups,
  })

  const confirmRestore = (confirmTitle: string, action: () => Promise<{ ok: boolean; message: string }>) => {
    modal.confirm({
      title: confirmTitle,
      okText: '确定',
      cancelText: '取消',
      onOk: async () => {
        setRestoring(true)
        try {
          const res = await action()
          message.info(res.message)
          if (res.ok) {
            // 恢复成功后老页面跳转首页
            window.location.href = '/'
          }
        } finally {
          setRestoring(false)
        }
      },
    })
  }

  const handleRestore = () => {
    if (!backupId) {
      message.warning('请选择备份文件')
      return
    }
    confirmRestore('确定要恢复此备份吗？这将覆盖当前所有数据！', () => restoreBackup(backupId))
  }

  const handleRestoreFromFile = () => {
    if (!file) {
      message.warning('请选择备份文件')
      return
    }
    confirmRestore('确定要从文件恢复吗？这将覆盖当前所有数据！', () => restoreFromFile(file))
  }

  const fileList: UploadFile[] = file
    ? [{ uid: '-restore-file', name: file.name, status: 'done' }]
    : []

  return (
    <Card size="small" title="数据恢复">
      <Alert
        type="warning"
        showIcon
        style={{ marginBottom: 24 }}
        message={
          <span>
            <strong>警告！</strong>数据恢复将覆盖当前所有数据，请确保已备份最新数据。
          </span>
        }
      />

      <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
        <Space wrap>
          <Typography.Text>选择备份：</Typography.Text>
          <Select
            style={{ width: 400 }}
            loading={isLoading}
            value={backupId}
            onChange={(v) => setBackupId(v)}
            options={backups.map((b) => ({ value: b.id, label: `${b.backup_time} - ${b.file_name}` }))}
          />
        </Space>
        <Button danger type="primary" loading={restoring} onClick={handleRestore}>
          确认恢复
        </Button>

        <Typography.Title level={4} style={{ marginTop: 16, marginBottom: 0 }}>
          从文件恢复
        </Typography.Title>
        <Upload
          accept=".db"
          maxCount={1}
          fileList={fileList}
          beforeUpload={(f) => {
            setFile(f)
            return false
          }}
          onRemove={() => setFile(null)}
        >
          <Button>选择文件</Button>
        </Upload>
        <div>
          <Button type="primary" style={{ background: '#faad14', borderColor: '#faad14' }} loading={restoring} onClick={handleRestoreFromFile}>
            从文件恢复
          </Button>
        </div>
      </Space>
    </Card>
  )
}
