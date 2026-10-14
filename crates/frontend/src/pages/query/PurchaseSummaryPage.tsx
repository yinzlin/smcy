import { useState } from 'react'
import type { CSSProperties } from 'react'
import { Button, DatePicker, Table } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import {
  getPurchaseSummary,
  type PurchaseSummaryProductItem,
  type PurchaseSummarySupplierItem,
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

/** 老页面口径：quantity > 0 时取 amount / quantity，否则 '0.00'（照抄） */
function avgAmount(amount: number, quantity: number): string {
  return '¥' + (quantity > 0 ? (amount / quantity).toFixed(2) : '0.00')
}

export default function PurchaseSummaryPage() {
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  // 老页面不自动查询：点击「查询」才发起
  const [submitted, setSubmitted] = useState(false)
  const [params, setParams] = useState({ start_date: '', end_date: '' })

  const { data, isFetching } = useQuery({
    queryKey: ['purchaseSummary', params],
    queryFn: () => getPurchaseSummary(params.start_date, params.end_date),
    enabled: submitted,
  })

  const search = () => {
    setParams({ start_date: startDate, end_date: endDate })
    setSubmitted(true)
  }

  const suppliers = data?.by_supplier ?? []
  const products = data?.by_product ?? []
  const supplierTotalQty = suppliers.reduce((s, r) => s + r.quantity, 0)
  const supplierTotalAmt = suppliers.reduce((s, r) => s + r.amount, 0)
  const productTotalQty = products.reduce((s, r) => s + r.quantity, 0)
  const productTotalAmt = products.reduce((s, r) => s + r.amount, 0)

  const supplierColumns: TableProps<PurchaseSummarySupplierItem>['columns'] = [
    { title: '供应商', dataIndex: 'name' },
    { title: '采购数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '采购金额', dataIndex: 'amount', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '平均成本', key: 'avg_cost', align: 'right', render: (_v, o) => avgAmount(o.amount, o.quantity) },
  ]

  const productColumns: TableProps<PurchaseSummaryProductItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '-' },
    { title: '采购数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '采购金额', dataIndex: 'amount', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '平均单价', key: 'avg_price', align: 'right', render: (_v, o) => avgAmount(o.amount, o.quantity) },
  ]

  return (
    <ReportCard
      title="采购汇总统计"
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
      <h4 style={SECTION_TITLE}>按供应商汇总</h4>
      <Table<PurchaseSummarySupplierItem>
        rowKey={(_, i) => String(i)}
        bordered
        size="small"
        loading={isFetching}
        columns={supplierColumns}
        dataSource={suppliers}
        pagination={false}
        summary={() =>
          suppliers.length > 0 ? (
            <Table.Summary>
              <Table.Summary.Row>
                <Table.Summary.Cell index={0}>
                  <b>合计</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={1}>
                  <b>{fmt2(supplierTotalQty)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={2}>
                  <b>{fmtMoney(supplierTotalAmt)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={3} />
              </Table.Summary.Row>
            </Table.Summary>
          ) : null
        }
      />

      <h4 style={SECTION_TITLE_GAP}>按商品汇总</h4>
      <Table<PurchaseSummaryProductItem>
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
                  <b>{fmt2(productTotalQty)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={3}>
                  <b>{fmtMoney(productTotalAmt)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={4} />
              </Table.Summary.Row>
            </Table.Summary>
          ) : null
        }
      />
    </ReportCard>
  )
}
