import { useState } from 'react'
import type { CSSProperties } from 'react'
import { Button, DatePicker, Table } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import {
  getProductRank,
  type ProductRankSlowItem,
  type ProductRankTopItem,
} from '../../api/query/summaries'
import { fmt2, fmtMoney } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'

const SECTION_TITLE: CSSProperties = { fontSize: 15, fontWeight: 600, margin: '0 0 10px' }
const SECTION_TITLE_GAP: CSSProperties = { fontSize: 15, fontWeight: 600, margin: '20px 0 10px' }

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

export default function ProductRankPage() {
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  // 老页面加载即自动查询（空日期也发起请求）
  const [params, setParams] = useState({ start_date: '', end_date: '' })

  const { data, isFetching } = useQuery({
    queryKey: ['productRank', params],
    queryFn: () => getProductRank(params.start_date, params.end_date),
  })

  const search = () => setParams({ start_date: startDate, end_date: endDate })

  const topSelling = data?.top_selling ?? []
  const slowMoving = data?.slow_moving ?? []
  const topTotalQty = topSelling.reduce((s, r) => s + r.quantity, 0)
  const topTotalAmt = topSelling.reduce((s, r) => s + r.amount, 0)

  const topColumns: TableProps<ProductRankTopItem>['columns'] = [
    { title: '排名', key: 'rank', width: 60, render: (_v, _o, idx) => idx + 1 },
    { title: '商品名称', dataIndex: 'product_name' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '-' },
    { title: '销售数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '销售金额', dataIndex: 'amount', align: 'right', render: (v: number) => fmtMoney(v) },
  ]

  const slowColumns: TableProps<ProductRankSlowItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '-' },
    { title: '当前库存', dataIndex: 'stock_quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '最后销售日期', dataIndex: 'last_sale_date', render: (v: string | null) => v || '从未销售' },
  ]

  return (
    <ReportCard
      title="畅销滞销商品查询"
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
      <h4 style={SECTION_TITLE}>畅销商品 TOP 10</h4>
      <Table<ProductRankTopItem>
        rowKey={(_, i) => String(i)}
        bordered
        size="small"
        loading={isFetching}
        columns={topColumns}
        dataSource={topSelling}
        scroll={scrollX('max-content')}
        pagination={false}
        summary={() =>
          topSelling.length > 0 ? (
            <Table.Summary>
              <Table.Summary.Row>
                <Table.Summary.Cell index={0} colSpan={3}>
                  <b>合计</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={3}>
                  <b>{fmt2(topTotalQty)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={4}>
                  <b>{fmtMoney(topTotalAmt)}</b>
                </Table.Summary.Cell>
              </Table.Summary.Row>
            </Table.Summary>
          ) : null
        }
      />

      <h4 style={SECTION_TITLE_GAP}>滞销商品（期间无销售）</h4>
      {/* 老页面滞销表无合计行 */}
      <Table<ProductRankSlowItem>
        rowKey={(_, i) => String(i)}
        bordered
        size="small"
        loading={isFetching}
        columns={slowColumns}
        dataSource={slowMoving}
        scroll={scrollX('max-content')}
        pagination={false}
      />
    </ReportCard>
  )
}
