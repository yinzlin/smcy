import { Alert, Button, Card, Col, Row, Typography } from 'antd'
import { useNavigate } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import dayjs from 'dayjs'
import { useAuth } from '../stores/auth'
import { apiFetch } from '../api/client'

/** 老首页 /supplier/list 返回字段（仅取证照/合同到期提醒所需） */
interface SupplierListItem {
  id: number
  name: string
  is_designated: number
  designated_no: string | null
  audit_status: string | null
  license_expire: string | null
  contract_end: string | null
}

/** 单条提醒里的一个日期片段（许可证/供货合同） */
interface AlertPart {
  label: string
  date: string
  suffix: string
  /** true=已过期（红），false=30天内（黄），plain=日期无法解析（不着色） */
  danger: boolean
  plain: boolean
}

interface AlertItem {
  id: number
  name: string
  no: string
  parts: AlertPart[]
}

/** 快捷入口卡片：标题/图标/说明照抄老首页，链接映射到 /app 新路由 */
interface EntryCard {
  icon: string
  title: string
  desc: string
  path: string
  color: string
  superAdminOnly?: boolean
}

const ENTRY_CARDS: EntryCard[] = [
  { icon: '🏭', title: '供应商档案', desc: '供应商档案与定点资质维护', path: '/supplier', color: '#0d6efd' },
  { icon: '🏢', title: '采购方管理', desc: '管理采购单位信息', path: '/purchaser', color: '#198754' },
  { icon: '📦', title: '商品管理', desc: '管理食材商品信息', path: '/product', color: '#0dcaf0' },
  { icon: '🏠', title: '库存管理', desc: '查看和管理库存', path: '/warehouse', color: '#ffc107' },
  { icon: '🛒', title: '采购订单', desc: '创建和管理采购订单', path: '/purchase', color: '#dc3545' },
  { icon: '💰', title: '销售订单', desc: '创建和管理销售订单', path: '/sales', color: '#6c757d' },
  { icon: '🧺', title: '采购分拣', desc: '统筹汇总所有采购需求', path: '/query/purchase-summary', color: '#059669' },
  { icon: '🏢', title: '按单位分拣', desc: '按采购单位分组采购', path: '/query/purchaser-balance', color: '#f59e0b' },
  { icon: '🗂️', title: '按分类分拣', desc: '按商品分类汇总采购', path: '/query/category-stats', color: '#7c3aed' },
  { icon: '🏭', title: '按供应商分拣', desc: '按供应商分组汇总采购', path: '/query/supplier-balance', color: '#10b981' },
  { icon: '🔀', title: '综合分拣', desc: '按采购单位+分类汇总', path: '/query/overview', color: '#06b6d4' },
  { icon: '💰', title: '今日进价采集', desc: '按供应商采集录入今日进价', path: '/today-price', color: '#f97316', superAdminOnly: true },
]

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/

/** 解析 YYYY-MM-DD（前 10 位），失败返回 null（对应老页 chrono 解析失败分支） */
function parseExpireDate(raw: string | null): dayjs.Dayjs | null {
  const head = (raw ?? '').trim().slice(0, 10)
  return DATE_RE.test(head) ? dayjs(head).startOf('day') : null
}

/**
 * 复刻老首页证照/合同到期提醒的服务端查询逻辑：
 * 定点(is_designated=1)且已审核确认(audit_status='confirmed')，
 * 许可证或供货合同日期非空且 <= 今天+30 天，按 COALESCE(许可证, 合同) 升序。
 * 已过期标红（已过期），30 天内标黄（剩N天），无法解析的日期原样展示。
 */
