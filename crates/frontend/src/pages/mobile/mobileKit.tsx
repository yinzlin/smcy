// 移动分拣族共享的 antd 表现层套件（纯 UI，不含业务逻辑）
// 统一技术栈为 React + AntD；通过 MobileShell 收敛移动端触控主题
import { memo, type ReactNode } from 'react'
import { Button, Card, Checkbox, ConfigProvider, DatePicker, Empty, Input, Space, Tag } from 'antd'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import dayjs from 'dayjs'

/** 各页头部主色（沿用老页各自的品牌色，仅改为 antd 纯色头部） */
export const MOBILE_ACCENT = {
  sort: '#1677ff',
  purchased: '#3b82f6',
  category: '#a855f7',
  supplier: '#10b981',
  comprehensive: '#06b6d4',
  todayPrice: '#f97316',
} as const

/** 分类名 → 头部色（与老页 getCategoryClass 的分类口径一致） */
export function categoryColor(name: string): string {
  if (name.includes('荤鲜')) return '#dc2626'
  if (name.includes('鲜蔬')) return '#16a34a'
  if (name.includes('粮油') || name.includes('干调')) return '#1d4ed8'
  if (name.includes('豆制品')) return '#ca8a04'
  if (name.includes('粉面')) return '#64748b'
  if (name.includes('水果')) return '#ea580c'
  return '#6b7280'
}

/** 移动端分拣族统一导航（此前各页链接集合不一致，此处收敛为单一入口清单） */
const MOBILE_NAV: { label: string; path: string }[] = [
  { label: '统筹分拣', path: '/mobile/sort' },
  { label: '按单位分拣', path: '/mobile/sort-by-purchaser' },
  { label: '按分类分拣', path: '/mobile/sort-by-category' },
  { label: '按供应商分拣', path: '/mobile/sort-by-supplier' },
  { label: '综合分拣', path: '/mobile/sort-comprehensive' },
  { label: '💰 今日进价', path: '/today-price' },
]

/** basename='/app'：把完整 pathname 还原为路由 key */
function currentRouteKey(pathname: string): string {
  return pathname.startsWith('/app') ? pathname.slice(4) : pathname
}

/**
 * 移动路由组外壳：ConfigProvider 触控 token + 页面底色。
 * 在 App.tsx 中以布局路由使用（子路由通过 <Outlet/> 渲染）。
 */
export function MobileShell() {
  return (
    <ConfigProvider
      theme={{
        token: { fontSize: 16, controlHeight: 44, controlHeightSM: 32, borderRadius: 8 },
        components: { Tag: { fontSizeSM: 12 } },
      }}
    >
      <div className="mobile-root">
        <Outlet />
      </div>
    </ConfigProvider>
  )
}

/** 吸顶页头：主色块（标题/说明）+ 统一切换导航 + 统计栏，children 为下方检索/搜索栏 */
export function MobileHeader(props: {
  color: string
  title: string
  info: string
  stats: { value: ReactNode; label: string }[]
  children?: ReactNode
}) {
  const navigate = useNavigate()
  const location = useLocation()
  const active = currentRouteKey(location.pathname)

  return (
    <div className="mobile-sticky">
      <div className="mobile-header" style={{ background: props.color }}>
        <h1>{props.title}</h1>
        <div className="mobile-header-info">{props.info}</div>
        <div className="mobile-nav">
          {MOBILE_NAV.map((item) => (
            <Button
              key={item.path}
              size="small"
              className={`mobile-nav-link${active === item.path ? ' active' : ''}`}
              onClick={() => void navigate(item.path)}
            >
              {item.label}
            </Button>
          ))}
        </div>
        <div className="mobile-stats">
          {props.stats.map((s) => (
            <div className="mobile-stat" key={s.label}>
              <b>{s.value}</b>
              <span>{s.label}</span>
            </div>
          ))}
        </div>
      </div>
      {props.children}
    </div>
  )
}

/** 历史日期检索栏（留空=当前待分拣，与老页 data 口径一致） */
export function MobileDateBar(props: {
  value: string
  onChange: (v: string) => void
  onSearch: () => void
  onClear: () => void
}) {
  return (
    <div className="mobile-bar">
      <DatePicker
        allowClear
        inputReadOnly
        placeholder="选择日期（留空=当前待分拣）"
        value={props.value ? dayjs(props.value) : null}
        onChange={(d) => props.onChange(d ? d.format('YYYY-MM-DD') : '')}
        style={{ flex: 1, minWidth: 0 }}
      />
      <Button type="primary" onClick={props.onSearch}>
        检索
      </Button>
      <Button onClick={props.onClear}>清除</Button>
    </div>
  )
}

