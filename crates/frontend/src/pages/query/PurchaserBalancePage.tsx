import { useMemo } from 'react'
import { Button, Table } from 'antd'
import { DownloadOutlined, SearchOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import ReportCard from '../../components/report/ReportCard'
import { fmtMoney } from '../../utils/format'
import { fetchPurchaserBalance, PURCHASER_BALANCE_EXPORT_URL, type PurchaserBalanceRow } from '../../api/query/prices'

const EXPORT_BTN_STYLE = { background: '#52c41a', borderColor: '#52c41a', color: '#fff' }

export default function PurchaserBalancePage() {
  // 老页面进入即调用 searchPurchaserBalance()：自动加载
  const { data, isLoading, isFetching, refetch } = useQuery({
    queryKey: ['purchaserBalance'],
    queryFn: fetchPurchaserBalance,
  })
  const rows = data ?? []

  // 合计照抄老页面：仅累加累计销售/已收款/未收款，预收款余额列留空
  const totals = useMemo(
    () =>
      rows.reduce(
        (acc, r) => ({
          sales: acc.sales + (Number(r.sales_total) || 0),
          received: acc.received + (Number(r.received_total) || 0),
          unreceived: acc.unreceived + (Number(r.unreceived) || 0),
        }),
        { sales: 0, received: 0, unreceived: 0 },
      ),
    [rows],
  )

  // 列顺序照抄老页面：采购单位名称 累计销售 已收款 未收款 预收款余额
  const columns: TableProps<PurchaserBalanceRow>['columns'] = [
    { title: '采购单位名称', dataIndex: 'name' },
    { title: '累计销售', dataIndex: 'sales_total', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '已收款', dataIndex: 'received_total', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '未收款', dataIndex: 'unreceived', align: 'right', render: (v: number) => fmtMoney(v) },
    { title: '预收款余额', dataIndex: 'prepay_balance', align: 'right', render: (v: number) => fmtMoney(v) },
  ]

  return (
    <ReportCard
      title="采购方应收对账"
      actions={
        <>
          <Button type="primary" icon={<SearchOutlined />} loading={isFetching && !isLoading} onClick={() => void refetch()}>
            查询
          </Button>
          <Button icon={<DownloadOutlined />} href={PURCHASER_BALANCE_EXPORT_URL} style={EXPORT_BTN_STYLE}>
            导出Excel
          </Button>
        </>
      }
    >
      <Table<PurchaserBalanceRow>
        rowKey="name"
        size="small"
        loading={isLoading}
        columns={columns}
        dataSource={rows}
        pagination={false}
        summary={() =>
          rows.length > 0 ? (
            <Table.Summary.Row style={{ background: '#fafafa', fontWeight: 600 }}>
              <Table.Summary.Cell index={0}>合计</Table.Summary.Cell>
              <Table.Summary.Cell index={1} align="right">
                {fmtMoney(totals.sales)}
              </Table.Summary.Cell>
              <Table.Summary.Cell index={2} align="right">
                {fmtMoney(totals.received)}
              </Table.Summary.Cell>
              <Table.Summary.Cell index={3} align="right">
                {fmtMoney(totals.unreceived)}
              </Table.Summary.Cell>
              {/* 老页面合计行预收款余额列为空 */}
              <Table.Summary.Cell index={4}>{' '}</Table.Summary.Cell>
            </Table.Summary.Row>
          ) : null
        }
      />
    </ReportCard>
  )
}
