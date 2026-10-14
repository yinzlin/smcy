import { useMemo, useRef, useState } from 'react'
import { App, Button, DatePicker, Input, Modal, Select, Space, Tag } from 'antd'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import dayjs from 'dayjs'
import {
  deletePurchaseDocument,
  fetchPurchaseDocuments,
  uploadPurchaseDocument,
  type PurchaseDocumentItem,
} from '../../api/query/orders'
import { listSuppliers } from '../../api/purchase'
import ReportCard, { FilterItem } from '../../components/report/ReportCard'

export default function PurchaseDocumentPage() {
  const { message, modal } = App.useApp()
  const queryClient = useQueryClient()

  // 老页面：单据日期默认今天，且进入页面即按「供应商空 + 今天」加载单据
  const today = dayjs().format('YYYY-MM-DD')
  const [supplierId, setSupplierId] = useState('')
  const [date, setDate] = useState(today)
  const [remark, setRemark] = useState('')
  const [applied, setApplied] = useState<{ supplier_id: string; document_date: string }>({
    supplier_id: '',
    document_date: today,
  })

  // 上传状态文案（老页面：正在上传 X/Y ... → 本次上传完成：X/Y 张）
  const [uploadStatus, setUploadStatus] = useState('')
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const { data: suppliers = [] } = useQuery({ queryKey: ['suppliers'], queryFn: listSuppliers })

  const docsQuery = useQuery({
    queryKey: ['purchaseDocuments', applied.supplier_id, applied.document_date],
    queryFn: () => fetchPurchaseDocuments(applied.supplier_id, applied.document_date),
  })
  const docs = docsQuery.data ?? []

  // 老页面按「供应商名 | 日期」分组，组顺序按接口返回顺序首次出现位置
  const groups = useMemo(() => {
    const map = new Map<string, PurchaseDocumentItem[]>()
    docs.forEach((d) => {
      const key = `${d.supplier_name} | ${d.document_date}`
      const arr = map.get(key)
      if (arr) arr.push(d)
      else map.set(key, [d])
    })
    return Array.from(map.entries())
  }, [docs])

  const handleSearch = () => {
    setApplied({ supplier_id: supplierId, document_date: date })
  }

  const startCapture = () => {
    if (!supplierId) {
      message.warning('请先选择供应商')
      return
    }
    if (!date) {
      message.warning('请先选择单据日期')
      return
    }
    fileInputRef.current?.click()
  }

  const handleFiles = async (files: FileList | null) => {
    if (!files || files.length === 0) return
    const supplier = suppliers.find((s) => String(s.id) === supplierId)
    const fileArr = Array.from(files)
    let done = 0
    for (const file of fileArr) {
      setUploadStatus(`正在上传 ${done + 1}/${fileArr.length} ...`)
      const ok = await uploadPurchaseDocument(file, supplierId, supplier?.name ?? '', date, remark)
      if (ok) done++
    }
    setUploadStatus(`本次上传完成：${done}/${fileArr.length} 张`)
    void queryClient.invalidateQueries({ queryKey: ['purchaseDocuments'] })
  }

  const confirmDelete = (id: number) => {
    modal.confirm({
      title: '确定删除该单据图片？',
      okText: '确定',
      cancelText: '取消',
      onOk: async () => {
        const ok = await deletePurchaseDocument(id)
        if (ok) {
          void queryClient.invalidateQueries({ queryKey: ['purchaseDocuments'] })
        } else {
          message.error('删除失败')
        }
      },
    })
  }

  return (
    <>
      <ReportCard
        title="采购单据列表"
        desc="按供应商、日期采集采购单据图片。支持手机连续拍照/多选上传，图片按供应商+日期分组归档。"
        filters={
          <>
            <FilterItem label="供应商" width={220}>
              <Select
                showSearch
                optionFilterProp="label"
                style={{ width: '100%' }}
                value={supplierId}
                onChange={setSupplierId}
                options={[
                  { value: '', label: '请选择供应商' },
                  ...suppliers.map((s) => ({ value: String(s.id), label: s.name })),
                ]}
              />
            </FilterItem>
            <FilterItem label="单据日期" width={150}>
              <DatePicker<string>
                style={{ width: '100%' }}
                value={date || null}
                onChange={(d) => setDate(d ?? '')}
              />
            </FilterItem>
            <FilterItem label="备注（可选）" width={220}>
              <Input placeholder="如：验收单/送货单" value={remark} onChange={(e) => setRemark(e.target.value)} />
            </FilterItem>
          </>
        }
        actions={
          <Space wrap>
            <Button type="primary" onClick={handleSearch}>
              查询
            </Button>
            <Button onClick={startCapture}>📷 连续采集单据（拍照/多选）</Button>
            {uploadStatus && <span style={{ color: '#999', fontSize: 12 }}>{uploadStatus}</span>}
          </Space>
        }
      >
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          capture="environment"
          multiple
          style={{ display: 'none' }}
          onChange={(e) => {
            void handleFiles(e.target.files)
            e.target.value = ''
          }}
        />
        {docs.length === 0 ? (
          <div style={{ textAlign: 'center', color: '#999', padding: 24 }}>暂无单据</div>
        ) : (
          groups.map(([key, items]) => (
            <div key={key} style={{ marginBottom: 24 }}>
              <div style={{ fontWeight: 600, fontSize: 14, borderBottom: '1px solid #f0f0f0', paddingBottom: 4, marginBottom: 10 }}>
                {key} <Tag color="cyan">{items.length} 张</Tag>
              </div>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 10 }}>
                {items.map((d) => (
                  <div key={d.id} style={{ position: 'relative', width: 120 }}>
                    <img
                      src={d.image_url}
                      alt={d.remark || d.supplier_name}
                      style={{ width: 120, height: 120, objectFit: 'cover', borderRadius: 6, border: '1px solid #ddd', cursor: 'pointer' }}
                      onClick={() => setPreviewUrl(d.image_url)}
                    />
                    <Button
                      danger
                      size="small"
                      style={{ position: 'absolute', top: 2, right: 2, padding: '0 6px' }}
                      onClick={() => confirmDelete(d.id)}
                    >
                      ×
                    </Button>
                    {d.remark && (
                      <div
                        style={{
                          fontSize: 12,
                          color: '#999',
                          width: 120,
                          whiteSpace: 'nowrap',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                        }}
                      >
                        {d.remark}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))
        )}
      </ReportCard>

      {/* 大图预览弹窗：老页面 modal-lg 居中，图片 max-width:100% / max-height:85vh */}
      <Modal open={previewUrl !== null} footer={null} centered width={880} onCancel={() => setPreviewUrl(null)}>
        <div style={{ textAlign: 'center' }}>
          {previewUrl && <img src={previewUrl} alt="单据图片" style={{ maxWidth: '100%', maxHeight: '85vh' }} />}
        </div>
      </Modal>
    </>
  )
}
