import { useEffect, useMemo, useState } from 'react'
import { App, Button, Card, Image, Input, Select, Space, Table, Tag, Upload } from 'antd'
import type { TableProps } from 'antd'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useSearchParams } from 'react-router-dom'
import dayjs from 'dayjs'
import {
  approveProduct,
  batchSetAutoUpdatePrice,
  deleteProduct,
  importProducts,
  listCategories,
  listProducts,
  lookupBatchPrices,
  setAutoUpdatePrice,
  toggleProductStatus,
  unapproveProduct,
  type Product,
} from '../../api/product'
import { useAuth } from '../../stores/auth'
import CreateProductCard from './CreateProductCard'
import ProductEditModal from './ProductEditModal'

const GOV_STATUS_OPTIONS = [
  { value: '', label: '政采状态：全部' },
  { value: 0, label: '非政采商品' },
  { value: 1, label: '政采（有指导价）' },
  { value: 2, label: '政采（无指导价）' },
]

export default function ProductPage() {
  const { message, modal } = App.useApp()
  const { user } = useAuth()
  const isSuperAdmin = user?.role === 'super_admin'
  const queryClient = useQueryClient()
  const [searchParams] = useSearchParams()

  const [keywordInput, setKeywordInput] = useState('')
  const [keyword, setKeyword] = useState('')
  const [govStatus, setGovStatus] = useState<number | null>(null)
  const [categoryId, setCategoryId] = useState<number | null>(
    searchParams.get('category_id') ? Number(searchParams.get('category_id')) : null,
  )

  // 分类筛选由全局侧边栏导航驱动（/product?category_id=N），URL 变化时同步
  useEffect(() => {
    const cid = searchParams.get('category_id')
    setCategoryId(cid ? Number(cid) : null)
  }, [searchParams])
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)
  const [editing, setEditing] = useState<Product | null>(null)

  const { data: categories = [] } = useQuery({
    queryKey: ['categories', 'product'],
    queryFn: async () => (await listCategories()).filter((c) => c.entity_type === 'product'),
  })

  const { data: listResp, isLoading } = useQuery({
    queryKey: ['products', { categoryId, keyword, govStatus, page, pageSize }],
    queryFn: () =>
      listProducts({
        category_id: categoryId,
        keyword,
        gov_status: govStatus,
        page,
        page_size: pageSize,
      }),
  })

  const products = listResp?.data ?? []
  const total = listResp?.total ?? 0

  const today = dayjs().format('YYYY-MM-DD')
  const ids = useMemo(() => products.map((p) => p.id), [products])
  const { data: batchPrices } = useQuery({
    queryKey: ['batchPrices', today, ids],
    queryFn: () => lookupBatchPrices(today, ids),
    enabled: ids.length > 0,
  })

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['products'] })

  const runAction = async (
    action: () => Promise<{ ok: boolean; message: string }>,
    successText: string,
    failPrefix: string,
  ) => {
    const res = await action()
    if (res.ok) {
      message.success(successText)
      refresh()
    } else {
      message.error(`${failPrefix}：${res.message || '操作失败'}`)
    }
  }

  const confirmThen = (title: string, action: () => void) => {
    modal.confirm({
      title,
      okText: '确定',
      cancelText: '取消',
      onOk: action,
    })
  }

  const handleToggle = (row: Product) => {
    if (row.audit_status === 'confirmed') {
      message.warning('该商品已审核，如需停用/启用请先反审核')
      return
    }
    const action = row.status === 1 ? '停用' : '启用'
    confirmThen(`确定要${action}商品「${row.name}」吗？`, () =>
      runAction(() => toggleProductStatus(row.id), `${action}成功`, `${action}失败`),
    )
  }

  const handleAuto = (row: Product) => {
    const next = row.auto_update_price === 1 ? 0 : 1
    const text = next === 1 ? '开启' : '关闭'
    confirmThen(`确定要${text}该商品的自动更新售价吗？`, () =>
      runAction(() => setAutoUpdatePrice(row.id, next), `${text}成功`, '操作失败'),
    )
  }

  const handleDelete = (row: Product) => {
    if (row.audit_status === 'confirmed') {
      message.warning('该商品已审核，如需删除请先反审核')
      return
    }
    confirmThen(`确定要删除商品「${row.name}」吗？`, () =>
      runAction(() => deleteProduct(row.id), '删除成功', '删除失败'),
    )
  }

  const handleBatchAuto = (auto: number) => {
    const text = auto === 1 ? '开启' : '关闭'
    confirmThen(`确定要${text}所有商品的自动更新售价吗？`, () =>
      runAction(() => batchSetAutoUpdatePrice(auto), `已${text}所有商品的自动更新售价`, '操作失败'),
    )
  }

  const columns: TableProps<Product>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    {
      title: '图片',
      dataIndex: 'image_url',
      width: 70,
      render: (url: string | null) =>
        url ? (
          <Image src={url} width={50} height={50} style={{ objectFit: 'cover', borderRadius: 4 }} />
        ) : (
          <div
            style={{
              width: 50,
              height: 50,
              background: '#f5f5f5',
              borderRadius: 4,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: '#ccc',
              fontSize: 12,
            }}
          >
            无图
          </div>
        ),
    },
    { title: '名称', dataIndex: 'name', width: 140 },
    { title: '规格', dataIndex: 'spec', width: 160, render: (v: string | null) => v || '' },
    { title: '显示单位', dataIndex: 'unit', width: 90 },
    { title: '基础单位', dataIndex: 'base_unit', width: 90 },
    { title: '保质期', dataIndex: 'shelf_life', width: 90, render: (v: string | null) => v || '' },
    {
      title: '售价',
      dataIndex: 'base_price',
      width: 90,
      render: (v: number) => (v > 0 ? `¥${v.toFixed(2)}` : <span style={{ color: '#999' }}>-</span>),
    },
    ...(isSuperAdmin
      ? [
          {
            title: '进价',
            dataIndex: 'purchase_price',
            width: 90,
            render: (v: number) => v || 0,
          } as NonNullable<TableProps<Product>['columns']>[number],
        ]
      : []),
    {
      title: '多单位',
      dataIndex: 'units',
      width: 160,
      render: (units: Product['units']) =>
        units && units.length > 0
          ? units.map((u) => `${u.unit_name}(${u.ratio})`).join(', ')
          : '',
    },
    {
      title: '分类',
      dataIndex: 'category_name',
      width: 110,
      render: (v: string | null) => v || '无分类',
    },
    {
      title: '状态',
      key: 'status',
      width: 190,
      render: (_v, row) => (
        <Space size={4} wrap>
          <Tag color={row.status === 1 ? 'success' : 'default'}>{row.status === 1 ? '启用' : '停用'}</Tag>
          <Tag color={row.auto_update_price === 1 ? 'blue' : 'default'}>
            {row.auto_update_price === 1 ? '自动' : '人工'}
          </Tag>
          {row.gov_status === 1 && <Tag color="geekblue">政采价</Tag>}
          {row.gov_status === 2 && <Tag color="purple">政采无价</Tag>}
        </Space>
      ),
    },
    {
      title: '最新政采/比价',
      key: 'policy',
      width: 130,
      render: (_v, row) => {
        const pp = batchPrices?.prices?.[String(row.id)]
        if (!pp) return <span style={{ color: '#999' }}>-</span>
        const gov = pp.gov_procurement || 0
        const maxSm = Math.max(pp.supermarket_1 || 0, pp.supermarket_2 || 0, pp.supermarket_3 || 0)
        if (gov > 0) return <Tag color="geekblue">政采 ¥{gov.toFixed(2)}</Tag>
        if (maxSm > 0) return <Tag color="cyan">比价 ¥{maxSm.toFixed(2)}</Tag>
        return <span style={{ color: '#999' }}>-</span>
      },
    },
    {
      title: '审核',
      dataIndex: 'audit_status',
      width: 90,
      render: (v: string | null) =>
        v === 'confirmed' ? <Tag color="success">已审核</Tag> : <Tag color="warning">待审核</Tag>,
    },
    {
      title: '操作',
      key: 'action',
      width: 340,
      render: (_v, row) => (
        <Space size={4} wrap>
          {isSuperAdmin &&
            (row.audit_status === 'confirmed' ? (
              <Button
                size="small"
                onClick={() => confirmThen('确定反审核该商品吗？反审核后需重新审核。', () =>
                  runAction(() => unapproveProduct(row.id), '反审核成功', '反审核失败'),
                )}
              >
                反审核
              </Button>
            ) : (
              <Button
                size="small"
                type="primary"
                ghost
                onClick={() => confirmThen('确定审核通过该商品吗？', () =>
                  runAction(() => approveProduct(row.id), '审核成功', '审核失败'),
                )}
              >
                审核
              </Button>
            ))}
          <Button
            size="small"
            href={`/query/price_schedule?product_id=${row.id}&product_name=${encodeURIComponent(row.name)}`}
          >
            价策略
          </Button>
          <Button
            size="small"
            onClick={() => {
              if (row.audit_status === 'confirmed') {
                message.warning('该商品已审核，如需修改请先反审核')
                return
              }
              setEditing(row)
            }}
          >
            编辑
          </Button>
          <Button size="small" onClick={() => handleToggle(row)}>
            {row.status === 1 ? '停用' : '启用'}
          </Button>
          <Button size="small" onClick={() => handleAuto(row)}>
            {row.auto_update_price === 1 ? '关闭自动' : '开启自动'}
          </Button>
          <Button size="small" danger onClick={() => handleDelete(row)}>
            删除
          </Button>
        </Space>
      ),
    },
  ]

  return (
    <Space direction="vertical" size="middle" style={{ display: 'flex' }}>
      <CreateProductCard categories={categories} isSuperAdmin={isSuperAdmin} onCreated={refresh} />

      <Card size="small">
        <Space wrap>
          <Input
            placeholder="搜索商品名称"
            value={keywordInput}
            style={{ width: 200 }}
            onChange={(e) => setKeywordInput(e.target.value)}
            onPressEnter={() => {
              setKeyword(keywordInput.trim())
              setPage(1)
            }}
          />
          <Select
            style={{ width: 170 }}
            value={govStatus ?? ''}
            options={GOV_STATUS_OPTIONS}
            onChange={(v) => {
              setGovStatus(v === '' ? null : Number(v))
              setPage(1)
            }}
          />
          <Select
            style={{ width: 160 }}
            placeholder="分类：全部"
            allowClear
            value={categoryId ?? undefined}
            options={categories.map((c) => ({ value: c.id, label: c.name }))}
            onChange={(v) => {
              setCategoryId(v ?? null)
              setPage(1)
            }}
          />
          <Button
            type="primary"
            onClick={() => {
              setKeyword(keywordInput.trim())
              setPage(1)
            }}
          >
            搜索
          </Button>
          <Button
            onClick={() => {
              setKeywordInput('')
              setKeyword('')
              setGovStatus(null)
              setCategoryId(null)
              setPage(1)
            }}
          >
            显示全部
          </Button>
          <Button type="link" href="/query/price_schedule">
            价格策略（按日期段）
          </Button>
          <Button onClick={() => handleBatchAuto(1)}>全部开启自动更新售价</Button>
          <Button onClick={() => handleBatchAuto(0)}>全部关闭自动更新售价</Button>
          <Button type="primary" ghost href="/api/product/export">
            导出
          </Button>
          <Upload
            accept=".xlsx,.csv"
            showUploadList={false}
            beforeUpload={(file) => {
              void (async () => {
                const res = await importProducts(file)
                if (res.ok) {
                  message.success(res.message || '导入完成')
                  refresh()
                } else {
                  message.error(res.message || '导入失败')
                }
              })()
              return false
            }}
          >
            <Button>导入</Button>
          </Upload>
        </Space>
      </Card>

      <Table<Product>
        rowKey="id"
        size="small"
        loading={isLoading}
        dataSource={products}
        columns={columns}
        scroll={{ x: 1500 }}
        pagination={{
          current: page,
          pageSize,
          total,
          showSizeChanger: true,
          pageSizeOptions: ['20', '50', '100'],
          showTotal: (t) => `共 ${t} 条商品`,
          onChange: (p, ps) => {
            setPage(p)
            setPageSize(ps)
          },
        }}
      />

      <ProductEditModal
        open={!!editing}
        product={editing}
        categories={categories}
        isSuperAdmin={isSuperAdmin}
        onClose={() => setEditing(null)}
        onSaved={() => {
          setEditing(null)
          refresh()
        }}
      />
    </Space>
  )
}
