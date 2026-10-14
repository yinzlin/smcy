import { Button, Card, Descriptions, Space, Typography } from 'antd'
import { useAuth } from '../stores/auth'

/** 首页占位：验证登录态与接口连通，核心页面按计划逐个迁移 */
export default function HomePage() {
  const { user } = useAuth()

  return (
    <Card title="React 前端骨架已就绪">
      <Descriptions column={1} bordered size="middle" style={{ maxWidth: 520 }}>
        <Descriptions.Item label="当前用户">{user?.nickname}</Descriptions.Item>
        <Descriptions.Item label="账号">{user?.username}</Descriptions.Item>
        <Descriptions.Item label="角色">{user?.role}</Descriptions.Item>
      </Descriptions>
      <Typography.Paragraph type="secondary" style={{ marginTop: 24 }}>
        核心页面（商品 / 采购订单 / 销售订单 / 今日进价采集 / 价格策略）迁移中，其余功能请暂时使用老系统。
      </Typography.Paragraph>
      <Space>
        <Button type="primary" href="/">
          进入老系统
        </Button>
      </Space>
    </Card>
  )
}
