import { useEffect, useState } from 'react'
import { Button, Card, Form, Input, message } from 'antd'
import { LockOutlined, UserOutlined } from '@ant-design/icons'
import { useSearchParams } from 'react-router-dom'
import { useAuth } from '../stores/auth'

/** 登录成功后的落地页：默认回老系统首页（React 页面尚未迁完），可用 ?redirect= 指定 */
export default function LoginPage() {
  const { user, checked, fetchMe, login } = useAuth()
  const [searchParams] = useSearchParams()
  const [submitting, setSubmitting] = useState(false)
  const [messageApi, contextHolder] = message.useMessage()

  const redirect = searchParams.get('redirect') || '/'

  useEffect(() => {
    if (!checked) {
      void fetchMe()
    }
  }, [checked, fetchMe])

  useEffect(() => {
    if (checked && user) {
      window.location.href = redirect
    }
  }, [checked, user, redirect])

  const onFinish = async (values: { username: string; password: string }) => {
    setSubmitting(true)
    const result = await login(values.username.trim(), values.password)
    setSubmitting(false)
    if (result.ok) {
      window.location.href = redirect
    } else {
      void messageApi.error(result.message)
    }
  }

  return (
    <div
      style={{
        minHeight: '100vh',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        background: '#f0f2f5',
        padding: 16,
      }}
    >
      {contextHolder}
      <Card title="食材收发系统" style={{ width: '100%', maxWidth: 360 }} styles={{ header: { textAlign: 'center' } }}>
        <Form layout="vertical" onFinish={onFinish} autoComplete="off">
          <Form.Item name="username" rules={[{ required: true, message: '请输入用户名' }]}>
            <Input prefix={<UserOutlined />} placeholder="用户名" size="large" />
          </Form.Item>
          <Form.Item name="password" rules={[{ required: true, message: '请输入密码' }]}>
            <Input.Password prefix={<LockOutlined />} placeholder="密码" size="large" />
          </Form.Item>
          <Form.Item style={{ marginBottom: 0 }}>
            <Button type="primary" htmlType="submit" size="large" block loading={submitting}>
              登 录
            </Button>
          </Form.Item>
        </Form>
      </Card>
    </div>
  )
}