function buildLicenseAlertItems(list: SupplierListItem[]): AlertItem[] {
  const today = dayjs().startOf('day')
  const limit = today.add(30, 'day')
  const items: (AlertItem & { sortKey: string })[] = []
  for (const s of list) {
    if (s.is_designated !== 1 || s.audit_status !== 'confirmed') continue
    const license = parseExpireDate(s.license_expire)
    const contract = parseExpireDate(s.contract_end)
    const licenseHit = license !== null && !license.isAfter(limit)
    const contractHit = contract !== null && !contract.isAfter(limit)
    if (!licenseHit && !contractHit) continue
    const parts: AlertPart[] = []
    for (const [label, parsed, raw] of [
      ['许可证', license, s.license_expire],
      ['供货合同', contract, s.contract_end],
    ] as const) {
      const d = (raw ?? '').trim()
      if (!d) continue
      if (parsed === null) {
        parts.push({ label, date: d, suffix: '', danger: false, plain: true })
      } else {
        const days = parsed.diff(today, 'day')
        parts.push(
          days < 0
            ? { label, date: d, suffix: '（已过期）', danger: true, plain: false }
            : { label, date: d, suffix: `（剩${days}天）`, danger: false, plain: false },
        )
      }
    }
    items.push({ id: s.id, name: s.name, no: (s.designated_no ?? '').trim(), parts, sortKey: s.license_expire ?? s.contract_end ?? '' })
  }
  items.sort((a, b) => (a.sortKey < b.sortKey ? -1 : a.sortKey > b.sortKey ? 1 : 0))
  return items.map(({ id, name, no, parts }) => ({ id, name, no, parts }))
}

export default function HomePage() {
  const { user } = useAuth()
  const navigate = useNavigate()
  // 老首页权限规则：所有登录用户可见全部入口卡片，"今日进价采集"卡片与到期提醒仅超级管理员
  const isSuperAdmin = user?.role === 'super_admin'

  const { data: alertItems = [] } = useQuery({
    queryKey: ['supplier-license-alert'],
    queryFn: async () => buildLicenseAlertItems(await apiFetch<SupplierListItem[]>('/api/supplier/list')),
    enabled: isSuperAdmin,
    staleTime: 60_000,
  })

  const cards = ENTRY_CARDS.filter((c) => !c.superAdminOnly || isSuperAdmin)

  return (
    <>
      {/* 老首页：row row-cols-1 row-cols-md-2 row-cols-lg-3 g-4 */}
      <Row gutter={[24, 24]}>
        {cards.map((c) => (
          <Col key={c.title} xs={24} sm={12} lg={8}>
            <Card style={{ height: '100%', background: c.color }} styles={{ body: { padding: 16 } }}>
              <Typography.Title level={5} style={{ color: '#fff', marginBottom: 8 }}>
                {c.icon} {c.title}
              </Typography.Title>
              <Typography.Paragraph style={{ color: 'rgba(255,255,255,0.9)', marginBottom: 16 }}>{c.desc}</Typography.Paragraph>
              {/* 老卡片 btn btn-light：点击进入对应新路由 */}
              <Button
                style={{ background: '#f8f9fa', borderColor: '#f8f9fa', color: '#212529', boxShadow: 'none' }}
                onClick={() => void navigate(c.path)}
              >
                进入
              </Button>
            </Card>
          </Col>
        ))}
      </Row>
      {/* 老首页：alert-warning mt-4 mb-0，无到期数据时整块不渲染 */}
      {isSuperAdmin && alertItems.length > 0 && (
        <Alert
          type="warning"
          style={{ marginTop: 24 }}
          message={<span style={{ fontWeight: 600 }}>⚠ 定点供应商证照/合同到期提醒（30天内）</span>}
          description={
            <ul style={{ margin: '4px 0 0', paddingLeft: 20 }}>
              {alertItems.map((item) => (
                <li key={item.id}>
                  {item.name}{' '}
                  {item.no && <Typography.Text type="secondary">({item.no})</Typography.Text>}：
                  {item.parts.map((p, i) => (
                    <span key={i}>
                      {i > 0 && '；'}
                      <Typography.Text style={p.plain ? undefined : { color: p.danger ? '#cf1322' : '#d48806', fontWeight: 600 }}>
                        {`${p.label} ${p.date}${p.suffix}`}
                      </Typography.Text>
                    </span>
                  ))}
                </li>
              ))}
            </ul>
          }
        />
      )}
    </>
  )
}
