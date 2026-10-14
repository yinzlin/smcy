import { useState } from 'react'
import type { CSSProperties } from 'react'
import { Button, DatePicker, Table } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import {
  getSalesSummary,
  SALES_SUMMARY_EXPORT_URL,
  type SalesSummaryPurchaserItem,
  type SalesSummaryProductItem,
} from '../../api/query/summaries'
import { fmt2, fmtMoney } from '../../utils/format'

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

/** 老页面毛利率口径：sales_amount > 0 时 (毛利 / 销售额 * 100).toFixed(1)，否则 '0'，再拼 '%' */
function marginRate(sales_amount: number, cost_amount: number): string {
  const margin = sales_amount - cost_amount
  return (sales_amount > 0 ? ((margin / sales_amount) * 100).toFixed(1) : '0') + '%'
}

export default function SalesSummaryPage() {
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  // 老页面加载即自动查询（空日期也发起请求）
  const [params, setParams] = useState({ start_date: '', end_date: '' })

  const { data, isFetching } = useQuery({
    queryKey: ['salesSummary', params],
    queryFn: () => getSalesSummary(params.start_date, params.end_date),
  })

  const search = () => setParams({ start_date: startDate, end_date: endDate })

  const purchasers = data?.by_purchaser ?? []
  const products = data?.by_product ?? []
  const pTotalQty = purchasers.reduce((s, r) => s + r.quantity, 0)
  const pTotalAmt = purchasers.reduce((s, r) => s + r.sales_amount, 0)
  const pTotalMargin = purchasers.reduce((s, r) => s + (r.sales_amount - r.cost_amount), 0)
  // 老页面合计毛利率：totalAmt > 0 时 (totalMargin / totalAmt * 100).toFixed(1)，否则 '0'
  const pTotalRate = pTotalAmt > 0 ? ((pTotalMargin / pTotalAmt) * 100).toFixed(1) : '0'
  const dTotalQty = products.reduce((s, r) => s + r.quantity, 0)
  const dTotalAmt = products.reduce((s, r) => s + r.sales_amount, 0)
  const dTotalMargin = products.reduce((s, r) => s + (r.sales_amount - r.cost_amount), 0)

  const purchaserColumns: TableProps<SalesSummaryPurchaserItem>['columns'] = [
    { title: '采购单位', dataIndex: 'name' },
    { title: '销售数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '销售金额', dataIndex: 'sales_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    {
      title: '毛利',
      key: 'margin',
      align: 'right',
      render: (_v, o) => fmtMoney(o.sales_amount - o.cost_amount),
    },
    {
      title: '毛利率',
      key: 'margin_rate',
      align: 'right',
      render: (_v, o) => <span>{marginRate(o.sales_amount, o.cost_amount)}</span>,
    },
  ]

  const productColumns: TableProps<SalesSummaryProductItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '-' },
    { title: '销售数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '销售金额', dataIndex: 'sales_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    {
      title: '毛利',
      key: 'margin',
      align: 'right',
      render: (_v, o) => fmtMoney(o.sales_amount - o.cost_amount),
    },
  ]

  return (
    <ReportCard
      title="销售汇总报表"
      filters={
        <>
          <DateFilter label="开始日期" value={startDate} onChange={setStartDate} />
          <DateFilter label="结束日期" value={endDate} onChange={setEndDate} />
        </>
      }
      actions={
        <>
          <Button type="primary" onClick={search}>
            查询
          </Button>
          <Button href={SALES_SUMMARY_EXPORT_URL}>导出Excel</Button>
        </>
      }
    >
      <h4 style={SECTION_TITLE}>按采购单位汇总</h4>
      <Table<SalesSummaryPurchaserItem>
        rowKey={(_, i) => String(i)}
        bordered
        size="small"
        loading={isFetching}
        columns={purchaserColumns}
        dataSource={purchasers}
        pagination={false}
        summary={() =>
          purchasers.length > 0 ? (
            <Table.Summary>
              <Table.Summary.Row>
                <Table.Summary.Cell index={0}>
                  <b>合计</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={1}>
                  <b>{fmt2(pTotalQty)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={2}>
                  <b>{fmtMoney(pTotalAmt)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={3}>
                  <b>{fmtMoney(pTotalMargin)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={4}>
                  <b>{pTotalRate}%</b>
                </Table.Summary.Cell>
              </Table.Summary.Row>
            </Table.Summary>
          ) : null
        }
      />

      <h4 style={SECTION_TITLE_GAP}>按商品汇总</h4>
      <Table<SalesSummaryProductItem>
        rowKey={(_, i) => String(i)}
        bordered
        size="small"
        loading={isFetching}
        columns={productColumns}
        dataSource={products}
        pagination={false}
        summary={() =>
          products.length > 0 ? (
            <Table.Summary>
              <Table.Summary.Row>
                <Table.Summary.Cell index={0} colSpan={2}>
                  <b>合计</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={2}>
                  <b>{fmt2(dTotalQty)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={3}>
                  <b>{fmtMoney(dTotalAmt)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={4}>
                  <b>{fmtMoney(dTotalMargin)}</b>
                </Table.Summary.Cell>
              </Table.Summary.Row>
            </Table.Summary>
          ) : null
        }
      />
    </ReportCard>
  )
}
