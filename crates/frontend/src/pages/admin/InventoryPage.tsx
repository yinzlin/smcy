import { Card, Table } from 'antd'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import { listInventory, type InventoryRow } from '../../api/admin/system'
import { scrollX } from '../../utils/tableScroll'

const columns: TableProps<InventoryRow>['columns'] = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '商品ID', dataIndex: 'product_id', width: 90 },
  { title: '商品名称', dataIndex: 'name', width: 220 },
  { title: '规格', dataIndex: 'spec', width: 180, render: (v: string | null) => v || '' },
  { title: '库存数量', dataIndex: 'quantity', width: 110, align: 'right' },
  { title: '最低库存', dataIndex: 'min_stock', width: 110, align: 'right' },
  { title: '最高库存', dataIndex: 'max_stock', width: 110, align: 'right' },
]

/** 旧页面 /inventory：纯表格（ID/商品ID/商品名称/规格/库存数量/最低库存/最高库存），无筛选无分页 */
export default function InventoryPage() {
  const { data: rows = [], isLoading } = useQuery({
    queryKey: ['inventory'],
    queryFn: listInventory,
  })

  return (
    <Card size="small" title="库存管理">
      <Table<InventoryRow>
        rowKey="id"
        size="small"
        loading={isLoading}
        dataSource={rows}
        columns={columns}
        scroll={scrollX('max-content')}
        pagination={false}
      />
    </Card>
  )
}
