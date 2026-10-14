import { Button, Drawer, Dropdown, Grid, Layout, Menu, Space } from 'antd'
import type { MenuProps } from 'antd'
import { LogoutOutlined, MenuOutlined, UserOutlined } from '@ant-design/icons'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useAuth } from '../stores/auth'
import {
  createCategory,
  deleteCategory,
  listCategoryTree,
  renameCategory,
  type CategoryTreeNode,
} from '../api/admin/basic'

const { Header, Sider, Content } = Layout

/** 基础数据三大分类树对应的实体类型 */
type Entity = 'supplier' | 'purchaser' | 'product'

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

/** 基础数据下三个可右键管理的分类树文件夹（父项点击直达全部列表页） */
const ENTITY_FOLDERS: Record<string, { entity: Entity; route: string }> = {
  'g-supplier': { entity: 'supplier', route: '/supplier' },
  'g-purchaser': { entity: 'purchaser', route: '/purchaser' },
  'g-product': { entity: 'product', route: '/product' },
}

/** 菜单 key 一律使用 basename 相对路径（BrowserRouter basename="/app"） */
const MENU_TREE: TreeItem[] = [
  { key: '/', label: '首页', icon: '🏠' },
  {
    key: 'g-base',
    label: '基础数据',
    icon: '📁',
    role: 'admin',
    children: [
      // 三大管理项为新 UI 改良（用户确认）：父项点击直达全部列表页（route 见 ENTITY_FOLDERS），
      // 不保留「全部XX」子项，分类树为动态子节点，图标仅保留 icon 槽一处
      {
        key: 'g-supplier',
        label: '供应商管理',
        icon: '🏭',
        role: 'supplier',
      },
      {
        key: 'g-purchaser',
        label: '采购方管理',
        icon: '🏢',
        role: 'purchaser',
      },
      {
        key: 'g-product',
        label: '商品管理',
        icon: '📦',
        role: 'admin',
      },
      { key: '/warehouse', label: '仓库管理', icon: '🏠', role: 'admin' },
    ],
  },
  {
    key: 'g-order',
    label: '订单管理',
    icon: '📁',
    role: 'admin',
    children: [
      { key: '/purchase', label: '采购订单', icon: '🛒', role: 'supplier' },
      { key: '/sales', label: '销售订单', icon: '💰', role: 'purchaser' },
      { key: '/supplement', label: '耗材分摊管理', icon: '🔄', role: 'admin' },
      { key: '/today-price', label: '今日进价采集', icon: '💰', role: 'super_admin' },
      { key: '/price-schedule', label: '价格策略', icon: '💵' },
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
          { key: '/query/purchase-order', label: '采购订单查询', icon: '📋' },
          { key: '/query/purchase-document', label: '采购单据列表', icon: '🧾' },
          { key: '/query/purchase-price', label: '采购台账查询', icon: '💰' },
          { key: '/query/purchase-summary', label: '采购汇总统计', icon: '📊' },
          { key: '/query/supplier-balance', label: '供应商往来对账', icon: '📈' },
        ],
      },
      {
        key: 'g-query-sales',
        label: '销售查询',
        icon: '💵',
        role: 'purchaser',
        children: [
          { key: '/query/sales-order', label: '销售订单查询', icon: '📋' },
          { key: '/query/sales-summary', label: '销售汇总报表', icon: '📊' },
          { key: '/query/sales-price', label: '销售价格查询', icon: '💰' },
          { key: '/query/purchaser-balance', label: '采购方应收对账', icon: '📈' },
          { key: '/query/product-rank', label: '畅销滞销商品', icon: '🏆' },
          { key: '/query/reimburse-summary', label: '报销口径汇总', icon: '🧾' },
          { key: '/query/allocation-source', label: '分摊来源统计', icon: '🔀' },
          { key: '/query/order-adjust', label: '订单调整与同屏比对', icon: '✏️' },
        ],
      },
      {
        key: 'g-query-stock',
        label: '库存查询',
        icon: '📦',
        role: 'admin',
        children: [
          { key: '/query/stock-balance', label: '实时库存余额', icon: '📊' },
          { key: '/query/stock-flow', label: '出入库记录', icon: '📋' },
          { key: '/query/stock-summary', label: '真实出入库统计', icon: '📈' },
          { key: '/query/stock-summary-reimburse', label: '报销出入库统计', icon: '🧾' },
          { key: '/query/stock-warning', label: '库存上下限预警', icon: '⚠️' },
          { key: '/query/slow-stock', label: '呆滞库存查询', icon: '⏳' },
        ],
      },
      {
        key: 'g-query-finance',
        label: '财务查询',
        icon: '💰',
        role: 'admin',
        children: [
          { key: '/query/income-expense', label: '收支流水查询', icon: '📈' },
          { key: '/query/profit-detail', label: '毛利明细查询', icon: '📊' },
          { key: '/query/finance-settlement', label: '财务结算查询', icon: '💰' },
        ],
      },
      {
        key: 'g-query-report',
        label: '统计报表',
        icon: '📈',
        role: 'admin',
        children: [
          { key: '/query/overview', label: '进销存汇总报表', icon: '📋' },
          { key: '/query/category-stats', label: '品类进销存统计', icon: '📊' },
          { key: '/query/document-summary', label: '单据汇总查询', icon: '📄' },
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
      { key: '/user', label: '用户管理', icon: '👥' },
      { key: '/system', label: '系统参数', icon: '📋' },
      { key: '/backup', label: '数据备份', icon: '💾' },
      { key: '/restore', label: '数据恢复', icon: '🔄' },
      { key: '/operation-log', label: '操作日志', icon: '📝' },
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

const ROOT_MENU_ITEMS: MenuProps['items'] = [
  { key: 'addRoot', label: '➕ 新增顶级分类' },
  { type: 'divider' },
  { key: 'refresh', label: '🔄 刷新分类树' },
]

const CAT_MENU_ITEMS: MenuProps['items'] = [
  { key: 'sub', label: '➕ 新增子分类' },
  { key: 'sibling', label: '➕ 新增同级分类' },
  { type: 'divider' },
  { key: 'rename', label: '✏️ 重命名' },
  { key: 'delete', label: '🗑️ 删除', danger: true },
]

export default function MainLayout() {
  const { user, logout } = useAuth()
  const navigate = useNavigate()
  const location = useLocation()
  const screens = Grid.useBreakpoint()
  // 断点未测量时按桌面处理，避免 PC 端首帧闪一下抽屉布局
  const isMobile = screens.md === false

  const [drawerOpen, setDrawerOpen] = useState(false)
  const [openKeys, setOpenKeys] = useState<string[]>([])
  const [catTrees, setCatTrees] = useState<Record<Entity, CategoryTreeNode[]>>({
    supplier: [],
    purchaser: [],
    product: [],
  })
  const loadedRef = useRef<Record<Entity, boolean>>({ supplier: false, purchaser: false, product: false })

  const visibleTree = useMemo(() => filterByRole(MENU_TREE, user?.role ?? ''), [user?.role])

  // 当前选中项：pathname（去掉可能残留的 /app 前缀）+ query，与分类节点 key 格式一致
  const pathNoApp = location.pathname.replace(/^\/app(?=\/|$)/, '')
  const selectedKey = (pathNoApp.replace(/\/+$/, '') || '/') + location.search
  const selectedKeys = selectedKey === '/app' ? ['/'] : [selectedKey]

  const loadCategory = async (entity: Entity, force = false) => {
    if (!force && loadedRef.current[entity]) return
    loadedRef.current[entity] = true
    try {
      const tree = await listCategoryTree(entity)
      setCatTrees((prev) => ({ ...prev, [entity]: tree }))
    } catch {
      loadedRef.current[entity] = false
    }
  }

  const reloadCategory = (entity: Entity) => loadCategory(entity, true)

  // 老系统右键菜单动作：新增顶级分类 / 刷新分类树
  const handleRootMenu = async (action: string, entity: Entity) => {
    if (action === 'refresh') {
      void reloadCategory(entity)
      return
    }
    if (action === 'addRoot') {
      const name = window.prompt('请输入新的顶级分类名称：')
      if (!name) return
      const res = await createCategory({ name, parent_id: null, entity_type: entity, sort_order: 0 })
      if (res.ok) void reloadCategory(entity)
      else window.alert(res.message || '创建失败')
    }
  }

  // 老系统右键菜单动作：新增子/同级分类、重命名、删除
  const handleCatMenu = async (action: string, node: CategoryTreeNode, entity: Entity) => {
    if (action === 'sub' || action === 'sibling') {
      const name = window.prompt(action === 'sub' ? '请输入新的子分类名称：' : '请输入新的同级分类名称：')
      if (!name) return
      const parent_id = action === 'sub' ? node.id : (node.parent_id ?? null)
      const res = await createCategory({ name, parent_id, entity_type: entity, sort_order: 0 })
      if (res.ok) void reloadCategory(entity)
      else window.alert(res.message || '创建失败')
      return
    }
    if (action === 'rename') {
      const name = window.prompt('请输入新的分类名称：', node.name)
      if (!name || name === node.name) return
      const res = await renameCategory(node.id, name)
      if (res.ok) void reloadCategory(entity)
      else window.alert(res.message || '重命名失败')
      return
    }
    if (action === 'delete') {
      if (!window.confirm(`确定要删除分类"${node.name}"吗？\n\n注意：有子分类或已被引用的分类无法删除。`)) return
      const res = await deleteCategory(node.id)
      if (res.ok) void reloadCategory(entity)
      else window.alert(res.message || '删除失败')
    }
  }

  // 分类节点 → 菜单项：左键筛选跳转，右键分类管理（多层分类递归）
  const catToItems = (nodes: CategoryTreeNode[], entity: Entity): NonNullable<MenuProps['items']> =>
    nodes.map((n) => {
      const key = `/${entity}?category_id=${n.id}`
      const hasChildren = !!n.children && n.children.length > 0
      const label = (
        <Dropdown
          trigger={['contextMenu']}
          menu={{
            items: CAT_MENU_ITEMS,
            onClick: ({ key: k, domEvent }) => {
              domEvent.stopPropagation()
              void handleCatMenu(k, n, entity)
            },
          }}
        >
          {/* 老系统：分类节点头左键 = toggleNode + 按分类筛选。有子级时点击展开（父级菜单项），
              同时手动跳转筛选；叶子节点由 Menu onClick 负责跳转 */}
          <span style={{ userSelect: 'none' }} onClick={hasChildren ? () => navigate(key) : undefined}>
            📂 {n.name}
          </span>
        </Dropdown>
      )
      if (hasChildren) return { key, label, children: catToItems(n.children, entity) }
      return { key, label }
    })

  const buildMenuItems = (items: TreeItem[]): NonNullable<MenuProps['items']> =>
    items.map((it) => {
      const folder = ENTITY_FOLDERS[it.key]
      if (folder) {
        const { entity, route } = folder
        // 供应商/采购方/商品管理：文件夹头部左键直达全部列表页，
        // 右键弹出「新增顶级分类 / 刷新分类树」，分类子节点为动态数据
        const children: NonNullable<MenuProps['items']> = catToItems(catTrees[entity], entity)
        return {
          key: it.key,
          icon: <span>{it.icon}</span>,
          label: (
            <Dropdown
              trigger={['contextMenu']}
              menu={{
                items: ROOT_MENU_ITEMS,
                onClick: ({ key: k, domEvent }) => {
                  domEvent.stopPropagation()
                  void handleRootMenu(k, entity)
                },
              }}
            >
              <span
                style={{ userSelect: 'none' }}
                onClick={() => {
                  navigate(route)
                  if (isMobile) setDrawerOpen(false)
                }}
              >
                {it.label}
              </span>
            </Dropdown>
          ),
          children,
        }
      }
      if (it.children && it.children.length > 0) {
        return {
          key: it.key,
          icon: <span>{it.icon}</span>,
          label: it.label,
          children: buildMenuItems(it.children),
        }
      }
      return { key: it.key, icon: <span>{it.icon}</span>, label: it.label }
    })

  const menuItems = useMemo(
    () => buildMenuItems(visibleTree),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [visibleTree, catTrees],
  )

  const onMenuClick: MenuProps['onClick'] = ({ key }) => {
    if (key.startsWith('/')) {
      navigate(key)
      if (isMobile) setDrawerOpen(false)
    }
  }

  const onOpenChange = (keys: string[]) => {
    setOpenKeys(keys)
    const set = new Set(keys)
    if (set.has('g-supplier')) void loadCategory('supplier')
    if (set.has('g-purchaser')) void loadCategory('purchaser')
    if (set.has('g-product')) void loadCategory('product')
  }

  // 直接访问供应商/采购方/商品页面时，自动展开并加载对应分类树，保证高亮可见
  useEffect(() => {
    const seg = pathNoApp.replace(/^\/+/, '').split('/')[0]
    const folder = Object.keys(ENTITY_FOLDERS).find((k) => ENTITY_FOLDERS[k].entity === seg)
    if (folder) {
      setOpenKeys((prev) => Array.from(new Set([...prev, 'g-base', folder])))
      void loadCategory(seg as Entity)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [location.pathname])

  const onLogout = async () => {
    await logout()
    // 全量刷新回登录页，确保老页面与新页面 session 状态一致
    window.location.href = '/app/login'
  }

  const menuNode = (
    <Menu
      mode="inline"
      selectedKeys={selectedKeys}
      openKeys={openKeys}
      onOpenChange={onOpenChange}
      items={menuItems}
      onClick={onMenuClick}
    />
  )

  const brand = (
    <div style={{ height: 48, lineHeight: '48px', textAlign: 'center', fontWeight: 600 }}>食材收发系统</div>
  )

  return (
    <Layout style={{ minHeight: '100vh' }}>
      {!isMobile && (
        <Sider
          theme="light"
          width={230}
          style={{ overflowY: 'auto', height: '100vh', position: 'sticky', top: 0 }}
        >
          {brand}
          {menuNode}
        </Sider>
      )}
      {isMobile && (
        <Drawer
          placement="left"
          open={drawerOpen}
          onClose={() => setDrawerOpen(false)}
          width={260}
          closable={false}
          styles={{ body: { padding: 0 } }}
        >
          {brand}
          {menuNode}
        </Drawer>
      )}
      <Layout>
        <Header
          style={{
            background: '#fff',
            padding: isMobile ? '0 12px' : '0 24px',
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            borderBottom: '1px solid #f0f0f0',
          }}
        >
          <Space size="middle">
            {isMobile && <Button icon={<MenuOutlined />} onClick={() => setDrawerOpen(true)} />}
          </Space>
          <Space size="middle">
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
        <Content style={{ margin: isMobile ? 12 : 24 }}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  )
}
