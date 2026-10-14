import { useState } from 'react'
import { Button, Col, DatePicker, Row, Table } from 'antd'
import { DownloadOutlined, SearchOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import dayjs from 'dayjs'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2 } from '../../utils/format'
import {
  queryStockSummary,
  stockSummaryExportUrl,
  type StockSummaryKind,
  type StockSummaryRow,
} from '../../api/query/stock'

const GREEN = '#389e0d'
const RED = '#cf1322'

/** 老页面进入即按「近30天」自动查询 */
const INITIAL_RANGE = {
  startDate: dayjs().subtract(29, 'day').format('YYYY-MM-DD'),
  endDate: dayjs().format('YYYY-MM-DD'),
}

/**
 * 出入库统计共用视图：真实/报销两套口径结构完全相同，仅标题/说明/API 不同。
 * StockSummaryPage（真实）为本页主实现，StockSummaryReimbursePage 传入报销口径参数复用。
 */
export function StockSummaryView({ title, desc, kind }: { title: string; desc: string; kind: StockSummaryKind }) {
  const [startDate, setStartDate] = useState(INITIAL_RANGE.startDate)
  const [endDate, setEndDate] = useState(INITIAL_RANGE.endDate)
  const [submitted, setSubmitted] = useState(INITIAL_RANGE)

  const { data, isLoading } = useQuery({
    queryKey: ['stockSummary', kind, submitted],
    queryFn: () => queryStockSummary(kind, submitted),
    placeholderData: (prev) => prev,
  })

  const search = () => setSubmitted({ startDate, endDate })

  // 快捷日期按钮：设置起止后立即查询（照老页面 resetDate）
  const resetDate = (range: 'today' | '7d' | '30d' | 'month' | 'all') => {
    const today = dayjs()
    let s = ''
    let e = ''
    if (range === 'today') {
      s = today.format('YYYY-MM-DD')
      e = s
    } else if (range === '7d') {
      s = today.subtract(6, 'day').format('YYYY-MM-DD')
      e = today.format('YYYY-MM-DD')
    } else if (range === '30d') {
      s = today.subtract(29, 'day').format('YYYY-MM-DD')
      e = today.format('YYYY-MM-DD')
    } else if (range === 'month') {
      s = today.startOf('month').format('YYYY-MM-DD')
      e = today.format('YYYY-MM-DD')
    }
    setStartDate(s)
    setEndDate(e)
    setSubmitted({ startDate: s, endDate: e })
  }

  const exportExcel = () => {
    window.location.href = stockSummaryExportUrl(kind, { startDate, endDate })
  }

  const rows = data?.rows ?? []

  const columns: TableProps<StockSummaryRow>['columns'] = [
    {
      title: '日期',
      dataIndex: 'day',
      width: 110,
      // 同一日期只在首行显示（照老页面 prevDay 逻辑，含合计行）
      render: (v: string, _o, index) => (index > 0 && rows[index - 1]?.day === v ? '' : v),
    },
    { title: '仓库', dataIndex: 'warehouse_name', width: 130 },
    { title: '入库金额', dataIndex: 'in_amount', width: 110, align: 'right' as const, render: (v: number) => <span style={{ color: GREEN }}>{fmt2(v)}</span> },
    { title: '入库单数', dataIndex: 'in_order_count', width: 90, align: 'center' as const },
    { title: '入库条数', dataIndex: 'in_item_count', width: 90, align: 'center' as const },
    { title: '出库金额', dataIndex: 'out_amount', width: 110, align: 'right' as const, render: (v: number) => <span style={{ color: RED }}>{fmt2(v)}</span> },
    { title: '下浮后出库金额', dataIndex: 'discounted_out_amount', width: 140, align: 'right' as const, render: (v: number) => <span style={{ color: RED }}>{fmt2(v)}</span> },
    { title: '出库单数', dataIndex: 'out_order_count', width: 90, align: 'center' as const },
    { title: '出库条数', dataIndex: 'out_item_count', width: 90, align: 'center' as const },
    {
      title: '毛利',
      dataIndex: 'gross_profit',
      width: 110,
      align: 'right' as const,
      render: (v: number) => <span style={{ color: (v || 0) >= 0 ? GREEN : RED }}>{fmt2(v)}</span>,
    },
    {
      title: '毛利率',
      key: 'margin',
      width: 100,
      align: 'right' as const,
      render: (_v, o) => {
        const dOut = o.discounted_out_amount || 0
        // 毛利率 = 毛利润 / 下浮后出库金额
        const rate = dOut !== 0 ? ((o.gross_profit || 0) / dOut) * 100 : 0
        return <span style={{ color: rate >= 0 ? GREEN : RED }}>{fmt2(rate)}%</span>
      },
    },
  ]

  // 4 个汇总卡片（照老页面：合计入库/合计出库/下浮后合计出库/合计毛利）
  const totalProfit = data?.total_gross_profit ?? 0
  const summaryTiles = [
    { label: '合计入库金额', value: data?.total_in_amount ?? 0, color: GREEN },
    { label: '合计出库金额', value: data?.total_out_amount ?? 0, color: RED },
    { label: '下浮后合计出库金额', value: data?.total_discounted_out_amount ?? 0, color: RED },
    { label: '合计毛利', value: totalProfit, color: totalProfit >= 0 ? GREEN : RED },
  ]

  return (
    <ReportCard
      title={title}
      desc={desc}
      filters={
        <>
          <FilterItem label="开始日期">
            <DatePicker
              value={startDate ? dayjs(startDate) : null}
              onChange={(d) => setStartDate(d ? d.format('YYYY-MM-DD') : '')}
            />
          </FilterItem>
          <FilterItem label="结束日期">
            <DatePicker
              value={endDate ? dayjs(endDate) : null}
              onChange={(d) => setEndDate(d ? d.format('YYYY-MM-DD') : '')}
            />
          </FilterItem>
        </>
      }
      actions={
        <>
          <Button type="primary" icon={<SearchOutlined />} onClick={search}>
            查询
          </Button>
          <Button
            icon={<DownloadOutlined />}
            onClick={exportExcel}
            style={{ background: '#52c41a', borderColor: '#52c41a', color: '#fff' }}
          >
            导出Excel
          </Button>
          <Button onClick={() => resetDate('today')}>今日</Button>
          <Button onClick={() => resetDate('7d')}>近7天</Button>
          <Button onClick={() => resetDate('30d')}>近30天</Button>
          <Button onClick={() => resetDate('month')}>本月</Button>
          <Button onClick={() => resetDate('all')}>全部</Button>
        </>
      }
    >
      <Row gutter={16} style={{ marginBottom: 16 }}>
        {summaryTiles.map((t) => (
          <Col span={6} key={t.label}>
            <div style={{ background: '#fafafa', borderRadius: 8, textAlign: 'center', padding: '8px 0' }}>
              <div style={{ color: '#999', fontSize: 12 }}>{t.label}</div>
              <div style={{ fontSize: 22, fontWeight: 600, color: t.color }}>{fmt2(t.value)}</div>
            </div>
          </Col>
        ))}
      </Row>
      <Table<StockSummaryRow>
        rowKey={(_, i) => String(i)}
        size="small"
        loading={isLoading}
        columns={columns}
        dataSource={rows}
        pagination={false}
        locale={{ emptyText: '暂无数据' }}
        // 合计行：加粗 + 浅黄底（照老页面 #fff8e1）
        onRow={(r) => (r.is_summary ? { style: { fontWeight: 600, background: '#fff8e1' } } : {})}
        scroll={{ x: 1200 }}
      />
    </ReportCard>
  )
}

export default function StockSummaryPage() {
  return (
    <StockSummaryView
      title="真实出入库统计"
      desc="真实账套口径：按日汇总采购入库金额与销售出库金额，下浮后出库金额 = 出库金额 × (1 - 销售订单下浮率/100)，毛利 = 下浮后出库金额 - 入库金额。"
      kind="real"
    />
  )
}
