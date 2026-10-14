import { useEffect, useMemo, useRef, useState } from 'react'
import {
  Alert, App, AutoComplete, Button, Card, DatePicker, Form, Input, InputNumber, Modal, Popconfirm, Row, Col, Select, Space, Table, Tag, Upload,
} from 'antd'
import { DownloadOutlined, PlusOutlined, ThunderboltOutlined, UploadOutlined } from '@ant-design/icons'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useSearchParams } from 'react-router-dom'
import {
  backfillSalesOrder,
  batchImportExcel,
  createPriceSchedule,
  diagnosePriceSchedule,
  EXPORT_TEMPLATE_URL,
  exportListUrl,
  deletePriceSchedule,
  getPriceSchedule,
  listPriceSchedules,
  PRICE_TYPE_COLOR,
  PRICE_TYPE_LABEL,
  PRICE_TYPE_OPTIONS,
  updatePriceSchedule,
  type DiagnoseResp,
  type PriceScheduleRow,
  type PriceType,
} from '../../api/priceschedule'
import { listSuppliers, searchProducts, type ProductSearchItem } from '../../api/purchase'
import { listPurchasers } from '../../api/sales'
import { useAuth } from '../../stores/auth'
import { scrollX } from '../../utils/tableScroll'

const SOURCE_PRESETS = ['菜篮子/询价表', '政采平台', '超市实地采价', 'Excel导入']

interface EditForm {
  id: number | null
  product_id: number
  product_keyword: string
  price_type: PriceType
  price: number | null
  effective_date: string
  source: string
  remark: string
  supplier_id: number | null
  purchaser_id: number | null
}

