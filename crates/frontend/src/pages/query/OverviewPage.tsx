import { useState } from 'react'
import { Button, DatePicker, Table } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { fmt2, fmtMoney } from '../../utils/format'
import { fetchOverview, type OverviewSummaryRow } from '../../api/query/finance'

/** Bootstrap 色板：bg-success / bg-primary / bg-warning / bg-info */
const CARD_COLORS = { success: '#198754', primary: '#0d6efd', warning: '#ffc107', info: '#0dcaf0' }

function StatCard({ title, value, color }: { title: string; value: number | undefined; color: string }) {
  return (
    <div style={{ flex: '1 1 240px', background: color, color: '#fff', borderRadius: 8, padding: '16px 24px' }}>
      <div style={{ fontSize: 16, fontWeight: 600 }}>{title}</div>
      <div style={{ fontSize: 26, fontWeight: 600 }}>{fmtMoney(value)}</div>
    </div>
  )
}

export default function OverviewPage() {
  const [month, setMonth] = useState('')
  // 老页面加载即以空月份自动查询，点「查询」才应用新月份
  const [appliedMonth, setAppliedMonth] = useState('')

  const { data, isFetching } = useQuery({
    queryKey: ['overview', appliedMonth],
    queryFn: () => fetchOverview(appliedMonth),
  })

  const search = () => setAppliedMonth(month)

  return (
    <ReportCard
      title="进销存汇总报表"
      filters={
        <FilterItem label="月份" width={220}>
          <DatePicker picker="month" style={{ width: '100%' }} value={month ? dayjs(month) : null} onChange={(d) => setMonth(d ? d.format('YYYY-MM') : '')} />
        </FilterItem>
      }
      actions={
        <Button type="primary" onClick={search} loading={isFetching}>查询</Button>
      }
    >
      {/* 汇总卡片：总进货（绿）/ 总销售（蓝）/ 库存（黄）/ 本期毛利（青），初始 ¥0.00 */}
      <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap', marginBottom: 16 }}>
        <StatCard title="总进货金额" value={data?.purchase_total} color={CARD_COLORS.success} />
        <StatCard title="总销售金额" value={data?.sales_total} color={CARD_COLORS.primary} />
        <StatCard title="库存金额" value={data?.stock_total} color={CARD_COLORS.warning} />
        <StatCard title="本期毛利" value={data?.profit_total} color={CARD_COLORS.info} />
      </div>

      {/* 采购汇总 */}
      <div style={{ fontSize: 15, fontWeight: 600, margin: '8px 0' }}>采购汇总</div>
      <Table<OverviewSummaryRow>
        rowKey="name"
        size="small"
        bordered
        loading={isFetching}
        columns={[
          { title: '供应商', dataIndex: 'name' },
          { title: '采购金额', dataIndex: 'amount', align: 'right', render: (v: number) => fmt2(v) },
          { title: '采购数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
        ]}
        dataSource={data?.purchase_by_supplier ?? []}
        pagination={false}
      />

      {/* 销售汇总 */}
      <div style={{ fontSize: 15, fontWeight: 600, margin: '16px 0 8px' }}>销售汇总</div>
      <Table<OverviewSummaryRow>
        rowKey="name"
        size="small"
        bordered
        loading={isFetching}
        columns={[
          { title: '采购单位', dataIndex: 'name' },
          { title: '销售金额', dataIndex: 'amount', align: 'right', render: (v: number) => fmt2(v) },
          { title: '销售数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
        ]}
        dataSource={data?.sales_by_purchaser ?? []}
        pagination={false}
      />
    </ReportCard>
  )
}
