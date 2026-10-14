import { useMemo, useRef, useState } from 'react'
import { AutoComplete, Button, Card, DatePicker, Select, Spin, Table } from 'antd'
import { SearchOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2, fmtMoney } from '../../utils/format'
import { listPurchasers } from '../../api/sales'
import { searchProducts, type ProductSearchItem } from '../../api/purchase'
import {
  fetchPriceTrend,
  fetchSalesPrice,
  type PriceTrendDetail,
  type PriceTrendPoint,
  type PriceTrendResp,
  type SalesPriceRow,
} from '../../api/query/prices'

const PAGE_SIZE = 20

// ===== 价格趋势图（老页面为 Chart.js 折线图，此处用内联 SVG 还原：双系列均价线 + max~min 区间带 + hover 提示）=====

const TREND_W = 960
const TREND_H = 320
const M = { left: 64, right: 16, top: 14, bottom: 46 }
const PURCHASE_COLOR = '#dc3545'
const SELLING_COLOR = '#0d6efd'

interface SeriesDef {
  label: string
  color: string
  fill: string
  points: PriceTrendPoint[]
}

function buildPath(pts: Array<{ x: number; y: number }>): string {
  return pts.map((p, i) => `${i === 0 ? 'M' : 'L'}${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ')
}

/** 明细行文案照抄老页面 tooltip afterBody：'名称 | 单位 ¥单价 ×数量 → 基础单价¥x' */
function detailLine(d: PriceTrendDetail): string {
  const party = d.supplier_name ?? d.purchaser_name ?? ''
  return `${party} | ${d.unit} ¥${d.unit_price.toFixed(2)} ×${d.quantity} → 基础单价¥${d.base_unit_price.toFixed(2)}`
}

function TrendChart({ trend }: { trend: PriceTrendResp }) {
  const [hoverIdx, setHoverIdx] = useState<number | null>(null)
  const [hoverPos, setHoverPos] = useState<{ x: number; width: number } | null>(null)
  const boxRef = useRef<HTMLDivElement>(null)

  const purchaseMap = useMemo(() => new Map(trend.purchase_points.map((p) => [p.date, p])), [trend])
  const sellingMap = useMemo(() => new Map(trend.selling_points.map((p) => [p.date, p])), [trend])
  const labels = useMemo(() => {
    const s = new Set<string>()
    trend.purchase_points.forEach((p) => s.add(p.date))
    trend.selling_points.forEach((p) => s.add(p.date))
    return Array.from(s).sort()
  }, [trend])

  const series: SeriesDef[] = useMemo(
    () => [
      { label: '进价（加权均价）', color: PURCHASE_COLOR, fill: 'rgba(220,53,69,0.12)', points: trend.purchase_points },
      { label: '售价（加权均价）', color: SELLING_COLOR, fill: 'rgba(13,110,253,0.12)', points: trend.selling_points },
    ],
    [trend],
  )

  // Y 轴范围：老页面 beginAtZero=false，取全部 min/max 再外扩 10%
  const yRange = useMemo(() => {
    let vMin = Infinity
    let vMax = -Infinity
    for (const p of [...trend.purchase_points, ...trend.selling_points]) {
      if (p.min_price < vMin) vMin = p.min_price
      if (p.max_price > vMax) vMax = p.max_price
    }
    if (!Number.isFinite(vMin) || !Number.isFinite(vMax)) return null
    if (vMin === vMax) {
      vMin -= 1
      vMax += 1
    }
    const pad = (vMax - vMin) * 0.1
    return { min: vMin - pad, max: vMax + pad }
  }, [trend])

  if (labels.length === 0 || yRange === null) {
    return <div style={{ color: '#999', textAlign: 'center', padding: '48px 0' }}>暂无趋势数据</div>
  }

  const plotW = TREND_W - M.left - M.right
  const plotH = TREND_H - M.top - M.bottom
  const xAt = (i: number) => (labels.length === 1 ? M.left + plotW / 2 : M.left + (i * plotW) / (labels.length - 1))
  const yAt = (v: number) => M.top + ((yRange.max - v) / (yRange.max - yRange.min)) * plotH

  // X 轴刻度：最多约 12 个，避免拥挤
  const xStep = Math.max(1, Math.ceil(labels.length / 12))
  const yTicks = [0, 1, 2, 3, 4].map((k) => yRange.min + ((yRange.max - yRange.min) * k) / 4)

  const onMove = (e: React.MouseEvent) => {
    const box = boxRef.current
    if (!box) return
    const rect = box.getBoundingClientRect()
    const scale = rect.width / TREND_W
    const svgX = (e.clientX - rect.left) / scale
    const i = Math.round(((svgX - M.left) / plotW) * (labels.length - 1))
    const clamped = Math.max(0, Math.min(labels.length - 1, i))
    setHoverIdx(clamped)
    setHoverPos({ x: e.clientX - rect.left, width: rect.width })
  }
  const onLeave = () => {
    setHoverIdx(null)
    setHoverPos(null)
  }

  const hoverDate = hoverIdx !== null ? labels[hoverIdx] : null
  const hoverPurchase = hoverDate ? purchaseMap.get(hoverDate) : undefined
  const hoverSelling = hoverDate ? sellingMap.get(hoverDate) : undefined
  const tooltipLeft = hoverPos ? (hoverPos.x > hoverPos.width - 300 ? hoverPos.x - 288 : hoverPos.x + 12) : 0

  return (
    <div>
      {/* 图例（老页面 legend position: top，仅显示两条均价线） */}
      <div style={{ display: 'flex', justifyContent: 'center', gap: 24, marginBottom: 4 }}>
        {series.map((s) => (
          <span key={s.label} style={{ fontSize: 12, color: '#666', display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            <span style={{ display: 'inline-block', width: 18, height: 3, background: s.color, borderRadius: 2 }} />
            {s.label}
          </span>
        ))}
      </div>
      <div ref={boxRef} style={{ position: 'relative' }} onMouseLeave={onLeave}>
        <svg viewBox={`0 0 ${TREND_W} ${TREND_H}`} style={{ width: '100%', height: 'auto', display: 'block' }}>
          {/* 网格线 + Y 轴刻度 */}
          {yTicks.map((v, k) => (
            <g key={k}>
              <line x1={M.left} y1={yAt(v)} x2={M.left + plotW} y2={yAt(v)} stroke="#f0f0f0" />
              <text x={M.left - 6} y={yAt(v) + 4} textAnchor="end" fontSize={11} fill="#666">
                {Number(v.toFixed(2))}
              </text>
            </g>
          ))}
          {/* 坐标轴 */}
          <line x1={M.left} y1={M.top} x2={M.left} y2={M.top + plotH} stroke="#d9d9d9" />
          <line x1={M.left} y1={M.top + plotH} x2={M.left + plotW} y2={M.top + plotH} stroke="#d9d9d9" />
          {/* X 轴刻度 */}
          {labels.map((d, i) =>
            i % xStep === 0 ? (
              <text key={d} x={xAt(i)} y={M.top + plotH + 18} textAnchor="middle" fontSize={11} fill="#666">
                {d}
              </text>
            ) : null,
          )}
          {/* 轴标题（老页面 scales.x.title=日期 / scales.y.title=价格（元）） */}
          <text x={8} y={M.top + 4} fontSize={12} fill="#666">
            价格（元）
          </text>
          <text x={M.left + plotW / 2} y={TREND_H - 6} textAnchor="middle" fontSize={12} fill="#666">
            日期
          </text>
          {/* hover 竖向参考线 */}
          {hoverIdx !== null && (
            <line x1={xAt(hoverIdx)} y1={M.top} x2={xAt(hoverIdx)} y2={M.top + plotH} stroke="#999" strokeDasharray="4 3" />
          )}
          {/* 系列：先画 max~min 区间带，再画均价折线与数据点（spanGaps：跨缺口直连；count>1 时点半径 5，否则 3） */}
          {series.map((s) => {
            const map = new Map(s.points.map((p) => [p.date, p]))
            const idxs = labels.map((d, i) => (map.has(d) ? i : -1)).filter((i) => i >= 0)
            if (idxs.length === 0) return null
            const up = idxs.map((i) => ({ x: xAt(i), y: yAt(map.get(labels[i])!.max_price) }))
            const down = idxs.map((i) => ({ x: xAt(i), y: yAt(map.get(labels[i])!.min_price) })).reverse()
            const avg = idxs.map((i) => ({ x: xAt(i), y: yAt(map.get(labels[i])!.avg_price) }))
            return (
              <g key={s.label}>
                <path d={`${buildPath([...up, ...down])} Z`} fill={s.fill} stroke="none" />
                <path d={buildPath(avg)} fill="none" stroke={s.color} strokeWidth={2} />
                {idxs.map((i) => {
                  const p = map.get(labels[i])!
                  return (
                    <circle
                      key={labels[i]}
                      cx={xAt(i)}
                      cy={yAt(p.avg_price)}
                      r={p.count > 1 ? 5 : 3}
                      fill={s.color}
                      stroke="#fff"
                      strokeWidth={1}
                    />
                  )
                })}
              </g>
            )
          })}
          {/* 透明覆盖层：hover 按最近日期取数（老页面 tooltip mode: index） */}
          <rect x={M.left} y={M.top} width={plotW} height={plotH} fill="transparent" onMouseMove={onMove} />
        </svg>
        {/* 悬浮提示：照抄老页面 tooltip 文案（加权区间 + 采购/销售明细） */}
        {hoverDate && (hoverPurchase || hoverSelling) && hoverPos && (
          <div
            style={{
              position: 'absolute',
              left: tooltipLeft,
              top: 8,
              width: 276,
              maxHeight: 300,
              overflowY: 'auto',
              background: 'rgba(255,255,255,0.97)',
              border: '1px solid #e0e0e0',
              borderRadius: 4,
              boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
              padding: '8px 10px',
              fontSize: 12,
              lineHeight: '20px',
              pointerEvents: 'none',
              zIndex: 10,
            }}
          >
            <div style={{ fontWeight: 600 }}>{hoverDate}</div>
            {hoverPurchase && (
              <div style={{ color: PURCHASE_COLOR }}>
                进价（加权均价）: ¥{hoverPurchase.avg_price.toFixed(2)}
                {hoverPurchase.count > 1 &&
                  `（${hoverPurchase.count}笔加权 | ¥${hoverPurchase.min_price.toFixed(2)}~¥${hoverPurchase.max_price.toFixed(2)}）`}
              </div>
            )}
            {hoverPurchase && hoverPurchase.details.length > 0 && (
              <div style={{ color: '#666' }}>
                <div>── 采购明细（{hoverPurchase.details.length}笔）──</div>
                {hoverPurchase.details.map((d, i) => (
                  <div key={i} style={{ paddingLeft: 8 }}>
                    {detailLine(d)}
                  </div>
                ))}
              </div>
            )}
            {hoverSelling && (
              <div style={{ color: SELLING_COLOR }}>
                售价（加权均价）: ¥{hoverSelling.avg_price.toFixed(2)}
                {hoverSelling.count > 1 &&
                  `（${hoverSelling.count}笔加权 | ¥${hoverSelling.min_price.toFixed(2)}~¥${hoverSelling.max_price.toFixed(2)}）`}
              </div>
            )}
            {hoverSelling && hoverSelling.details.length > 0 && (
              <div style={{ color: '#666' }}>
                <div>── 销售明细（{hoverSelling.details.length}笔）──</div>
                {hoverSelling.details.map((d, i) => (
                  <div key={i} style={{ paddingLeft: 8 }}>
                    {detailLine(d)}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

// ===== 页面 =====

interface TrendQuery {
  product_id: number
  start_date: string
  end_date: string
}

export default function SalesPricePage() {
  // 商品模糊搜索联想（老页面 250ms 防抖 + 前 50 条 + 「无匹配商品」提示）
  const [keyword, setKeyword] = useState('')
  const [productId, setProductId] = useState<number | null>(null)
  const [searchMap, setSearchMap] = useState<ProductSearchItem[]>([])
  const [searching, setSearching] = useState(false)
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [purchaserId, setPurchaserId] = useState<number | null>(null)

  // 老页面进入即调用 searchSalesPrice()：默认条件自动加载一次
  const [submitted, setSubmitted] = useState({ product_name: '', product_id: null as number | null, purchaser_id: null as number | null })
  const [page, setPage] = useState(1)
  // 趋势图仅在选择具体商品并点「查询」后出现（老页面 trendCard 默认 display:none）
  const [trendQuery, setTrendQuery] = useState<TrendQuery | null>(null)

  const { data: purchasers = [] } = useQuery({ queryKey: ['purchasers'], queryFn: listPurchasers })

  const { data: resp, isLoading } = useQuery({
    queryKey: ['salesPrice', submitted, page],
    queryFn: () =>
      fetchSalesPrice({
        // 老页面：仅在选中具体商品时才传 product_name
        product_name: submitted.product_id ? submitted.product_name : '',
        product_id: submitted.product_id,
        purchaser_id: submitted.purchaser_id,
        page,
        page_size: PAGE_SIZE,
      }),
    placeholderData: (prev) => prev,
  })
  const rows = resp?.data ?? []
  const totalQty = rows.reduce((s, r) => s + (Number(r.quantity) || 0), 0)

  const { data: trend, isFetching: trendLoading } = useQuery({
    queryKey: ['priceTrend', trendQuery],
    queryFn: () => fetchPriceTrend(trendQuery!.product_id, trendQuery!.start_date, trendQuery!.end_date),
    enabled: trendQuery !== null,
    placeholderData: (prev) => prev,
  })

  const handleKeywordChange = (v: string) => {
    // 选中联想项时 AntD 会以 option value（商品 id 字符串）回调 onChange，跳过避免误清 productId
    if (searchMap.some((p) => String(p.id) === v)) return
    setKeyword(v)
    // 老页面：清空输入即清掉已选商品；有内容时 250ms 防抖联想
    if (!v.trim()) {
      setProductId(null)
      setSearchMap([])
      if (searchTimer.current) clearTimeout(searchTimer.current)
      return
    }
    if (searchTimer.current) clearTimeout(searchTimer.current)
    setSearching(true)
    searchTimer.current = setTimeout(async () => {
      try {
        const products = await searchProducts(v.trim())
        setSearchMap(products)
      } catch {
        setSearchMap([])
      } finally {
        setSearching(false)
      }
    }, 250)
  }

  const productOptions = searchMap.slice(0, 50).map((p) => ({
    value: String(p.id),
    product: p,
    label: (
      <div>
        {/* 老页面 label：名称 (规格) / 基础单位，第二行灰字进价 */}
        <strong>
          {p.name}
          {p.spec ? ` (${p.spec})` : ''}
          {p.base_unit ? ` / ${p.base_unit}` : ''}
        </strong>
        {(p.purchase_price || 0) > 0 && (
          <div>
            <small style={{ color: '#6c757d' }}>进价: {p.purchase_price}</small>
          </div>
        )}
      </div>
    ),
  }))

  const search = () => {
    const sid = productId
    setSubmitted({ product_name: keyword.trim(), product_id: sid, purchaser_id: purchaserId })
    setPage(1)
    setTrendQuery(sid !== null ? { product_id: sid, start_date: startDate, end_date: endDate } : null)
  }

  const columns: TableProps<SalesPriceRow>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name', width: 200 },
    { title: '规格', dataIndex: 'spec', width: 140, render: (v: string | null) => v || '-' },
    { title: '采购单位', dataIndex: 'purchaser_name', width: 160 },
    { title: '销售单价', dataIndex: 'unit_price', width: 110, align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '销售日期', dataIndex: 'order_date', width: 110 },
    { title: '销售数量', dataIndex: 'quantity', width: 110, align: 'right', render: (v: number) => fmt2(v) },
  ]

  const totalPages = resp?.total_pages ?? 1

  return (
    <>
      <ReportCard
        title="销售价格查询"
        filters={
          <>
            <FilterItem label="商品名称" width={280}>
              <AutoComplete
                style={{ width: '100%' }}
                value={keyword}
                placeholder="输入商品名或别称进行模糊搜索"
                options={productOptions}
                notFoundContent={
                  searching ? (
                    <Spin size="small" />
                  ) : searchMap.length === 0 ? (
                    <div style={{ padding: '4px 8px', color: '#999' }}>无匹配商品</div>
                  ) : null
                }
                onChange={handleKeywordChange}
                onSelect={(_v, opt) => {
                  const p = (opt as { product?: ProductSearchItem }).product
                  if (!p) return
                  // 老页面 selectSuggestProduct：输入框回填商品名并记录 productId
                  setProductId(p.id)
                  setKeyword(p.name)
                }}
              />
            </FilterItem>
            <FilterItem label="开始日期" width={150}>
              <DatePicker
                style={{ width: '100%' }}
                value={startDate ? dayjs(startDate) : null}
                onChange={(d) => setStartDate(d ? d.format('YYYY-MM-DD') : '')}
              />
            </FilterItem>
            <FilterItem label="结束日期" width={150}>
              <DatePicker
                style={{ width: '100%' }}
                value={endDate ? dayjs(endDate) : null}
                onChange={(d) => setEndDate(d ? d.format('YYYY-MM-DD') : '')}
              />
            </FilterItem>
            <FilterItem label="采购单位" width={200}>
              <Select<number | null>
                style={{ width: '100%' }}
                value={purchaserId}
                onChange={(v) => setPurchaserId(v)}
                options={[{ value: null, label: '全部采购单位' }, ...purchasers.map((p) => ({ value: p.id, label: p.name }))]}
              />
            </FilterItem>
          </>
        }
        actions={
          <Button type="primary" icon={<SearchOutlined />} onClick={search}>
            查询
          </Button>
        }
      >
        <Table<SalesPriceRow>
          rowKey={(_, i) => String(i)}
          size="small"
          loading={isLoading}
          columns={columns}
          dataSource={rows}
          pagination={{
            current: page,
            pageSize: PAGE_SIZE,
            total: resp?.total ?? 0,
            showSizeChanger: false,
            showTotal: () => `共 ${resp?.total ?? 0} 条记录，当前第 ${page}/${totalPages} 页`,
            onChange: (p) => setPage(p),
          }}
          summary={() =>
            rows.length > 0 ? (
              <Table.Summary.Row style={{ background: '#fafafa', fontWeight: 600 }}>
                {/* 老页面合计行：前 5 列合并显示「合计」，末列为当页数量合计 */}
                <Table.Summary.Cell index={0} colSpan={5}>
                  合计
                </Table.Summary.Cell>
                <Table.Summary.Cell index={1} align="right">
                  {fmt2(totalQty)}
                </Table.Summary.Cell>
              </Table.Summary.Row>
            ) : null
          }
        />
      </ReportCard>

      {/* 价格趋势卡片（选中具体商品并查询后显示） */}
      {trendQuery !== null && (
        <Card size="small" style={{ marginTop: 16 }}>
          <div style={{ fontSize: 15, fontWeight: 600, marginBottom: 8 }}>
            价格趋势
            {trend ? ` - ${trend.product_name}${trend.base_unit ? `（基础单位：${trend.base_unit}）` : ''}` : ''}
          </div>
          <div style={{ color: '#999', fontSize: 12, marginBottom: 8 }}>
            进价来源于历史采购单实际成交价，售价来源于历史销售单实际成交价（均已换算为同一基础单位），保留历史原值。
          </div>
          {trendLoading && !trend ? (
            <div style={{ textAlign: 'center', padding: '60px 0' }}>
              <Spin />
            </div>
          ) : trend ? (
            <TrendChart trend={trend} />
          ) : (
            <div style={{ color: '#999', textAlign: 'center', padding: '48px 0' }}>暂无趋势数据</div>
          )}
        </Card>
      )}
    </>
  )
}
