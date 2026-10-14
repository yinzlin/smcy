import { Button, Dropdown, Layout, Menu, Space } from 'antd'
import type { MenuProps } from 'antd'
import { LogoutOutlined, SwapOutlined, UserOutlined } from '@ant-design/icons'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useMemo } from 'react'
import { useAuth } from '../stores/auth'

const { Header, Sider, Content } = Layout

/** 老系统菜单树节点：label/icon/role 与 sidebar_html() 1:1 对应 */
interface TreeItem {
  key: string
  label: string
  icon: string
  /** 老菜单 data-role 标记 */
  role?: string
  children?: TreeItem[]
}

/** 老系统 filterMenuByRole 的角色可见集合 */
const ROLE_PERMS: Record<string, string[]> = {
  super_admin: ['admin', 'supplier', 'purchaser', 'query', 'super_admin'],
  admin: ['admin', 'supplier', 'purchaser', 'query'],
  supplier: ['supplier', 'query'],
  purchaser: ['purchaser', 'query'],
}

const MENU_TREE: TreeItem[] = [
  { key: '/', label: '首页', icon: '🏠' },
  {
    key: 'g-base',
    label: '基础数据',
    icon: '📁',
    role: 'admin',
    children: [
      {
        key: 'g-supplier',
        label: '供应商管理',
        icon: '🏭',
        role: 'supplier',
        children: [{ key: '/app/supplier', label: '全部供应商', icon: '📋' }],
      },
      {
        key: 'g-purchaser',
        label: '采购方管理',
        icon: '🏢',
        role: 'purchaser',
        children: [{ key: '/app/purchaser', label: '全部采购方', icon: '📋' }],
      },
      {
        key: 'g-product',
        label: '商品管理',
        icon: '📦',
        role: 'admin',
        children: [{ key: '/app/product', label: '全部商品', icon: '📋' }],
      },
      { key: '/app/warehouse', label: '仓库管理', icon: '🏠', role: 'admin' },
    ],
  },
  {
    key: 'g-order',
    label: '订单管理',
    icon: '📁',
    role: 'admin',
    children: [
      { key: '/app/purchase', label: '采购订单', icon: '🛒', role: 'supplier' },
      { key: '/app/sales', label: '销售订单', icon: '💰', role: 'purchaser' },
      { key: '/app/supplement', label: '耗材分摊管理', icon: '🔄', role: 'admin' },
      { key: '/app/today-price', label: '今日进价采集', icon: '💰', role: 'super_admin' },
      { key: '/app/price-schedule', label: '价格策略', icon: '💵' },
    ],
  },
  {
    key: 'g-query',
    label: '数据查询',
    icon: '🔍',
    role: 'query',
    children: [
      {
        key: 'g-query-purchase',
        label: '采购查询',
        icon: '📦',
        role: 'supplier',
        children: [
          { key: '/app/query/purchase-order', label: '采购订单查询', icon: '📋' },
          { key: '/app/query/purchase-document', label: '采购单据列表', icon: '🧾' },
          { key: '/app/query/purchase-price', label: '采购台账查询', icon: '💰' },
          { key: '/app/query/purchase-summary', label: '采购汇总统计', icon: '📊' },
          { key: '/app/query/supplier-balance', label: '供应商往来对账', icon: '📈' },
        ],
      },
      {
        key: 'g-query-sales',
        label: '销售查询',
        icon: '💵',
        role: 'purchaser',
        children: [
          { key: '/app/query/sales-order', label: '销售订单查询', icon: '📋' },
          { key: '/app/query/sales-summary', label: '销售汇总报表', icon: '📊' },
          { key: '/app/query/sales-price', label: '销售价格查询', icon: '💰' },
          { key: '/app/query/purchaser-balance', label: '采购方应收对账', icon: '📈' },
          { key: '/app/query/product-rank', label: '畅销滞销商品', icon: '🏆' },
          { key: '/app/query/reimburse-summary', label: '报销口径汇总', icon: '🧾' },
          { key: '/app/query/allocation-source', label: '分摊来源统计', icon: '🔀' },
          { key: '/app/query/order-adjust', label: '订单调整与同屏比对', icon: '✏️' },
        ],
      },
      {
        key: 'g-query-stock',
        label: '库存查询',
        icon: '📦',
        role: 'admin',
        children: [
          { key: '/app/query/stock-balance', label: '实时库存余额', icon: '📊' },
          { key: '/app/query/stock-flow', label: '出入库记录', icon: '📋' },
          { key: '/app/query/stock-summary', label: '真实出入库统计', icon: '📈' },
          { key: '/app/query/stock-summary-reimburse', label: '报销出入库统计', icon: '🧾' },
          { key: '/app/query/stock-warning', label: '库存上下限预警', icon: '⚠️' },
          { key: '/app/query/slow-stock', label: '呆滞库存查询', icon: '⏳' },
        ],
      },
      {
        key: 'g-query-finance',
        label: '财务查询',
        icon: '💰',
        role: 'admin',
        children: [
          { key: '/app/query/income-expense', label: '收支流水查询', icon: '📈' },
          { key: '/app/query/profit-detail', label: '毛利明细查询', icon: '📊' },
          { key: '/app/query/finance-settlement', label: '财务结算查询', icon: '💰' },
        ],
      },
      {
        key: 'g-query-report',
        label: '统计报表',
        icon: '📈',
        role: 'admin',
        children: [
          { key: '/app/query/overview', label: '进销存汇总报表', icon: '📋' },
          { key: '/app/query/category-stats', label: '品类进销存统计', icon: '📊' },
          { key: '/app/query/document-summary', label: '单据汇总查询', icon: '📄' },
        ],
      },
    ],
  },
  {
    key: 'g-system',
    label: '系统设置',
    icon: '⚙️',
    role: 'super_admin',
    children: [
      { key: '/app/user', label: '用户管理', icon: '👥' },
      { key: '/app/system', label: '系统参数', icon: '📋' },
      { key: '/app/backup', label: '数据备份', icon: '💾' },
      { key: '/app/restore', label: '数据恢复', icon: '🔄' },
      { key: '/app/operation-log', label: '操作日志', icon: '📝' },
    ],
  },
]

