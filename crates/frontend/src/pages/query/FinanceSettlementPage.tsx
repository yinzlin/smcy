import { useState } from 'react'
import { Button, DatePicker, Input, Select, Table, Tag } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmtMoney } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'
import {
  fetchFinanceSettlement,
  type FinanceSettlementQuery,
  type FinanceSettlementRow,
  type FinanceSettlementSummary,
} from '../../api/query/finance'

const PAGE_SIZE = 20

/** 老页面 5 张汇总卡：灰底居中，label 小灰字 + 彩色金额（text-primary/warning/danger/success） */
const SUMMARY_CARDS: { label: keyof FinanceSettlementSummary; text: string; color: string }[] = [
  { label: 'total_amount', text: '应付合计', color: '#0d6efd' },
  { label: 'discount_amount', text: '下浮合计', color: '#ffc107' },
  { label: 'reduction_amount', text: '扣减合计', color: '#dc3545' },
  { label: 'paid_amount', text: '已付合计', color: '#198754' },
  { label: 'unpaid_amount', text: '未付合计', color: '#dc3545' },
]

export default function FinanceSettlementPage() {
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [type, setType] = useState('')
  const [isSettled, setIsSettled] = useState('')
  const [keywordInput, setKeywordInput] = useState('')
  // 老页面：关键字输入框仅在回车或点「查询」时生效，其余控件 change 即查
  const [appliedKeyword, setAppliedKeyword] = useState('')
  const [page, setPage] = useState(1)

  const query: FinanceSettlementQuery = {
    start_date: startDate,
    end_date: endDate,
    type,
    is_settled: isSettled,
    keyword: appliedKeyword,
    page,
    page_size: PAGE_SIZE,
  }

  // 老页面加载即自动查询
  const { data, isFetching } = useQuery({
    queryKey: ['financeSettlement', query],
    queryFn: () => fetchFinanceSettlement(query),
  })

  // 改筛选即重查并回到第 1 页（对应老页面 onchange="searchFinance()"）
  const changeFilter = (setter: (v: string) => void) => (v: string) => {
    setter(v)
    setPage(1)
  }
  // 回车 / 查询按钮：应用关键字并回到第 1 页
  const search = () => {
    setAppliedKeyword(keywordInput.trim())
    setPage(1)
  }

  const columns: TableProps<FinanceSettlementRow>['columns'] = [
    { title: '订单号', dataIndex: 'order_no' },
    { title: '日期', dataIndex: 'order_date' },
    {
      title: '类型',
      dataIndex: 'order_type',
      render: (v: FinanceSettlementRow['order_type']) =>
        v === 'purchase' ? <Tag color="cyan">采购</Tag> : <Tag color="blue">销售</Tag>,
    },
    { title: '对方名称', dataIndex: 'party_name' },
    { title: '应付金额', dataIndex: 'total_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '下浮', dataIndex: 'discount_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '扣减', dataIndex: 'amount_reduction', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '最终金额', dataIndex: 'final_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '已付金额', dataIndex: 'paid_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '未付金额', dataIndex: 'unpaid_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    {
      title: '结算状态',
      dataIndex: 'is_settled',
      render: (v: number) => (v === 1 ? <Tag color="success">已结</Tag> : <Tag color="warning">未结</Tag>),
    },
  ]

  return (
    <>
      <ReportCard
        title="财务结算查询"
        filters={
          <>
            <FilterItem label="开始日期" width={200}>
              <DatePicker style={{ width: '100%' }} value={startDate ? dayjs(startDate) : null} onChange={(d) => changeFilter(setStartDate)(d ? d.format('YYYY-MM-DD') : '')} />
            </FilterItem>
            <FilterItem label="结束日期" width={200}>
              <DatePicker style={{ width: '100%' }} value={endDate ? dayjs(endDate) : null} onChange={(d) => changeFilter(setEndDate)(d ? d.format('YYYY-MM-DD') : '')} />
            </FilterItem>
            <FilterItem label="类型" width={160}>
              <Select
                style={{ width: '100%' }}
                value={type}
                onChange={changeFilter(setType)}
                options={[
                  { value: '', label: '全部' },
                  { value: 'purchase', label: '采购' },
                  { value: 'sales', label: '销售' },
                ]}
              />
            </FilterItem>
            <FilterItem label="结算状态" width={160}>
              <Select
                style={{ width: '100%' }}
                value={isSettled}
                onChange={changeFilter(setIsSettled)}
                options={[
                  { value: '', label: '全部' },
                  { value: '0', label: '未结' },
                  { value: '1', label: '已结' },
                ]}
              />
            </FilterItem>
            <FilterItem label="关键字（订单号/对方名称）" width={240}>
              <Input placeholder="搜索..." value={keywordInput} onPressEnter={search} onChange={(e) => setKeywordInput(e.target.value)} />
            </FilterItem>
          </>
        }
        actions={
          <Button type="primary" onClick={search} loading={isFetching}>查询</Button>
        }
      >
        {/* 汇总卡片行：老页面 5 张灰底卡片（应付/下浮/扣减/已付/未付合计） */}
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 12 }}>
          {SUMMARY_CARDS.map((c) => (
            <div
              key={c.label}
              style={{
                flex: '1 1 180px',
                background: '#f8f9fa',
                border: '1px solid #eee',
                borderRadius: 8,
                padding: 12,
                textAlign: 'center',
              }}
            >
              <div style={{ fontSize: 13, color: '#6c757d' }}>{c.text}</div>
              <div style={{ fontSize: 18, fontWeight: 600, color: c.color }}>{fmtMoney(data?.summary?.[c.label])}</div>
            </div>
          ))}
        </div>

        <Table<FinanceSettlementRow>
          rowKey={(o) => `${o.order_type}-${o.order_no}`}
          size="small"
          bordered
          loading={isFetching}
          columns={columns}
          dataSource={data?.data ?? []}
          scroll={scrollX('max-content')}
          pagination={{
            current: page,
            pageSize: PAGE_SIZE,
            total: data?.total ?? 0,
            showSizeChanger: false,
            showTotal: (t) => `共 ${t} 条，${Math.max(1, Math.ceil(t / PAGE_SIZE))} 页`,
            onChange: (p) => setPage(p),
          }}
        />
      </ReportCard>
    </>
  )
}
