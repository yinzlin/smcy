import { useEffect, useState } from 'react'
import { App, Button, Card, Form, Input } from 'antd'
import { useQuery } from '@tanstack/react-query'
import { fetchSystemConfig, saveSystemConfig, type SystemConfigMap } from '../../api/admin/system'

/** 老页面 default_configs：key / label / 默认值（照抄） */
const CONFIG_FIELDS: { key: string; label: string; initial: string }[] = [
  { key: 'system_name', label: '系统名称', initial: '进销存管理系统' },
  { key: 'company_name', label: '公司名称', initial: '' },
  { key: 'company_address', label: '公司地址', initial: '' },
  { key: 'company_phone', label: '联系电话', initial: '' },
  { key: 'decimal_places', label: '金额小数位数', initial: '2' },
  { key: 'auto_save_interval', label: '自动保存间隔(秒)', initial: '30' },
]

const initialValues: SystemConfigMap = Object.fromEntries(
  CONFIG_FIELDS.map((f) => [f.key, f.initial]),
)

/**
 * 旧页面 /system：卡片内表单（label 3/12 + input 6/12），提交 POST /api/system/config，
 * 成功提示"设置保存成功"，失败提示"保存失败"（照抄老 JS alert 文案）。
 */
export default function SystemConfigPage() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [saving, setSaving] = useState(false)

  // 老页面由服务端渲染回填现有配置；SPA 等效为 GET 拉取（后端暂无 GET 路由时保持默认值）
  const { data: config } = useQuery({
    queryKey: ['systemConfig'],
    queryFn: fetchSystemConfig,
    retry: false,
  })

  useEffect(() => {
    if (config) form.setFieldsValue(config)
  }, [config, form])

  const handleSave = async (values: SystemConfigMap) => {
    setSaving(true)
    try {
      const res = await saveSystemConfig(values)
      if (res.ok) {
        message.success('设置保存成功')
      } else {
        message.error('保存失败')
      }
    } finally {
      setSaving(false)
    }
  }

  return (
    <Card size="small" title="系统参数设置" style={{ maxWidth: 800 }}>
      <Form form={form} initialValues={initialValues} labelCol={{ span: 4 }} wrapperCol={{ span: 12 }} onFinish={handleSave}>
        {CONFIG_FIELDS.map((f) => (
          <Form.Item key={f.key} name={f.key} label={f.label}>
            <Input />
          </Form.Item>
        ))}
        <Form.Item wrapperCol={{ offset: 4, span: 12 }}>
          <Button type="primary" htmlType="submit" loading={saving}>
            保存设置
          </Button>
        </Form.Item>
      </Form>
    </Card>
  )
}