/** 商品名称搜索栏（即时过滤，与老页 oninput 行为一致） */
export function MobileSearchBar(props: {
  value: string
  onChange: (v: string) => void
  onClear: () => void
  placeholder?: string
}) {
  return (
    <div className="mobile-bar">
      <Input
        allowClear
        placeholder={props.placeholder ?? '搜索商品名称...'}
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        style={{ flex: 1, minWidth: 0 }}
      />
      <Button onClick={props.onClear}>清除</Button>
    </div>
  )
}

/** 吸底操作栏（自动换行保证触控尺寸） */
export function MobileBottomBar(props: { children: ReactNode }) {
  return <div className="mobile-bottom-bar">{props.children}</div>
}

/** 分组卡片：彩色分组头 + 无内边距卡片体 */
export function MobileSection(props: { color: string; title: ReactNode; extra?: ReactNode; children: ReactNode }) {
  return (
    <Card
      size="small"
      className="mobile-section"
      title={<span style={{ color: '#fff' }}>{props.title}</span>}
      extra={<span style={{ color: '#fff', fontSize: 13, opacity: 0.9 }}>{props.extra}</span>}
      styles={{ header: { background: props.color, borderBottom: 'none', minHeight: 44 }, body: { padding: 0 } }}
    >
      {props.children}
    </Card>
  )
}

export function MobileEmpty(props: { text: string }) {
  return <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={props.text} style={{ padding: '40px 0' }} />
}

/** 卡片体内部的次级分组头（采购单位等），用于「分类→单位」「供应商→单位」两级分组 */
export function MobileSubHeader(props: { title: ReactNode; extra?: ReactNode }) {
  return (
    <div className="mobile-subheader">
      <span className="mobile-subheader-title">{props.title}</span>
      <span className="mobile-subheader-extra">{props.extra}</span>
    </div>
  )
}

/** 列表行骨架（勾选框可选，主内容自适应，右侧固定操作区） */
export function MobileListRow(props: { checked?: boolean; onClick?: () => void; children: ReactNode }) {
  return (
    <div className={`mobile-row${props.checked ? ' checked' : ''}`} onClick={props.onClick}>
      {props.children}
    </div>
  )
}

export interface MobileSortRowProps {
  id: number
  checked: boolean
  /** 商品名 */
  title: string
  /** 基本单位（同时作为数量角标单位） */
  unit: string
  quantity: number
  /** 补充标签文本（采购单位/单价等） */
  extraText?: string
  remark?: string
  /** 形如「修正: 3→5」的预格式化文本 */
  correctionText?: string
  corrected: boolean
  correctedRaw?: number
  /** 修正输入框宽版（按供应商分拣页） */
  inputWide?: boolean
  /** 修正输入框用文本型（保留小数输入过程，按供应商分拣页） */
  inputText?: boolean
  /** 清除修正后递增以强制重挂载输入框 */
  correctionsVersion: number
  onToggle: (id: number) => void
  onCorrectionCommit: (id: number, raw: string) => void
}

/**
 * 分拣条目行：整行点击切换勾选，右侧展示数量并可录入修正值。
 * memo + 稳定回调 + 仅原始类型 props，避免长列表重复渲染。
 */
export const MobileSortRow = memo(function MobileSortRow(props: MobileSortRowProps) {
  return (
    <MobileListRow checked={props.checked} onClick={() => props.onToggle(props.id)}>
      <Checkbox
        checked={props.checked}
        onClick={(e) => e.stopPropagation()}
        onChange={() => props.onToggle(props.id)}
      />
      <div className="mobile-row-main">
        <div className="mobile-row-title">{props.title}</div>
        <Space size={4} wrap>
          <Tag>{props.unit}</Tag>
          {props.extraText ? <Tag>{props.extraText}</Tag> : null}
          {props.remark ? <Tag color="gold">备注: {props.remark}</Tag> : null}
          {props.correctionText ? <Tag color="orange">{props.correctionText}</Tag> : null}
        </Space>
      </div>
      <div className="mobile-row-side">
        <div className="mobile-qty-value">{props.quantity}</div>
        <div className="mobile-qty-unit">{props.unit}</div>
        <Input
          key={`${props.id}:${props.correctionsVersion}`}
          type={props.inputText ? 'text' : 'number'}
          inputMode={props.inputText ? 'decimal' : 'numeric'}
          placeholder={props.inputWide ? '修正数量' : '修正'}
          defaultValue={props.corrected && props.correctedRaw !== undefined ? String(props.correctedRaw) : ''}
          onClick={(e) => e.stopPropagation()}
          onBlur={(e) => props.onCorrectionCommit(props.id, e.currentTarget.value)}
          onPressEnter={(e) => props.onCorrectionCommit(props.id, (e.target as HTMLInputElement).value)}
          style={{ width: props.inputWide ? 96 : 72, textAlign: 'center' }}
        />
      </div>
    </MobileListRow>
  )
})
