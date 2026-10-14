import { useState } from 'react'
import type { CSSProperties } from 'react'
import { Button, DatePicker, Table } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import { getAllocationSource, type AllocationSourceItem } from '../../api/query/summaries'
import { fmtMoney } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'

const SECTION_TITLE: CSSProperties = { fontSize: 15, fontWeight: 600, margin: '0 0 10px' }

/** 日期筛选：产出 YYYY-MM-DD 字符串（本版本 antd 无 valueFormat，用 dayjs format 等效实现） */
function DateFilter({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <FilterItem label={label} width={200}>
      <DatePicker
        style={{ width: '100%' }}
        value={value ? dayjs(value) : null}
        onChange={(d) => onChange(d ? d.format('YYYY-MM-DD') : '')}
      />
    </FilterItem>
  )
}

/** 老页面状态映射照抄 */
const STATUS_MAP: Record<number, string> = { 0: '未分摊', 1: '分摊中', 2: '已完成', 3: '已终止' }

/** 老页面去向拼接：order_no(¥amount)、...，空则 '-'（照抄） */
function joinTargets(targets: AllocationSourceItem['targets']): string {
  return (
    (targets ?? [])
      .map((t) => `${t.order_no}(¥${t.amount.toFixed(2)})`)
      .join('、') || '-'
  )
}

export default function AllocationSourcePage() {
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  // 老页面加载即自动查询（空日期也发起请求）
  const [params, setParams] = useState({ start_date: '', end_date: '' })

  const { data, isFetching } = useQuery({
    queryKey: ['allocationSource', params],
    queryFn: () => getAllocationSource(params.start_date, params.end_date),
  })

  const search = () => setParams({ start_date: startDate, end_date: endDate })

  const rows = data ?? []
  const totalAmount = rows.reduce((s, r) => s + r.total_amount, 0)
  const totalAllocated = rows.reduce((s, r) => s + r.allocated_amount, 0)
  const totalRemain = rows.reduce((s, r) => s + r.remaining_balance, 0)

  const columns: TableProps<AllocationSourceItem>['columns'] = [
    { title: '来源订单', dataIndex: 'order_no' },
    { title: '日期', dataIndex: 'order_date' },
    { title: '来源金额', dataIndex: 'total_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '已分摊', dataIndex: 'allocated_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '剩余', dataIndex: 'remaining_balance', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '状态', dataIndex: 'status', render: (v: number) => STATUS_MAP[v] ?? '未知' },
    {
      title: '分摊去向',
      key: 'targets',
      // 老页面该列为 class="small"
      render: (_v, o) => <span style={{ fontSize: 12 }}>{joinTargets(o.targets)}</span>,
    },
  ]

  return (
    <ReportCard
      title="分摊来源统计"
      desc="统计所有作为分摊来源的订单（如耗材单），显示其金额、已分摊金额、剩余、状态及分摊去向，供来源侧单独核账。"
      filters={
        <>
          <DateFilter label="开始日期" value={startDate} onChange={setStartDate} />
          <DateFilter label="结束日期" value={endDate} onChange={setEndDate} />
        </>
      }
      actions={
        <Button type="primary" onClick={search}>
          查询
        </Button>
      }
    >
      <h4 style={SECTION_TITLE}>分摊来源单汇总</h4>
      <Table<AllocationSourceItem>
        rowKey="order_no"
        bordered
        size="small"
        loading={isFetching}
        columns={columns}
        dataSource={rows}
        scroll={scrollX('max-content')}
        pagination={false}
        summary={() =>
          rows.length > 0 ? (
            <Table.Summary>
              <Table.Summary.Row>
                <Table.Summary.Cell index={0} colSpan={2}>
                  <b>合计</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={2}>
                  <b>{fmtMoney(totalAmount)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={3}>
                  <b>{fmtMoney(totalAllocated)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={4}>
                  <b>{fmtMoney(totalRemain)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={5} colSpan={2} />
              </Table.Summary.Row>
            </Table.Summary>
          ) : null
        }
      />
    </ReportCard>
  )
}
