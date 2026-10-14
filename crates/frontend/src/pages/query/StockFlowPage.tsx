import { useMemo, useState } from 'react'
import { Button, DatePicker, Input, Radio, Table } from 'antd'
import { DownloadOutlined, SearchOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import dayjs from 'dayjs'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import {
  queryStockFlow,
  stockFlowExportUrl,
  type StockFlowItem,
  type StockFlowView,
} from '../../api/query/stock'

const PAGE_SIZE = 200
/** 老页面表格共 12 列（分户标题行整行合并） */
const FLOW_COLS = 12

type FlowRow =
  | { kind: 'group'; key: string; product_name: string; spec: string; unit: string }
  | { kind: 'flow'; key: string; item: StockFlowItem }

/** 0/空留白（照老页面 sfQty） */
function sfQty(v: number | null | undefined): string {
  const n = Number(v || 0)
  return n === 0 ? '' : n.toFixed(2)
}

export default function StockFlowPage() {
  const [productName, setProductName] = useState('')
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [view, setView] = useState<StockFlowView>('timeline')
  const [submitted, setSubmitted] = useState<{
    productName: string
    startDate: string
    endDate: string
    view: StockFlowView
  } | null>(null)
  const [page, setPage] = useState(1)

  const { data, isLoading } = useQuery({
    queryKey: ['stockFlow', submitted, page],
    queryFn: () => queryStockFlow({ ...submitted!, page, pageSize: PAGE_SIZE }),
    enabled: submitted !== null,
    placeholderData: (prev) => prev,
  })

  const search = () => {
    setSubmitted({ productName, startDate, endDate, view })
    setPage(1)
  }

  // 切换视图：已查询过则按新视图重查第一页（照老页面 switchStockView）
  const switchView = (v: StockFlowView) => {
    setView(v)
    if (submitted !== null) {
      setSubmitted({ ...submitted, view: v })
      setPage(1)
    }
  }

  const exportFlow = (format: 'ledger' | 'timeline') => {
    window.location.href = stockFlowExportUrl({ productName, startDate, endDate, format })
  }

  // 分户视图：商品切换时插入分户标题行（照老页面拼行逻辑）
  const rows = useMemo<FlowRow[]>(() => {
    const out: FlowRow[] = []
    let lastPid: number | null = null
    ;(data?.items ?? []).forEach((item, idx) => {
      if ((submitted?.view ?? 'timeline') === 'group' && lastPid !== item.product_id) {
        out.push({
          kind: 'group',
          key: `g-${idx}`,
          product_name: item.product_name,
          spec: item.spec || '',
          unit: item.unit || '',
        })
        lastPid = item.product_id
      }
      out.push({ kind: 'flow', key: `r-${idx}`, item })
    })
    return out
  }, [data, submitted])

  // 分户标题行整行合并：首列 colSpan=12，其余列 colSpan=0（照老页面 colspan 写法）
  const groupHidden = { props: { colSpan: 0 }, children: null }
  const groupView = submitted?.view === 'group'

  const columns: TableProps<FlowRow>['columns'] = [
    {
      title: '日期',
      key: 'date',
      width: 150,
      render: (_v, r) => {
        if (r.kind === 'group') {
          return {
            props: { colSpan: FLOW_COLS },
            children: (
              <span>
                <strong>{r.product_name}</strong>
                {r.spec ? '　' + r.spec : ''}
                {'　'}
                <span style={{ color: '#999' }}>单位：{r.unit}</span>
              </span>
            ),
          }
        }
        return r.item.create_time
      },
    },
    {
      title: '摘要',
      key: 'summary',
      render: (_v, r) => (r.kind === 'group' ? groupHidden : r.item.summary),
    },
    {
      title: '单据号',
      key: 'ref_no',
      width: 130,
      render: (_v, r) =>
        r.kind === 'group' ? groupHidden : <small>{r.item.ref_no || ''}</small>,
    },
    {
      title: '商品名称',
      key: 'product_name',
      width: 160,
      render: (_v, r) => {
        if (r.kind === 'group') return groupHidden
        // 分户视图下商品名称列显示占位破折号（照老页面）
        return groupView ? <span style={{ color: '#999' }}>—</span> : r.item.product_name
      },
    },
    { title: '规格', key: 'spec', width: 110, render: (_v, r) => (r.kind === 'group' ? groupHidden : r.item.spec || '') },
    { title: '单位', key: 'unit', width: 70, render: (_v, r) => (r.kind === 'group' ? groupHidden : r.item.unit || '') },
    {
      title: '入库数量',
      key: 'in_quantity',
      width: 90,
      align: 'right' as const,
      render: (_v, r) => {
        if (r.kind === 'group') return groupHidden
        const v = sfQty(r.item.in_quantity)
        // 入库绿色加粗（照老页面 text-success font-weight-bold）
        return v === '' ? '' : <span style={{ color: '#389e0d', fontWeight: 600 }}>{v}</span>
      },
    },
    {
      title: '出库数量',
      key: 'out_quantity',
      width: 90,
      align: 'right' as const,
      render: (_v, r) => (r.kind === 'group' ? groupHidden : sfQty(r.item.out_quantity)),
    },
    {
      title: '结存',
      key: 'balance',
      width: 90,
      align: 'right' as const,
      render: (_v, r) => {
        if (r.kind === 'group') return groupHidden
        const v = sfQty(r.item.balance)
        if (v === '') return ''
        // 负结存红字加粗（照老页面）
        return (r.item.balance || 0) < 0 ? (
          <span style={{ color: '#cf1322', fontWeight: 600 }}>{v}</span>
        ) : (
          v
        )
      },
    },
    {
      title: '来源/去向',
      key: 'party_name',
      width: 140,
      render: (_v, r) => (r.kind === 'group' ? groupHidden : r.item.party_name || ''),
    },
    {
      title: '生产日期/批号',
      key: 'batch',
      width: 140,
      render: (_v, r) => {
        if (r.kind === 'group') return groupHidden
        return <small>{[r.item.production_date, r.item.batch_no].filter(Boolean).join(' / ')}</small>
      },
    },
    {
      title: '备注',
      key: 'remark',
      width: 110,
      render: (_v, r) => {
        if (r.kind === 'group') return groupHidden
        // 仅大单位（与基础单位不同）出入库时标注原始数量，如"原2件"（照老页面）
        const it = r.item
        if (it.orig_unit && it.orig_unit !== it.unit && it.orig_quantity) {
          const q = Math.round(it.orig_quantity * 100) / 100
          return (
            <small>
              原{q}
              {it.orig_unit}
            </small>
          )
        }
        return ''
      },
    },
  ]

  return (
    <ReportCard
      title="出入库记录"
      filters={
        <>
          <FilterItem label="商品名称" width={220}>
            <Input
              value={productName}
              placeholder="输入商品名称搜索"
              onChange={(e) => setProductName(e.target.value)}
              onPressEnter={search}
            />
          </FilterItem>
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
          <FilterItem label="视图">
            <Radio.Group
              value={view}
              onChange={(e) => switchView(e.target.value as StockFlowView)}
              optionType="button"
              options={[
                { label: '时间线', value: 'timeline' },
                { label: '按商品分户', value: 'group' },
              ]}
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
            onClick={() => exportFlow('ledger')}
            style={{ background: '#52c41a', borderColor: '#52c41a', color: '#fff' }}
          >
            导出保管账
          </Button>
          <Button onClick={() => exportFlow('timeline')}>按时间线补打</Button>
        </>
      }
      desc="保管账=一品一户成册；时间线补打=新期间连续紧凑页，结存自动承接前期余额，续订在旧账本后。"
    >
      <Table<FlowRow>
        rowKey="key"
        size="small"
        loading={isLoading}
        columns={columns}
        dataSource={rows}
        scroll={{ x: 1300 }}
        locale={{ emptyText: submitted ? '该条件下没有出入库记录' : '请设置查询条件后点击"查询"' }}
        onRow={(r) => (r.kind === 'group' ? { style: { background: '#fafafa' } } : {})}
        pagination={{
          current: page,
          pageSize: PAGE_SIZE,
          total: data?.total ?? 0,
          showSizeChanger: false,
          showTotal: (t) => `共 ${t} 条，第 ${page} / ${Math.max(1, Math.ceil(t / PAGE_SIZE))} 页`,
          onChange: (p) => setPage(p),
        }}
      />
    </ReportCard>
  )
}