/** 与老系统一致：不可见节点隐藏，空分组折叠隐藏 */
function filterByRole(items: TreeItem[], role: string): TreeItem[] {
  const allowed = ROLE_PERMS[role] ?? []
  const out: TreeItem[] = []
  for (const it of items) {
    const kids = it.children ? filterByRole(it.children, role) : undefined
    if (it.role && !allowed.includes(it.role)) continue
    if (it.children && kids && kids.length === 0) continue
    out.push(kids ? { ...it, children: kids } : it)
  }
  return out
}

function toMenuItems(items: TreeItem[]): NonNullable<MenuProps['items']> {
  return items.map((it) =>
    it.children && it.children.length > 0
      ? {
          key: it.key,
          icon: <span>{it.icon}</span>,
          label: it.label,
          children: toMenuItems(it.children),
        }
      : { key: it.key, icon: <span>{it.icon}</span>, label: it.label },
  )
}

function collectOpenKeys(items: TreeItem[]): string[] {
  const keys: string[] = []
  for (const it of items) {
    if (it.children && it.children.length > 0) {
      keys.push(it.key)
      keys.push(...collectOpenKeys(it.children))
    }
  }
  return keys
}

export default function MainLayout() {
  const { user, logout } = useAuth()
  const navigate = useNavigate()
  const location = useLocation()

  const visibleTree = useMemo(() => filterByRole(MENU_TREE, user?.role ?? ''), [user?.role])
  const menuItems = useMemo(() => toMenuItems(visibleTree), [visibleTree])
  const defaultOpenKeys = useMemo(() => collectOpenKeys(visibleTree), [visibleTree])

  // '/app' 与 '/app/' 均视为首页
  const selectedKey = location.pathname.replace(/\/+$/, '') || '/'
  const selectedKeys = selectedKey === '/app' ? ['/'] : [selectedKey]

  const onLogout = async () => {
    await logout()
    // 全量刷新回登录页，确保老页面与新页面 session 状态一致
    window.location.href = '/app/login'
  }

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider theme="light" width={230} style={{ overflowY: 'auto', height: '100vh', position: 'sticky', top: 0 }}>
        <div style={{ height: 48, lineHeight: '48px', textAlign: 'center', fontWeight: 600 }}>
          食材收发系统
        </div>
        <Menu
          mode="inline"
          selectedKeys={selectedKeys}
          defaultOpenKeys={defaultOpenKeys}
          items={menuItems}
          onClick={({ key }) => {
            if (key.startsWith('/')) navigate(key)
          }}
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
