import { Button, Dropdown, Layout, Menu, Space } from 'antd'
import { HomeOutlined, LogoutOutlined, SwapOutlined, UserOutlined } from '@ant-design/icons'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useAuth } from '../stores/auth'

const { Header, Sider, Content } = Layout

export default function MainLayout() {
  const { user, logout } = useAuth()
  const navigate = useNavigate()
  const location = useLocation()

  const onLogout = async () => {
    await logout()
    // 全量刷新回登录页，确保老页面与新页面 session 状态一致
    window.location.href = '/app/login'
  }

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider theme="light" width={200}>
        <div style={{ height: 48, lineHeight: '48px', textAlign: 'center', fontWeight: 600 }}>
          食材收发系统
        </div>
        <Menu
          mode="inline"
          selectedKeys={[location.pathname]}
          items={[{ key: '/', icon: <HomeOutlined />, label: '首页' }]}
          onClick={({ key }) => navigate(key)}
        />
      </Sider>
      <Layout>
        <Header
          style={{
            background: '#fff',
            padding: '0 24px',
            display: 'flex',
            justifyContent: 'flex-end',
            alignItems: 'center',
            borderBottom: '1px solid #f0f0f0',
          }}
        >
          <Space size="middle">
            <Button icon={<SwapOutlined />} href="/">
              老系统入口
            </Button>
            <Dropdown
              menu={{
                items: [{ key: 'logout', icon: <LogoutOutlined />, label: '退出登录' }],
                onClick: ({ key }) => {
                  if (key === 'logout') void onLogout()
                },
              }}
            >
              <Space style={{ cursor: 'pointer' }}>
                <UserOutlined />
                {user?.nickname || user?.username}
              </Space>
            </Dropdown>
          </Space>
        </Header>
        <Content style={{ margin: 24 }}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  )
}
