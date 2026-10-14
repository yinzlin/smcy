import { useMemo } from 'react'
import { Button, Table } from 'antd'
import { DownloadOutlined, SearchOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import ReportCard from '../../components/report/ReportCard'
import { fmtMoney } from '../../utils/format'
import { scrollX } from '../../utils/tableScroll'
import { fetchSupplierBalance, SUPPLIER_BALANCE_EXPORT_URL, type SupplierBalanceRow } from '../../api/query/prices'

const EXPORT_BTN_STYLE = { background: '#52c41a', borderColor: '#52c41a', color: '#fff' }

export default function SupplierBalancePage() {
  // 老页面进入即调用 searchSupplierBalance()：自动加载
  const { data, isLoading, isFetching, refetch } = useQuery({
    queryKey: ['supplierBalance'],
    queryFn: fetchSupplierBalance,
  })
  const rows = data ?? []

  // 合计照抄老页面：仅累加进货总额/已付款/未付款，预付款余额列留空
  const totals = useMemo(
    () =>
      rows.reduce(
        (acc, r) => ({
          purchase: acc.purchase + (Number(r.purchase_total) || 0),
          paid: acc.paid + (Number(r.paid_total) || 0),
          unpaid: acc.unpaid + (Number(r.unpaid) || 0),
        }),
        { purchase: 0, paid: 0, unpaid: 0 },
      ),
    [rows],
  )

  // 列顺序照抄老页面：供应商名称 本期进货总额 已付款 未付款 预付款余额
  const columns: TableProps<SupplierBalanceRow>['columns'] = [
    { title: '供应商名称', dataIndex: 'name' },
    { title: '本期进货总额', dataIndex: 'purchase_total', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '已付款', dataIndex: 'paid_total', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '未付款', dataIndex: 'unpaid', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '预付款余额', dataIndex: 'prepay_balance', align: 'right', render: (v: number) => fmtMoney(v) },
  ]

  return (
    <ReportCard
      title="供应商往来对账"
      actions={
        <>
          <Button type="primary" icon={<SearchOutlined />} loading={isFetching && !isLoading} onClick={() => void refetch()}>
            查询
          </Button>
          <Button icon={<DownloadOutlined />} href={SUPPLIER_BALANCE_EXPORT_URL} style={EXPORT_BTN_STYLE}>
            导出Excel
          </Button>
        </>
      }
    >
      <Table<SupplierBalanceRow>
        rowKey="name"
        size="small"
        loading={isLoading}
        columns={columns}
        dataSource={rows}
        scroll={scrollX('max-content')}
        pagination={false}
        summary={() =>
          rows.length > 0 ? (
            <Table.Summary.Row style={{ background: '#fafafa', fontWeight: 600 }}>
              <Table.Summary.Cell index={0}>合计</Table.Summary.Cell>
              <Table.Summary.Cell index={1} align="right">
                {fmtMoney(totals.purchase)}
              </Table.Summary.Cell>
              <Table.Summary.Cell index={2} align="right">
                {fmtMoney(totals.paid)}
              </Table.Summary.Cell>
              <Table.Summary.Cell index={3} align="right">
                {fmtMoney(totals.unpaid)}
              </Table.Summary.Cell>
              {/* 老页面合计行预付款余额列为空 */}
              <Table.Summary.Cell index={4}>{' '}</Table.Summary.Cell>
            </Table.Summary.Row>
          ) : null
        }
      />
    </ReportCard>
  )
}
