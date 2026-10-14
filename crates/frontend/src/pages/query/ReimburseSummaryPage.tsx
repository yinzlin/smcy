import { useState } from 'react'
import type { CSSProperties } from 'react'
import { Button, DatePicker, Table } from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery } from '@tanstack/react-query'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'
import {
  getReimburseSummary,
  REIMBURSE_SUMMARY_EXPORT_URL,
  type ReimburseSummaryPurchaserItem,
  type ReimburseSummaryProductItem,
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

export default function ReimburseSummaryPage() {
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  // 老页面加载即自动查询（空日期也发起请求）
  const [params, setParams] = useState({ start_date: '', end_date: '' })

  const { data, isFetching } = useQuery({
    queryKey: ['reimburseSummary', params],
    queryFn: () => getReimburseSummary(params.start_date, params.end_date),
  })

  const search = () => setParams({ start_date: startDate, end_date: endDate })

  const purchasers = data?.by_purchaser ?? []
  const products = data?.by_product ?? []
  const pTotalReal = purchasers.reduce((s, r) => s + r.real_amount, 0)
  const pTotalSupp = purchasers.reduce((s, r) => s + r.supplement_amount, 0)
  const pTotalReim = purchasers.reduce((s, r) => s + r.reimburse_amount, 0)
  const dTotalQty = products.reduce((s, r) => s + r.quantity, 0)
  const dTotalAmt = products.reduce((s, r) => s + r.reimburse_amount, 0)

  const purchaserColumns: TableProps<ReimburseSummaryPurchaserItem>['columns'] = [
    { title: '采购单位', dataIndex: 'name' },
    { title: '真实金额', dataIndex: 'real_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '分摊增项净额', dataIndex: 'supplement_amount', align: 'right', render: (v: number) => fmtMoney(v) },
    // 老页面数据行报销金额加粗（<strong>）
    { title: '报销金额', dataIndex: 'reimburse_amount', align: 'right', render: (v: number) => <b>{fmtMoney(v)}</b> },
  ]

  const productColumns: TableProps<ReimburseSummaryProductItem>['columns'] = [
    { title: '商品名称', dataIndex: 'product_name' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '-' },
    { title: '数量', dataIndex: 'quantity', align: 'right', render: (v: number) => fmt2(v) },
    { title: '报销金额', dataIndex: 'reimburse_amount', align: 'right', render: (v: number) => fmtMoney(v) },
  ]

  return (
    <ReportCard
      title="报销口径汇总"
      desc="口径说明：仅统计分摊后的目标单（真实明细 + 分摊增项净影响），耗材来源单已作为分摊来源单独统计、不计入此处，确保金额不重计。"
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
          <Button href={REIMBURSE_SUMMARY_EXPORT_URL}>导出Excel</Button>
        </>
      }
    >
      <h4 style={SECTION_TITLE}>按采购单位汇总（报销口径）</h4>
      <Table<ReimburseSummaryPurchaserItem>
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
                  <b>{fmtMoney(pTotalReal)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={2}>
                  <b>{fmtMoney(pTotalSupp)}</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={3}>
                  <b>{fmtMoney(pTotalReim)}</b>
                </Table.Summary.Cell>
              </Table.Summary.Row>
            </Table.Summary>
          ) : null
        }
      />

      <h4 style={SECTION_TITLE_GAP}>按商品汇总（报销口径）</h4>
      <Table<ReimburseSummaryProductItem>
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
              </Table.Summary.Row>
            </Table.Summary>
          ) : null
        }
      />
    </ReportCard>
  )
}