export default function PriceSchedulePage() {
  const { message, modal } = App.useApp()
  const { user } = useAuth()
  const queryClient = useQueryClient()
  const canManage = user?.role === 'super_admin' || user?.role === 'admin'

  // URL 预置商品筛选：/app/price-schedule?product_id=xx&product_name=yy（商品页跳转入口）
  const [searchParams, setSearchParams] = useSearchParams()
  const presetProductId = Number(searchParams.get('product_id') || 0)
  const presetProductName = searchParams.get('product_name') || ''

  const [filterType, setFilterType] = useState<string>('')
  const [filterFrom, setFilterFrom] = useState('')
  const [filterTo, setFilterTo] = useState('')
  const [keyword, setKeyword] = useState('')
  const [filterProductId, setFilterProductId] = useState<number | null>(presetProductId || null)
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)

  const [editOpen, setEditOpen] = useState(false)
  const [editSaving, setEditSaving] = useState(false)
  const [editForm, setEditForm] = useState<EditForm | null>(null)
  /** 新增模式下选中商品后的时段链缓存（用于带出最近一条价格） */
  const typeCacheRef = useRef<DiagnoseResp['types'] | null>(null)

  const [diagData, setDiagData] = useState<DiagnoseResp | null>(null)
  const [backfillOpen, setBackfillOpen] = useState(false)

  const { data: suppliers = [] } = useQuery({ queryKey: ['suppliers'], queryFn: listSuppliers })
  const { data: purchasers = [] } = useQuery({ queryKey: ['purchasers'], queryFn: listPurchasers })

  const listQuery = useMemo(
    () => ({
      page,
      page_size: pageSize,
      price_type: filterType,
      effective_from: filterFrom,
      effective_to: filterTo,
      keyword,
      product_id: filterProductId,
    }),
    [page, pageSize, filterType, filterFrom, filterTo, keyword, filterProductId],
  )

  const { data: listResp, isLoading } = useQuery({
    queryKey: ['priceSchedules', listQuery],
    queryFn: () => listPriceSchedules(listQuery),
  })
  const rows = listResp?.data ?? []

  // URL 带 product_id 时直接以该商品查询
  useEffect(() => {
    if (presetProductId > 0) {
      setFilterProductId(presetProductId)
      setKeyword(presetProductName)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const clearPreset = () => {
    searchParams.delete('product_id')
    searchParams.delete('product_name')
    setSearchParams(searchParams, { replace: true })
    setFilterProductId(null)
    setKeyword('')
    setPage(1)
  }

  // ===== 商品搜索联想（新增/编辑弹窗 + 列表筛选共用）=====
  const [searchMap, setSearchMap] = useState<Record<string, ProductSearchItem[]>>({})
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const handleProductSearch = (scope: string, kw: string) => {
    if (searchTimer.current) clearTimeout(searchTimer.current)
    const key = kw.trim()
    if (!key) {
      setSearchMap((m) => ({ ...m, [scope]: [] }))
      return
    }
    searchTimer.current = setTimeout(async () => {
      try {
        const products = await searchProducts(key)
        setSearchMap((m) => ({ ...m, [scope]: products }))
      } catch {
        // 忽略
      }
    }, 250)
  }

  const productOptions = (scope: string, withPurchasePrice: boolean) =>
    (searchMap[scope] ?? []).slice(0, 15).map((p) => {
      const aliases = [p.alias1, p.alias2].filter((a) => a && a.trim())
      return {
        value: String(p.id),
        label: (
          <div>
            <strong>{p.name}</strong>
            {aliases.length > 0 && <span style={{ color: '#1677ff', fontSize: 12 }}>（{aliases.join(' / ')}）</span>}
            {withPurchasePrice && (p.purchase_price || 0) > 0 && (
              <span style={{ float: 'right', fontSize: 12, fontWeight: 600, color: '#fd7e14', background: '#fff3e0', padding: '1px 6px', borderRadius: 8 }}>
                进价 ¥{(p.purchase_price || 0).toFixed(2)}/{p.base_unit || ''}
              </span>
            )}
            <div>
              <small>{[p.spec, p.unit, p.category_name].filter((x) => x && String(x).trim()).join(' · ')}</small>
            </div>
          </div>
        ),
        product: p,
      }
    })

  // ===== 编辑弹窗 =====
  const openCreate = () => {
    typeCacheRef.current = null
    setEditForm({
      id: null,
      product_id: 0,
      product_keyword: '',
      price_type: 'gov_procurement',
      price: null,
      effective_date: dayjs().format('YYYY-MM-DD'),
      source: '菜篮子/询价表',
      remark: '',
      supplier_id: null,
      purchaser_id: null,
    })
    setEditOpen(true)
  }

  // 新增模式下：选中商品后拉取完整时段链缓存，供价格类型切换时带出最近一条
  const loadTypeCache = (productId: number) => {
    typeCacheRef.current = null
    if (!productId) return
    void diagnosePriceSchedule(productId)
      .then((d) => {
        if (d.success) typeCacheRef.current = d.types
      })
      .catch(() => {
        typeCacheRef.current = null
      })
      .finally(() => applyPrefillFromCache())
  }

  // 仅新增模式回填：按当前价格类型取最近一条时段的价格与生效日期
  const applyPrefillFromCache = () => {
    setEditForm((f) => {
      if (!f || f.id) return f
      const list = typeCacheRef.current?.[f.price_type]
      if (!list || list.length === 0) {
        message.info(`该商品暂无「${PRICE_TYPE_LABEL[f.price_type]}」历史记录，生效日期默认为今天`)
        return f
      }
      const latest = list[list.length - 1]
      message.info(`已带出最近一条「${PRICE_TYPE_LABEL[f.price_type]}」：¥${Number(latest.price).toFixed(2)}（生效日 ${latest.effective_date}），如需新增时段请修改生效日期`)
      return {
        ...f,
        price: latest.price,
        effective_date: latest.effective_date,
        source: latest.source || f.source,
      }
    })
  }

  const openEdit = async (id: number) => {
    try {
      const d = await getPriceSchedule(id)
      if (!d.success) throw new Error('未找到该记录')
      const r = d.data
      typeCacheRef.current = null
      setEditForm({
        id: r.id,
        product_id: r.product_id,
        product_keyword: (r.product_name || '') + (r.spec ? `（${r.spec}）` : ''),
        price_type: r.price_type,
        price: r.price,
        effective_date: r.effective_date,
        source: r.source || '',
        remark: r.remark || '',
        supplier_id: r.supplier_id,
        purchaser_id: r.purchaser_id,
      })
      setEditOpen(true)
    } catch (e) {
      message.error(`${e instanceof Error ? e.message : e}（id=${id}），请刷新列表后重试`)
    }
  }

  const submitEdit = async () => {
    const f = editForm
    if (!f) return
    if (!f.product_id) {
      message.warning('请先选择商品')
      return
    }
    if ((f.price ?? -1) < 0 || f.price === null) {
      message.warning('价格无效')
      return
    }
    if (!f.effective_date) {
      message.warning('请选择生效日期')
      return
    }
    if (f.price_type === 'supplier_contract' && !f.supplier_id) {
      message.warning('请选择供应商')
      return
    }
    if (f.price_type === 'purchaser_contract' && !f.purchaser_id) {
      message.warning('请选择采购单位')
      return
    }
    // 编辑模式下切换商品：原商品下该时段的策略将被移除，新商品下将新增
    if (f.id) {
      const d = await getPriceSchedule(f.id).catch(() => null)
      if (d?.success && d.data.product_id !== f.product_id) {
        const ok = await new Promise<boolean>((resolve) => {
          modal.confirm({
            title: '已切换商品，原商品下该时段的策略将被移除，新商品下将新增。是否继续？',
            okText: '确定',
            cancelText: '取消',
            onOk: () => resolve(true),
            onCancel: () => resolve(false),
          })
        })
        if (!ok) return
      }
    }
    setEditSaving(true)
    try {
      const payload = {
        id: f.id,
        product_id: f.product_id,
        price_type: f.price_type,
        price: f.price,
        effective_date: f.effective_date,
        end_date: null,
        source: f.source,
        remark: f.remark,
        supplier_id: f.price_type === 'supplier_contract' ? f.supplier_id : null,
        purchaser_id: f.price_type === 'purchaser_contract' ? f.purchaser_id : null,
      }
      const res = f.id ? await updatePriceSchedule(payload) : await createPriceSchedule(payload)
      if (res.ok) {
        message.success(f.id ? '已更新' : '已创建')
        setEditOpen(false)
        void queryClient.invalidateQueries({ queryKey: ['priceSchedules'] })
      } else {
        let msg = res.message
        try {
          msg = (JSON.parse(res.message) as { message?: string }).message || res.message
        } catch {
          // 纯文本
        }
        message.error(`保存失败：${msg}`)
      }
    } finally {
      setEditSaving(false)
    }
  }

  const delRecord = async (id: number) => {
    const res = await deletePriceSchedule(id)
    if (res.ok) {
      message.success('已删除')
      void queryClient.invalidateQueries({ queryKey: ['priceSchedules'] })
    } else {
      let msg = res.message
      try {
        msg = (JSON.parse(res.message) as { message?: string }).message || res.message
      } catch {
        // 纯文本
      }
      message.error(`删除失败：${msg}`)
    }
  }

  const openDiagnose = async (productId: number) => {
    try {
      const d = await diagnosePriceSchedule(productId)
      if (!d.success) throw new Error('诊断失败')
      setDiagData(d)
    } catch (e) {
      message.error(`${e instanceof Error ? e.message : e}`)
    }
  }

  const columns: TableProps<PriceScheduleRow>['columns'] = [
    { title: '商品ID', dataIndex: 'product_id', width: 80 },
    {
      title: '商品名称',
      dataIndex: 'product_name',
      width: 170,
      render: (v: string, o) => (
        <div>
          <div>{v}</div>
          {(o.alias1 || o.alias2) && (
            <small style={{ color: '#999' }}>{[o.alias1, o.alias2].filter(Boolean).join(' ')}</small>
          )}
        </div>
      ),
    },
    {
      title: '规格',
      dataIndex: 'spec',
      width: 120,
      render: (v: string, o) => (
        <div>
          <div>{v}</div>
          <small>{o.unit}</small>
        </div>
      ),
    },
    {
      title: '价格类型',
      dataIndex: 'price_type',
      width: 130,
      render: (v: PriceType) => <Tag color={PRICE_TYPE_COLOR[v]}>{PRICE_TYPE_LABEL[v] ?? v}</Tag>,
    },
    {
      title: '约定对象',
      key: 'party',
      width: 170,
      render: (_v, o) =>
        o.supplier_name ? (
          <span>
            <Tag color="green">供应商</Tag>
            {o.supplier_name}
          </span>
        ) : o.purchaser_name ? (
          <span>
            <Tag color="magenta">采购单位</Tag>
            {o.purchaser_name}
          </span>
        ) : (
          '-'
        ),
    },
    { title: '生效日期', dataIndex: 'effective_date', width: 105, render: (v: string) => <code>{v}</code> },
    {
      title: '时段（含）',
      key: 'range',
      width: 190,
      render: (_v, o) =>
        o.end_date ? (
          <span>
            <code>{o.effective_date}</code> ~ <code>{dayjs(o.end_date).subtract(1, 'day').format('YYYY-MM-DD')}</code>
          </span>
        ) : (
          <span>
            <code>{o.effective_date}</code> ~ <Tag color="success">至今</Tag>
          </span>
        ),
    },
    { title: '价格', dataIndex: 'price', width: 90, align: 'right', render: (v: number) => <b>¥{v.toFixed(2)}</b> },
    { title: '来源', dataIndex: 'source', width: 110, render: (v: string) => <small>{v}</small> },
    { title: '备注', dataIndex: 'remark', width: 130, render: (v: string) => <small>{v}</small> },
    {
      title: '操作',
      key: 'action',
      width: 210,
      render: (_v, o) => (
        <Space size={4}>
          {canManage && (
            <>
              <Button size="small" onClick={() => void openEdit(o.id)}>
                编辑
              </Button>
              <Popconfirm title="确定删除该价格策略吗？此操作不可恢复。" onConfirm={() => void delRecord(o.id)}>
                <Button size="small" danger>
                  删除
                </Button>
              </Popconfirm>
            </>
          )}
          <Button size="small" onClick={() => void openDiagnose(o.product_id)}>
            时段
          </Button>
        </Space>
      ),
    },
  ]

  const exportUrl = exportListUrl(listQuery)

  return (
    <>
      <Card>
        <Space wrap size="middle" style={{ marginBottom: 16 }}>
          <div>
            <div style={{ marginBottom: 4 }}>价格类型</div>
            <Select style={{ width: 180 }} value={filterType} onChange={(v) => { setFilterType(v); setPage(1) }} options={[{ value: '', label: '全部' }, ...PRICE_TYPE_OPTIONS]} />
          </div>
          <div>
            <div style={{ marginBottom: 4 }}>生效日期（起）</div>
            <DatePicker value={filterFrom ? dayjs(filterFrom) : null} onChange={(d) => { setFilterFrom(d ? d.format('YYYY-MM-DD') : ''); setPage(1) }} />
          </div>
          <div>
            <div style={{ marginBottom: 4 }}>生效日期（止）</div>
            <DatePicker value={filterTo ? dayjs(filterTo) : null} onChange={(d) => { setFilterTo(d ? d.format('YYYY-MM-DD') : ''); setPage(1) }} />
          </div>
          <div>
            <div style={{ marginBottom: 4 }}>商品关键字</div>
            <AutoComplete
              style={{ width: 220 }}
              value={keyword}
              placeholder="商品名/别称模糊搜索"
              options={productOptions('filter', false)}
              onChange={(v) => {
                if ((searchMap['filter'] ?? []).some((p) => String(p.id) === v)) return
                setKeyword(v)
                setFilterProductId(null)
                handleProductSearch('filter', v)
              }}
              onSelect={(_v, opt) => {
                const p = (opt as { product: ProductSearchItem }).product
                setFilterProductId(p.id)
                setKeyword(p.name)
                setPage(1)
              }}
            />
          </div>
        </Space>

        {filterProductId !== null && filterProductId > 0 && (
          <Alert
            type="info"
            showIcon
            style={{ marginBottom: 12 }}
            message={
              <span>
                当前仅显示商品：<b>{presetProductName || keyword || `ID ${filterProductId}`}</b>（id={filterProductId}）
                <a style={{ marginLeft: 8 }} onClick={clearPreset}>清除筛选</a>
              </span>
            }
          />
        )}

        <Space wrap style={{ marginBottom: 16 }}>
          {canManage && (
            <>
              <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>
                新增价格策略
              </Button>
              <Upload accept=".xlsx" showUploadList={false} beforeUpload={(file) => {
                void batchImportExcel(file)
                  .then((d) => {
                    let msg = `导入完成：inserted=${d.inserted}, skipped=${d.skipped}`
                    if (d.errors.length > 0) msg += `\n\n错误明细：\n${d.errors.join('\n')}`
                    modal.info({ title: '批量导入结果', content: <div style={{ whiteSpace: 'pre-line' }}>{msg}</div> })
                    void queryClient.invalidateQueries({ queryKey: ['priceSchedules'] })
                  })
                  .catch((e) => message.error(`导入失败：${e instanceof Error ? e.message : e}`))
                return false
              }}>
                <Button icon={<UploadOutlined />}>批量导入 Excel</Button>
              </Upload>
              <Button icon={<ThunderboltOutlined />} onClick={() => setBackfillOpen(true)}>
                销售单按价补单
              </Button>
            </>
          )}
          <Button icon={<DownloadOutlined />} href={EXPORT_TEMPLATE_URL}>
            下载导入模板
          </Button>
          <Button icon={<DownloadOutlined />} href={exportUrl}>
            导出列表
          </Button>
          <Button
            onClick={() => {
              setFilterType('')
              setFilterFrom('')
              setFilterTo('')
              setKeyword('')
              setFilterProductId(null)
              setPage(1)
              if (searchParams.get('product_id')) clearPreset()
            }}
          >
            重置
          </Button>
          <span style={{ color: '#999' }}>共 {listResp?.total ?? 0} 条</span>
        </Space>

        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 16 }}
          message={
            <span>
              <b>按生效日期录入，系统自动维护时段连续性</b>：每个 (商品, 价格类型) 的多笔记录按生效日期升序排列，后一条的生效日期即前一条的失效日期，最后一条失效日期为「至今」。
            </span>
          }
        />

        <Table<PriceScheduleRow>
          rowKey="id"
          size="small"
          loading={isLoading}
          columns={columns}
          dataSource={rows}
          scroll={{ x: 1400 }}
          pagination={{
            current: page,
            pageSize,
            total: listResp?.total ?? 0,
            showSizeChanger: true,
            showTotal: (t) => `共 ${t} 条记录`,
            onChange: (p, ps) => {
              setPage(p)
              setPageSize(ps)
            },
          }}
        />
      </Card>

      {/* 新增/编辑弹窗 */}
      <Modal
        title={editForm?.id ? `编辑价格策略 #${editForm.id}` : '新增价格策略'}
        open={editOpen}
        onCancel={() => setEditOpen(false)}
        onOk={() => void submitEdit()}
        confirmLoading={editSaving}
        width={560}
      >
        {editForm && (
          <Form layout="vertical">
            <Form.Item label="商品" required>
              <AutoComplete
                style={{ width: '100%' }}
                value={editForm.product_keyword}
                placeholder="输入商品名称搜索"
                options={productOptions('edit', true)}
                onChange={(v) => {
                  if ((searchMap['edit'] ?? []).some((p) => String(p.id) === v)) return
                  // 手动改动文本即视为重新选择，清掉已绑定的 product_id 与时段缓存
                  setEditForm((f) => (f ? { ...f, product_keyword: v, product_id: 0 } : f))
                  typeCacheRef.current = null
                  handleProductSearch('edit', v)
                }}
                onSelect={(_v, opt) => {
                  const p = (opt as { product: ProductSearchItem }).product
                  setEditForm((f) =>
                    f ? { ...f, product_id: p.id, product_keyword: p.name + (p.spec ? `（${p.spec}）` : '') } : f,
                  )
                  loadTypeCache(p.id)
                }}
              />
            </Form.Item>
            <Row gutter={[8, 8]}>
              <Col xs={24} sm={12}>
                <Form.Item label="价格类型" required style={{ width: '100%', maxWidth: 240 }}>
                  <Select
                    value={editForm.price_type}
                    onChange={(v) => {
                      setEditForm((f) => (f ? { ...f, price_type: v } : f))
                      // 仅新增模式下自动带出该类型最近一条
                      if (!editForm.id) setTimeout(applyPrefillFromCache, 0)
                    }}
                    options={PRICE_TYPE_OPTIONS}
                  />
                </Form.Item>
              </Col>
              <Col xs={24} sm={12}>
                <Form.Item label="生效日期" required>
                  <DatePicker
                    value={editForm.effective_date ? dayjs(editForm.effective_date) : null}
                    onChange={(d) => setEditForm((f) => (f ? { ...f, effective_date: d ? d.format('YYYY-MM-DD') : '' } : f))}
                  />
                </Form.Item>
              </Col>
            </Row>
            {editForm.price_type === 'supplier_contract' && (
              <Form.Item label="供应商" required>
                <Select
                  showSearch
                  optionFilterProp="label"
                  placeholder="请选择供应商"
                  value={editForm.supplier_id ?? undefined}
                  onChange={(v) => setEditForm((f) => (f ? { ...f, supplier_id: v ?? null } : f))}
                  options={suppliers.map((s) => ({ value: s.id, label: s.name }))}
                />
              </Form.Item>
            )}
            {editForm.price_type === 'purchaser_contract' && (
              <Form.Item label="采购单位" required>
                <Select
                  showSearch
                  optionFilterProp="label"
                  placeholder="请选择采购单位"
                  value={editForm.purchaser_id ?? undefined}
                  onChange={(v) => setEditForm((f) => (f ? { ...f, purchaser_id: v ?? null } : f))}
                  options={purchasers.map((p) => ({ value: p.id, label: p.name }))}
                />
              </Form.Item>
            )}
            <Row gutter={[8, 8]}>
              <Col xs={24} sm={12}>
                <Form.Item label="价格" required>
                  <InputNumber
                    min={0}
                    step={0.01}
                    precision={2}
                    value={editForm.price}
                    onChange={(v) => setEditForm((f) => (f ? { ...f, price: v } : f))}
                  />
                </Form.Item>
              </Col>
              <Col xs={24} sm={12}>
                <Form.Item label="来源" style={{ width: '100%', maxWidth: 220 }}>
                  <AutoComplete
                    value={editForm.source}
                    placeholder="菜篮子/询价表"
                    options={SOURCE_PRESETS.map((s) => ({ value: s }))}
                    onChange={(v) => setEditForm((f) => (f ? { ...f, source: v } : f))}
                  />
                </Form.Item>
              </Col>
            </Row>
            <Form.Item label="备注">
              <Input value={editForm.remark} placeholder="选填" onChange={(e) => setEditForm((f) => (f ? { ...f, remark: e.target.value } : f))} />
            </Form.Item>
            <div style={{ color: '#999', fontSize: 12 }}>
              时段连续性由系统自动维护；保存后该 (商品, 价格类型) 的后续所有时段按生效日期自动归并。
            </div>
          </Form>
        )}
      </Modal>

      {/* 时段诊断弹窗 */}
      <Modal title="时段诊断" open={diagData !== null} footer={null} onCancel={() => setDiagData(null)} width={640}>
        {diagData && (
          <div>
            <div style={{ marginBottom: 12 }}>
              <b>{diagData.product_name}</b>（ID {diagData.product_id}）共 {Object.keys(diagData.types).length} 种价格类型
            </div>
            {Object.entries(diagData.types).map(([k, list]) => (
              <div key={k} style={{ marginBottom: 16 }}>
                <div style={{ marginBottom: 4 }}>
                  <Tag color={PRICE_TYPE_COLOR[k as PriceType]}>{PRICE_TYPE_LABEL[k as PriceType] ?? k}</Tag>
                  <small style={{ color: '#999' }}>{list.length} 个时段</small>
                </div>
                <Table
                  size="small"
                  rowKey="id"
                  pagination={false}
                  dataSource={list}
                  scroll={scrollX('max-content')}
                  columns={[
                    { title: 'ID', dataIndex: 'id', width: 60 },
                    {
                      title: '时段（含）',
                      key: 'range',
                      render: (_v, it) =>
                        it.end_date ? (
                          <span>
                            <code>{it.effective_date}</code> ~ <code>{dayjs(it.end_date).subtract(1, 'day').format('YYYY-MM-DD')}</code>
                          </span>
                        ) : (
                          <span>
                            <code>{it.effective_date}</code> ~ <Tag color="success">至今</Tag>
                          </span>
                        ),
                    },
                    { title: '价格', dataIndex: 'price', width: 90, align: 'right', render: (v: number) => <b>¥{v.toFixed(2)}</b> },
                    { title: '来源', dataIndex: 'source', render: (v: string) => <small>{v}</small> },
                  ]}
                />
              </div>
            ))}
          </div>
        )}
      </Modal>

      {/* 销售单按价补单 */}
      <BackfillModal
        open={backfillOpen}
        onClose={() => setBackfillOpen(false)}
      />
    </>
  )
}

function BackfillModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { message, modal } = App.useApp()
  const [orderId, setOrderId] = useState<number | null>(null)
  const [priceType, setPriceType] = useState<string>('gov_procurement')
  const [force, setForce] = useState(false)
  const [loading, setLoading] = useState(false)

  const run = async () => {
    if (!orderId) {
      message.warning('请输入销售单 ID')
      return
    }
    setLoading(true)
    try {
      const d = await backfillSalesOrder(orderId, priceType, force)
      let msg = `按 order_date=${d.order_date} 应用 ${d.price_type}\n更新 ${d.updated} 条，保留 ${d.kept} 条`
      if (d.missing && d.missing.length > 0) msg += `\n\n未找到价：\n${d.missing.join('\n')}`
      modal.info({ title: '补价完成', content: <div style={{ whiteSpace: 'pre-line' }}>{msg}</div> })
      onClose()
    } catch (e) {
      message.error(`失败：${e instanceof Error ? e.message : e}`)
    } finally {
      setLoading(false)
    }
  }

  return (
    <Modal
      title="销售单按价补单"
      open={open}
      onCancel={onClose}
      onOk={() => void run()}
      confirmLoading={loading}
      okText="执行补价"
    >
      <Form layout="vertical">
        <Form.Item label="销售单 ID" required>
          <InputNumber style={{ width: '100%' }} min={1} precision={0} value={orderId} onChange={(v) => setOrderId(v ?? null)} />
        </Form.Item>
        <Form.Item label="价格类型" required>
          <Select
            value={priceType}
            onChange={setPriceType}
            options={PRICE_TYPE_OPTIONS.filter((o) => o.value !== 'supplier_contract' && o.value !== 'purchaser_contract')}
          />
        </Form.Item>
        <Form.Item>
          <Select
            value={force ? '1' : '0'}
            onChange={(v) => setForce(v === '1')}
            options={[
              { value: '0', label: '仅补 unit_price=0 的明细' },
              { value: '1', label: '强制覆盖全部明细（force=1）' },
            ]}
          />
        </Form.Item>
      </Form>
    </Modal>
  )
}
