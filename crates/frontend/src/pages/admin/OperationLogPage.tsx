import { useState } from 'react'
import { Button, Card, DatePicker, Input, Space, Table, Tag, Typography } from 'antd'
import type { TableProps } from 'antd'
import { useQuery } from '@tanstack/react-query'
import dayjs from 'dayjs'
import {
  listOperationLogs,
  operationLogExportUrl,
  type OperationLogRow,
} from '../../api/admin/system'

const PAGE_SIZE = 20

/** 目标类型文案映射（照抄老页面 renderLogs） */
const TARGET_TYPE_LABELS: Record<string, string> = {
  purchase_order: '采购单',
  sales_order: '销售单',
  purchase_document: '采购单据',
}

const columns: TableProps<OperationLogRow>['columns'] = [
  { title: 'ID', dataIndex: 'id', width: 70 },
  { title: '时间', dataIndex: 'created_at', width: 170 },
  { title: '操作人', dataIndex: 'username', width: 110, render: (v: string | null) => v || '-' },
  {
    title: '动作',
    dataIndex: 'action_label',
    width: 110,
    render: (v: string) => <Tag color="cyan">{v}</Tag>,
  },
  {
    title: '目标',
    dataIndex: 'target_type',
    width: 160,
    render: (v: string, row) => ((TARGET_TYPE_LABELS[v] ?? v) || '-') + (row.target_id ? ` #${row.target_id}` : ''),
  },
  { title: '详情', dataIndex: 'detail', render: (v: string | null) => <span style={{ fontSize: 12 }}>{v || ''}</span> },
]

interface LogFilters {
  username: string
  action: string
  start_date: string
  end_date: string
}

const EMPTY_FILTERS: LogFilters = { username: '', action: '', start_date: '', end_date: '' }

/**
 * 旧页面 /system/operation_log：说明文字 + 查询条件（操作人/动作/起止日期）+ 分页表格 + 导出Excel，
 * 固定 page_size=20，文案与筛选行为照抄老页面。
 */
export default function OperationLogPage() {
  const [page, setPage] = useState(1)
  const [usernameInput, setUsernameInput] = useState('')
  const [actionInput, setActionInput] = useState('')
  const [startDateInput, setStartDateInput] = useState('')
  const [endDateInput, setEndDateInput] = useState('')
  const [filters, setFilters] = useState<LogFilters>(EMPTY_FILTERS)

  const { data, isLoading } = useQuery({
    queryKey: ['operationLogs', page, filters],
    queryFn: () => listOperationLogs({ page, page_size: PAGE_SIZE, ...filters }),
  })

  const rows = data?.data ?? []
  const total = data?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))

  const handleSearch = () => {
    setFilters({
      username: usernameInput.trim(),
      action: actionInput.trim(),
      start_date: startDateInput,
      end_date: endDateInput,
    })
    setPage(1)
  }

  const handleReset = () => {
    setUsernameInput('')
    setActionInput('')
    setStartDateInput('')
    setEndDateInput('')
    setFilters(EMPTY_FILTERS)
    setPage(1)
  }

  // 导出用当前输入框的值（照抄老页面 exportLogs 直接读 input）
  const handleExport = () => {
    window.location.href = operationLogExportUrl({
      username: usernameInput.trim(),
      action: actionInput.trim(),
      start_date: startDateInput,
      end_date: endDateInput,
    })
  }

  return (
    <Card size="small" title="操作日志">
      <Typography.Text type="secondary">
        记录所有关键写操作（创建/修改/删除/审核/反审核/调价/状态流转等），全程留痕可追溯。
      </Typography.Text>

      <Space wrap style={{ marginTop: 12, marginBottom: 12 }}>
        <Input
          placeholder="操作人"
          value={usernameInput}
          style={{ width: 140 }}
          onChange={(e) => setUsernameInput(e.target.value)}
          onPressEnter={handleSearch}
        />
        <Input
          placeholder="动作（如 审核）"
          value={actionInput}
          style={{ width: 160 }}
          onChange={(e) => setActionInput(e.target.value)}
          onPressEnter={handleSearch}
        />
        <DatePicker
          value={startDateInput ? dayjs(startDateInput) : null}
          onChange={(d) => setStartDateInput(d ? d.format('YYYY-MM-DD') : '')}
        />
        <DatePicker
          value={endDateInput ? dayjs(endDateInput) : null}
          onChange={(d) => setEndDateInput(d ? d.format('YYYY-MM-DD') : '')}
        />
        <Button type="primary" onClick={handleSearch}>
          查询
        </Button>
        <Button onClick={handleReset}>重置</Button>
        <Button type="primary" style={{ background: '#52c41a', borderColor: '#52c41a' }} onClick={handleExport}>
          导出Excel
        </Button>
      </Space>

      <Table<OperationLogRow>
        rowKey="id"
        size="small"
        loading={isLoading}
        dataSource={rows}
        columns={columns}
        scroll={{ x: 900 }}
        locale={{ emptyText: '暂无日志记录' }}
        pagination={{
          current: page,
          pageSize: PAGE_SIZE,
          total,
          showSizeChanger: false,
          showTotal: (t) =>
            totalPages <= 1 ? `共 ${t} 条记录` : `共 ${t} 条记录，当前第 ${page}/${totalPages} 页`,
          onChange: (p) => setPage(p),
        }}
      />
    </Card>
  )
}
