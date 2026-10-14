import { useEffect, useMemo, useRef, useState } from 'react'
import {
  App, Button, Card, Checkbox, Col, DatePicker, Input, InputNumber, Modal, Radio, Row, Select, Space, Spin, Table, Tag, Tooltip,
} from 'antd'
import type { TableProps } from 'antd'
import dayjs from 'dayjs'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { listPurchasers } from '../../api/sales'
import {
  cancelAllocation,
  compareSupplement,
  completeAllocation,
  createAllocation,
  createSupplement,
  deleteSupplement,
  getAllocationSummary,
  getSupplementOrderDetail,
  listAllocatedOrders,
  listOrdersByPurchaser,
  listSupplementsBySource,
  listSupplementsByTarget,
  terminateAllocation,
  type AllocatedOrderRow,
  type PendingReplaceLine,
  type SupplementDetailItem,
  type SupplementOpType,
  type SupplementOrder,
  type SupplementProduct,
  type SupplementRecord,
} from '../../api/admin/supplement'
import { CompareTables, ProductSelect } from './SupplementParts'

const fmt = (v: number) => (Number(v) || 0).toFixed(2)

const SUCCESS_GREEN = '#198754'
const DANGER_RED = '#dc3545'

/** 源订单列表的分摊状态文案（含 -1 未初始化） */
const ALLOC_STATUS_TEXT: Record<number, string> = {
  [-1]: '未初始化',
  0: '未分摊',
  1: '分摊中',
  2: '已完成',
  3: '已终止',
}

/** 已分摊订单列表的状态 → 文案/Tag 颜色 */
const ALLOCATED_STATUS_MAP: Record<number, { text: string; color: string | undefined }> = {
  0: { text: '未分摊', color: undefined },
  1: { text: '分摊中', color: 'processing' },
  2: { text: '已完成', color: 'success' },
  3: { text: '已终止', color: 'warning' },
}

/** 增项列表操作类型文案 */
const PENDING_TYPE_TEXT: Record<string, string> = {
  new_item: '新增商品',
  increase_quantity: '追加数量',
  replace_remove: '替换-冲减',
  replace_add: '替换-换入',
}

/** 关联分摊订单操作类型文案 */
const SOURCE_OP_TEXT: Record<string, string> = {
  increase_quantity: '+追加',
  new_item: '+新增',
  replace_remove: '替换-冲减',
  replace_add: '替换-换入',
}

/** 页面内选中的订单引用（老页面仅消费 id/order_no/order_date/total_amount） */
interface SelectedOrderRef {
  id: number
  order_no: string
  order_date: string
  total_amount: number
}

interface PromptState {
  title: string
  value: string
  requireNonEmpty: boolean
  /** 老页面 createAllocation 的 prompt 取消仍会继续提交 */
  proceedOnCancel: boolean
  onOk: (value: string) => void
}

export default function SupplementPage() {
  const { message, modal } = App.useApp()
  const queryClient = useQueryClient()

  // ===== 顶部选择区 =====
  const [purchaserId, setPurchaserId] = useState<number | null>(null)
  const [srcOrder, setSrcOrder] = useState<SelectedOrderRef | null>(null)
  const [tgtOrder, setTgtOrder] = useState<SelectedOrderRef | null>(null)

  // ===== 源订单明细勾选（无方案时可编辑）=====
  const [selectedSourceIds, setSelectedSourceIds] = useState<number[]>([])

  // ===== 增项操作 =====
  const [opType, setOpType] = useState<'new_item' | 'increase_quantity' | 'replace_item'>('new_item')
  const [suppProduct, setSuppProduct] = useState<SupplementProduct | null>(null)
  const [addQty, setAddQty] = useState<number | null>(null)
  const [addPrice, setAddPrice] = useState<number | null>(null)
  const [increaseIdx, setIncreaseIdx] = useState(0)
  const [increaseQty, setIncreaseQty] = useState<number | null>(null)
  const [replaceSourceIdx, setReplaceSourceIdx] = useState(0)
  const [replaceProduct, setReplaceProduct] = useState<SupplementProduct | null>(null)
  const [replaceQty, setReplaceQty] = useState<number | null>(null)
  const [replacePrice, setReplacePrice] = useState<number | null>(null)
  const [replaceLines, setReplaceLines] = useState<PendingReplaceLine[]>([])
  const [allocateDate, setAllocateDate] = useState(dayjs().format('YYYY-MM-DD'))

  // ===== 待保存增项（服务端已保存 + 本地新增）=====
  const [pendingSupplements, setPendingSupplements] = useState<SupplementRecord[]>([])
  const lastSyncedTargetRef = useRef<number | null>(null)
  const [prompt, setPrompt] = useState<PromptState | null>(null)

  // ===== 查询 =====
  const { data: purchasers = [] } = useQuery({ queryKey: ['suppPurchasers'], queryFn: listPurchasers })

  const { data: orders = [], error: ordersError } = useQuery({
    queryKey: ['suppOrders', purchaserId],
    queryFn: () => listOrdersByPurchaser(purchaserId!),
    enabled: purchaserId !== null,
  })

  const { data: srcDetail } = useQuery({
    queryKey: ['suppOrderDetail', srcOrder?.id ?? null],
    queryFn: () => getSupplementOrderDetail(srcOrder!.id),
    enabled: srcOrder !== null,
  })
  const srcItems: SupplementDetailItem[] = srcDetail?.items ?? []

  const { data: tgtDetail } = useQuery({
    queryKey: ['suppOrderDetail', tgtOrder?.id ?? null],
    queryFn: () => getSupplementOrderDetail(tgtOrder!.id),
    enabled: tgtOrder !== null,
  })
  const tgtItems: SupplementDetailItem[] = tgtDetail?.items ?? []

  const { data: allocSummary } = useQuery({
    queryKey: ['suppAllocSummary', srcOrder?.id ?? null],
    queryFn: () => getAllocationSummary(srcOrder!.id),
    enabled: srcOrder !== null,
  })

  const { data: srcSupplements = [] } = useQuery({
    queryKey: ['suppSourceSupp', srcOrder?.id ?? null],
    queryFn: () => listSupplementsBySource(srcOrder!.id),
    enabled: srcOrder !== null,
  })

  const { data: allocatedOrders = [], isFetching: allocatedFetching } = useQuery({
    queryKey: ['suppAllocated'],
    queryFn: listAllocatedOrders,
  })

  const { data: compareData, isFetching: compareFetching } = useQuery({
    queryKey: ['suppCompare', tgtOrder?.id ?? null],
    queryFn: () => compareSupplement(tgtOrder!.id),
    enabled: tgtOrder !== null,
  })

  const { data: targetSupplements } = useQuery({
    queryKey: ['suppTargetSupp', tgtOrder?.id ?? null],
    queryFn: () => listSupplementsByTarget(tgtOrder!.id),
    enabled: tgtOrder !== null,
  })

  const hasScheme = !!allocSummary?.exists
  const schemeItemIds = hasScheme ? allocSummary?.source_item_ids ?? [] : []

  // ===== 数据同步（照抄 loadTargetSupplements：选中目标订单时整体替换，已保存记录带「已保存」标记）=====
  useEffect(() => {
    const orderId = tgtOrder?.id ?? null
    if (orderId !== lastSyncedTargetRef.current) {
      lastSyncedTargetRef.current = orderId
      setPendingSupplements(targetSupplements ?? [])
    } else if (!pendingSupplements.some((s) => !s.id)) {
      // 无未保存条目时跟随服务端数据刷新
      setPendingSupplements(targetSupplements ?? [])
    }
  }, [targetSupplements, tgtOrder?.id, pendingSupplements])

  // 无方案时源明细默认全选（全选=整单分摊）
  useEffect(() => {
    setSelectedSourceIds(srcItems.map((i) => i.id))
  }, [srcDetail, hasScheme])

  // 切换目标订单/操作类型时重置下拉（照抄 initIncreaseProductSelect / initReplaceSourceSelect 的重建行为）
  useEffect(() => {
    setIncreaseIdx(0)
    setReplaceSourceIdx(0)
  }, [tgtDetail])
  useEffect(() => {
    setReplaceSourceIdx(0)
  }, [opType])

  // ===== 派生值 =====
  const selectedSourceTotal = useMemo(
    () => srcItems.filter((i) => selectedSourceIds.includes(i.id)).reduce((s, i) => s + (i.amount || 0), 0),
    [srcItems, selectedSourceIds],
  )

  const pendingUnsaved = useMemo(() => pendingSupplements.filter((s) => !s.id), [pendingSupplements])

  // 余额提示（照抄 updateBalanceWarning：冲减负数不释放余额）
  const balance = useMemo(() => {
    const pendingSum = pendingUnsaved.reduce((sum, s) => sum + Math.max(s.amount, 0), 0)
    const total = allocSummary?.total_amount ?? 0
    const allocated = allocSummary?.allocated_amount ?? 0
    const remainingBalance = allocSummary?.remaining_balance ?? 0
    const projected = allocated + pendingSum
    const over = projected - total
    if (Math.abs(over) > 0.005) {
      return {
        over,
        color: '#6c757d',
        text: `耗材总额 ${fmt(total)}｜已分摊 ${fmt(allocated)}｜剩余 ${fmt(remainingBalance)}｜本次 ${fmt(pendingSum)}｜预计总分摊 ${fmt(projected)}（${over > 0 ? '超额' : '结余'} ${fmt(Math.abs(over))} 元，保存时校验）`,
      }
    }
    return {
      over,
      color: SUCCESS_GREEN,
      text: `耗材总额 ${fmt(total)}｜已分摊 ${fmt(allocated)}｜本次 ${fmt(pendingSum)}｜预计总分摊 ${fmt(projected)}（平衡）`,
    }
  }, [pendingUnsaved, allocSummary])

  // 替换差额提示（照抄 updateReplaceDiffHint）
  const replaceDiffHint = useMemo(() => {
    const src = tgtItems[replaceSourceIdx]
    if (!src) return null
    const origAmount = src.amount
    const replaceTotal = replaceLines.reduce((s, l) => s + l.amount, 0)
    const diff = replaceTotal - origAmount
    if (Math.abs(diff) <= 5.0) {
      return { color: SUCCESS_GREEN, text: `差额 ${fmt(diff)} 元（在±5元内，可提交）` }
    }
    return { color: DANGER_RED, text: `差额 ${fmt(diff)} 元（超过±5元限制）` }
  }, [tgtItems, replaceSourceIdx, replaceLines])

  const increaseItem = tgtItems[increaseIdx]
  const increasePrice = increaseItem?.unit_price ?? 0

  // ===== 选择与刷新 =====
  const refreshSourceRelated = () => {
    if (srcOrder) {
      void queryClient.invalidateQueries({ queryKey: ['suppAllocSummary', srcOrder.id] })
      void queryClient.invalidateQueries({ queryKey: ['suppSourceSupp', srcOrder.id] })
    }
    if (purchaserId !== null) void queryClient.invalidateQueries({ queryKey: ['suppOrders', purchaserId] })
    void queryClient.invalidateQueries({ queryKey: ['suppAllocated'] })
  }

  const resetOrderSelection = () => {
    setSrcOrder(null)
    setTgtOrder(null)
    setSuppProduct(null)
    setPendingSupplements([])
    lastSyncedTargetRef.current = null
  }

  const selectPurchaser = (id: number) => {
    setPurchaserId(id)
    resetOrderSelection()
  }

  const selectConsumableOrder = (o: SupplementOrder) => {
    setSrcOrder({ id: o.id, order_no: o.order_no, order_date: o.order_date, total_amount: o.total_amount })
  }

  const selectTargetOrder = (o: SupplementOrder) => {
    setTgtOrder({ id: o.id, order_no: o.order_no, order_date: o.order_date, total_amount: o.total_amount })
  }

  /** 从「已分摊订单列表」定位：自动切换采购单位并选中源订单（照抄 selectAllocatedOrder） */
  const selectAllocatedOrder = (o: AllocatedOrderRow) => {
    if (o.purchaser_id && o.purchaser_id !== purchaserId) {
      setPurchaserId(o.purchaser_id)
    }
    const cached = o.purchaser_id
      ? queryClient.getQueryData<SupplementOrder[]>(['suppOrders', o.purchaser_id])
      : undefined
    const found = cached?.find((x) => x.id === o.source_order_id)
    setSrcOrder(
      found
        ? { id: found.id, order_no: found.order_no, order_date: found.order_date, total_amount: found.total_amount }
        : { id: o.source_order_id, order_no: o.order_no || '', order_date: o.order_date || '', total_amount: o.total_amount || 0 },
    )
  }

  // ===== 分摊方案操作 =====
  const doCreateAllocation = async (remark: string) => {
    if (!srcOrder) return
    const ids = selectedSourceIds
    if (ids.length === 0) {
      message.warning('请至少勾选一条要分摊的明细')
      return
    }
    const res = await createAllocation({ source_order_id: srcOrder.id, source_item_ids: ids, remark: remark || '' })
    if (res.ok) {
      message.success('分摊方案创建成功')
      refreshSourceRelated()
    } else {
      message.error(`创建失败: ${res.message}`)
    }
  }

  const openCreateAllocation = () => {
    if (!srcOrder) return
    if (selectedSourceIds.length === 0) {
      message.warning('请至少勾选一条要分摊的明细')
      return
    }
    setPrompt({ title: '请输入分摊方案备注（选填）', value: '', requireNonEmpty: false, proceedOnCancel: true, onOk: (v) => void doCreateAllocation(v) })
  }

  const doTerminate = async (remark: string) => {
    if (!srcOrder) return
    const res = await terminateAllocation({ source_order_id: srcOrder.id, remark })
    if (res.ok) {
      message.success('终止成功')
      refreshSourceRelated()
    } else {
      message.error(res.message)
    }
  }

  const openTerminate = () => {
    setPrompt({ title: '请输入终止原因', value: '', requireNonEmpty: true, proceedOnCancel: false, onOk: (v) => void doTerminate(v) })
  }

  const doCancelAllocation = async () => {
    if (!srcOrder) return
    const res = await cancelAllocation({ source_order_id: srcOrder.id })
    if (res.ok) {
      message.success('取消成功，已回到未分摊状态')
      refreshSourceRelated()
    } else {
      message.error(`取消失败: ${res.message}`)
    }
  }

  const cancelAllocationAction = () => {
    if (!srcOrder) return
    if (!allocSummary || !allocSummary.exists) {
      message.warning('当前订单未初始化分摊方案')
      return
    }
    if ((allocSummary.allocated_amount || 0) > 0.0001) {
      message.warning('已存在分摊记录，请先回滚全部分摊后再取消')
      return
    }
    modal.confirm({
      title: '确定取消该分摊方案？取消后订单将回到未分摊状态，可重新选择明细。',
      onOk: () => void doCancelAllocation(),
    })
  }

  const doComplete = async (targetId: number | null, autoTail: boolean) => {
    if (!srcOrder) return
    try {
      const res = await completeAllocation({ source_order_id: srcOrder.id, target_order_id: targetId, auto_tail: autoTail })
      if (res.ok) {
        let autoTailResp = false
        try {
          autoTailResp = (JSON.parse(res.message) as { auto_tail?: boolean }).auto_tail ?? false
        } catch {
          // 非 JSON 文本
        }
        message.success(autoTailResp ? '分摊完成，已自动创建尾差冲销项。' : '分摊完成！')
        refreshSourceRelated()
      } else {
        message.error(res.message)
      }
    } catch (e) {
      message.error(`操作失败: ${e instanceof Error ? e.message : e}`)
    }
  }

  const confirmCompleteAllocation = () => {
    if (!srcOrder || !allocSummary || !allocSummary.exists) {
      message.warning('请先初始化分摊方案')
      return
    }
    if (allocSummary.status === 2) {
      message.warning('分摊方案已完成')
      return
    }
    if (allocSummary.status === 3) {
      message.warning('分摊方案已终止')
      return
    }
    const remaining = allocSummary.remaining_balance
    const threshold = 5.0
    if (remaining > threshold) {
      message.warning(`剩余 ${fmt(remaining)} 元未分摊，超过尾差限额（±${fmt(threshold)} 元），请继续分摊完成后确认。`)
      return
    }
    if (remaining < -threshold) {
      message.warning(`已超额分摊 ${fmt(Math.abs(remaining))} 元，超过尾差限额（±${fmt(threshold)} 元），请回滚部分分摊后再确认。`)
      return
    }
    let targetId: number | null = null
    let autoTail = false
    if (Math.abs(remaining) > 0.01) {
      autoTail = true
      if (tgtOrder) {
        targetId = tgtOrder.id
      } else if (orders.length > 0) {
        targetId = orders[0].id
      } else {
        message.warning('没有可用的目标订单来创建尾差冲销项')
        return
      }
      modal.confirm({
        title: '确认完成分摊',
        content: (
          <div style={{ whiteSpace: 'pre-line' }}>
            {`尾差 ${fmt(remaining)} 元（限额 ±${fmt(threshold)} 元），\n系统将自动在已选目标订单中创建一笔"分摊尾差"冲销项。是否继续？`}
          </div>
        ),
        onOk: () => void doComplete(targetId, autoTail),
      })
    } else {
      void doComplete(null, false)
    }
  }

  const rollbackSupplement = (supp: SupplementRecord) => {
    modal.confirm({
      title: '确定回滚该分摊项？',
      onOk: async () => {
        if (supp.id == null) return
        const res = await deleteSupplement(supp.id)
        if (res.ok) {
          message.success('回滚成功')
          refreshSourceRelated()
        } else {
          message.error(`回滚失败: ${res.message}`)
        }
      },
    })
  }

  // ===== 增项操作 =====
  const checkAllocationEditable = (): boolean => {
    if (!srcOrder || !tgtOrder) {
      message.warning('请先选择耗材订单和目标订单')
      return false
    }
    if (!allocSummary || !allocSummary.exists) {
      message.warning('请先初始化分摊方案')
      return false
    }
    if (allocSummary.status === 2) {
      message.warning('分摊已完成，不可继续分摊')
      return false
    }
    if (allocSummary.status === 3) {
      message.warning('分摊已终止')
      return false
    }
    return true
  }

  const addSupplement = () => {
    if (!checkAllocationEditable()) return
    if (!srcOrder || !tgtOrder) return

    let qty: number
    let amount: number
    let productId: number
    let productName: string
    let alias1: string
    let alias2: string
    let spec: string
    let unit: string
    let unitPrice: number
    let targetItemId: number | null

    if (opType === 'new_item') {
      if (!suppProduct) {
        message.warning('请选择要增项的商品')
        return
      }
      qty = addQty || 0
      unitPrice = addPrice || 0
      amount = qty * unitPrice
      if (qty <= 0 || amount <= 0) {
        message.warning('请输入有效的增项数量和单价')
        return
      }
      productId = suppProduct.id
      productName = suppProduct.name
      alias1 = suppProduct.alias1 || ''
      alias2 = suppProduct.alias2 || ''
      spec = suppProduct.spec || ''
      unit = suppProduct.unit || ''
      targetItemId = null
    } else {
      const item = tgtItems[increaseIdx]
      if (!item) {
        message.warning('请选择要追加的商品')
        return
      }
      qty = increaseQty || 0
      unitPrice = item.unit_price
      amount = qty * unitPrice
      if (qty <= 0) {
        message.warning('请输入有效的追加数量')
        return
      }
      productId = item.product_id
      productName = item.product_name
      alias1 = item.alias1 || ''
      alias2 = item.alias2 || ''
      spec = item.spec || ''
      unit = item.unit || ''
      targetItemId = item.id
    }

    // 添加时不立即校验金额，允许组合多条增项；统一在「保存增项」时校验差额（上下 5 元）
    setPendingSupplements((list) => [
      ...list,
      {
        id: null,
        source_order_id: srcOrder.id,
        target_order_id: tgtOrder.id,
        source_remark: srcOrder.order_no + ' 耗材分摊',
        product_id: productId,
        product_name: productName,
        alias1,
        alias2,
        spec,
        unit,
        unit_price: unitPrice,
        quantity: qty,
        amount,
        allocate_date: allocateDate,
        operation_type: opType,
        target_order_item_id: targetItemId,
      },
    ])
    setAddQty(null)
    setIncreaseQty(null)
  }

  const addReplaceLine = () => {
    if (!replaceProduct) {
      message.warning('请选择替换的商品')
      return
    }
    const qty = replaceQty || 0
    const price = replacePrice || 0
    const amount = qty * price
    if (qty <= 0 || amount <= 0) {
      message.warning('请输入有效的数量和单价')
      return
    }
    setReplaceLines((lines) => [
      ...lines,
      {
        product_id: replaceProduct.id,
        product_name: replaceProduct.name,
        alias1: replaceProduct.alias1 || '',
        alias2: replaceProduct.alias2 || '',
        spec: replaceProduct.spec || '',
        unit: replaceProduct.unit || '',
        unit_price: price,
        quantity: qty,
        amount,
      },
    ])
    setReplaceProduct(null)
    setReplaceQty(null)
    setReplacePrice(null)
  }

  const removeReplaceLine = (index: number) => {
    setReplaceLines((lines) => lines.filter((_, i) => i !== index))
  }

  const addReplacement = () => {
    if (!checkAllocationEditable()) return
    if (!srcOrder || !tgtOrder) return
    const src = tgtItems[replaceSourceIdx]
    if (!src) {
      message.warning('请选择被替换的原明细')
      return
    }
    if (replaceLines.length === 0) {
      message.warning('请至少添加一条替换商品')
      return
    }

    const srcRemark = `${srcOrder.order_no} 替换[${src.product_name}]`

    // 冲减原明细：负数记录（不释放余额）
    const removal: SupplementRecord = {
      id: null,
      source_order_id: srcOrder.id,
      target_order_id: tgtOrder.id,
      source_remark: srcRemark + ' 冲减',
      product_id: src.product_id,
      product_name: src.product_name,
      alias1: src.alias1 || '',
      alias2: src.alias2 || '',
      spec: src.spec || '',
      unit: src.unit || '',
      unit_price: src.unit_price,
      quantity: -src.quantity,
      amount: -src.amount,
      allocate_date: allocateDate,
      operation_type: 'replace_remove',
      target_order_item_id: src.id,
    }
    // 新增替换商品：正数记录
    const additions: SupplementRecord[] = replaceLines.map((line) => ({
      id: null,
      source_order_id: srcOrder.id,
      target_order_id: tgtOrder.id,
      source_remark: srcRemark + ' 换入',
      product_id: line.product_id,
      product_name: line.product_name,
      alias1: line.alias1,
      alias2: line.alias2,
      spec: line.spec,
      unit: line.unit,
      unit_price: line.unit_price,
      quantity: line.quantity,
      amount: line.amount,
      allocate_date: allocateDate,
      operation_type: 'replace_add',
      target_order_item_id: src.id,
    }))

    setPendingSupplements((list) => [...list, removal, ...additions])
    setReplaceLines([])
  }

  const dispatchAddOperation = () => {
    if (opType === 'replace_item') {
      addReplacement()
    } else {
      addSupplement()
    }
  }

  const removePendingSupplement = (index: number) => {
    setPendingSupplements((list) => list.filter((_, i) => i !== index))
  }

  const saveAllSupplements = async () => {
    const toSave = pendingSupplements.filter((s) => !s.id)
    if (toSave.length === 0) {
      message.warning('没有待保存的增项')
      return
    }
    // 保存前校验分摊总额（上下 5 元尾差）；冲减负数不释放余额，仅拦截超额分摊
    const pendingSum = toSave.reduce((sum, s) => sum + Math.max(s.amount, 0), 0)
    const totalAmount = allocSummary?.total_amount ?? 0
    const allocatedAmount = allocSummary?.allocated_amount ?? 0
    const remainingBalance = allocSummary?.remaining_balance ?? 0
    const projected = allocatedAmount + pendingSum
    const diff = projected - totalAmount
    if (diff > 5.0) {
      modal.warning({
        title: '提示',
        content: (
          <div style={{ whiteSpace: 'pre-line' }}>
            {`保存失败：预计总分摊金额超出耗材总额 ${fmt(diff)} 元（超出±5元限制）。\n\n耗材总额: ${fmt(totalAmount)} 元\n已分摊: ${fmt(allocatedAmount)} 元\n剩余余额: ${fmt(remainingBalance)} 元\n本次待保存: ${fmt(pendingSum)} 元\n预计总分摊: ${fmt(projected)} 元\n超额: ${fmt(diff)} 元\n\n请调整增项后再保存。`}
          </div>
        ),
      })
      return
    }
    for (const item of toSave) {
      // 与老页面一致：逐条提交，不因单条失败中断
      await createSupplement(item)
    }
    message.success('增项保存成功')
    // 保存成功后立即清空 pending 列表，避免残留条目被重复计入或重复保存
    setPendingSupplements([])
    if (srcOrder) {
      void queryClient.invalidateQueries({ queryKey: ['suppAllocSummary', srcOrder.id] })
      void queryClient.invalidateQueries({ queryKey: ['suppSourceSupp', srcOrder.id] })
    }
    if (tgtOrder) {
      void queryClient.invalidateQueries({ queryKey: ['suppCompare', tgtOrder.id] })
      void queryClient.invalidateQueries({ queryKey: ['suppTargetSupp', tgtOrder.id] })
    }
    if (purchaserId !== null) void queryClient.invalidateQueries({ queryKey: ['suppOrders', purchaserId] })
  }

  // ===== 表格列定义 =====
  const allocatedColumns: TableProps<AllocatedOrderRow>['columns'] = [
    {
      title: '订单号',
      dataIndex: 'order_no',
      width: 150,
      render: (v: string, o) => <a onClick={() => selectAllocatedOrder(o)}>{v || ''}</a>,
    },
    { title: '采购单位', dataIndex: 'purchaser_name', width: 120 },
    { title: '订单日期', dataIndex: 'order_date', width: 100 },
    { title: '总金额', dataIndex: 'total_amount', width: 90, align: 'right', render: (v: number) => fmt(v) },
    { title: '已分摊', dataIndex: 'allocated_amount', width: 90, align: 'right', render: (v: number) => <span style={{ color: SUCCESS_GREEN }}>{fmt(v)}</span> },
    { title: '剩余余额', dataIndex: 'remaining_balance', width: 90, align: 'right', render: (v: number) => <span style={{ color: DANGER_RED }}>{fmt(v)}</span> },
    {
      title: '状态',
      dataIndex: 'status',
      width: 80,
      render: (v: number) => {
        const st = ALLOCATED_STATUS_MAP[v] ?? { text: '未知', color: undefined }
        return <Tag color={st.color}>{st.text}</Tag>
      },
    },
    { title: '创建时间', dataIndex: 'created_at', width: 150 },
    { title: '完成时间', dataIndex: 'completed_at', width: 150 },
    {
      title: '操作',
      key: 'action',
      width: 70,
      render: (_v, o) => (
        <Button size="small" onClick={() => selectAllocatedOrder(o)}>
          查看
        </Button>
      ),
    },
  ]

  const srcListColumns: TableProps<SupplementOrder>['columns'] = [
    { title: '订单号', dataIndex: 'order_no', width: 150 },
    { title: '日期', dataIndex: 'order_date', width: 100 },
    { title: '金额', dataIndex: 'total_amount', width: 90, align: 'right', render: (v: number) => fmt(v) },
    {
      title: '状态',
      dataIndex: 'allocation_status',
      width: 90,
      render: (v: number) => ALLOC_STATUS_TEXT[v] ?? '未知',
    },
  ]

  const tgtListColumns: TableProps<SupplementOrder>['columns'] = [
    { title: '订单号', dataIndex: 'order_no', width: 150 },
    { title: '日期', dataIndex: 'order_date', width: 100 },
    { title: '金额', dataIndex: 'total_amount', width: 90, align: 'right', render: (v: number) => fmt(v) },
    {
      title: '订单状态',
      dataIndex: 'status',
      width: 90,
      render: (v: string) => (['accepted', 'settled'].includes(v) ? '已完成' : '未完成'),
    },
  ]

  const srcDetailColumns: TableProps<SupplementDetailItem>['columns'] = [
    {
      title: '选',
      width: 36,
      align: 'center',
      render: (_v, item) => (
        <Checkbox
          disabled={hasScheme}
          checked={hasScheme ? schemeItemIds.includes(item.id) : selectedSourceIds.includes(item.id)}
          onChange={(e) =>
            setSelectedSourceIds((ids) => (e.target.checked ? [...ids, item.id] : ids.filter((x) => x !== item.id)))
          }
        />
      ),
    },
    { title: '商品', dataIndex: 'product_name' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '-' },
    { title: '单位', dataIndex: 'unit', width: 60 },
    { title: '数量', dataIndex: 'quantity', width: 70, align: 'right', render: (v: number) => fmt(v) },
    { title: '金额', dataIndex: 'amount', width: 80, align: 'right', render: (v: number) => fmt(v) },
    { title: '类别', dataIndex: 'category_name', width: 90, render: (v: string | null | undefined) => v || '' },
  ]

  const tgtDetailColumns: TableProps<SupplementDetailItem>['columns'] = [
    { title: '商品', dataIndex: 'product_name' },
    { title: '规格', dataIndex: 'spec', render: (v: string | null) => v || '-' },
    { title: '单位', dataIndex: 'unit', width: 60 },
    { title: '数量', dataIndex: 'quantity', width: 70, align: 'right', render: (v: number) => fmt(v) },
    { title: '金额', dataIndex: 'amount', width: 80, align: 'right', render: (v: number) => fmt(v) },
  ]

  const srcSuppColumns: TableProps<SupplementRecord>['columns'] = [
    { title: '目标订单', dataIndex: 'target_order_no', width: 150, render: (v: string | undefined) => v || '未知' },
    {
      title: '商品',
      dataIndex: 'product_name',
      render: (v: string, o) => `${SOURCE_OP_TEXT[o.operation_type] ?? '+新增'} ${v}`,
    },
    { title: '数量', dataIndex: 'quantity', width: 80, align: 'right', render: (v: number) => fmt(v) },
    { title: '金额', dataIndex: 'amount', width: 90, align: 'right', render: (v: number) => fmt(v) },
    {
      title: '操作',
      key: 'action',
      width: 80,
      render: (_v, o) => (
        <Button size="small" danger type="primary" onClick={() => rollbackSupplement(o)}>
          回滚
        </Button>
      ),
    },
  ]

  const replaceLineColumns: TableProps<PendingReplaceLine>['columns'] = [
    { title: '替换为商品', dataIndex: 'product_name', render: (v: string, o) => `${v}${o.alias2 ? `(${o.alias2})` : ''}` },
    { title: '数量', dataIndex: 'quantity', width: 80, align: 'right', render: (v: number) => fmt(v) },
    { title: '单价', dataIndex: 'unit_price', width: 80, align: 'right', render: (v: number) => fmt(v) },
    { title: '金额', dataIndex: 'amount', width: 90, align: 'right', render: (v: number) => fmt(v) },
    {
      title: '操作',
      key: 'action',
      width: 80,
      render: (_v, _o, index) => (
        <Button size="small" danger type="primary" onClick={() => removeReplaceLine(index)}>
          删除
        </Button>
      ),
    },
  ]

  const pendingColumns: TableProps<SupplementRecord>['columns'] = [
    {
      title: '操作类型',
      dataIndex: 'operation_type',
      width: 140,
      render: (v: SupplementOpType, o) => (
        <span>
          {PENDING_TYPE_TEXT[v] ?? v}
          {o.id ? (
            <Tag color="processing" style={{ marginLeft: 4 }}>
              已保存
            </Tag>
          ) : null}
        </span>
      ),
    },
    { title: '商品名称', dataIndex: 'product_name' },
    { title: '数量', dataIndex: 'quantity', width: 80, align: 'right', render: (v: number) => fmt(v) },
    { title: '金额', dataIndex: 'amount', width: 90, align: 'right', render: (v: number) => fmt(v) },
    {
      title: '来源订单',
      dataIndex: 'source_order_no',
      width: 150,
      render: (v: string | undefined) => v || srcOrder?.order_no || '',
    },
    {
      title: '操作',
      key: 'action',
      width: 70,
      render: (_v, _o, index) =>
        !_o.id ? (
          <Button size="small" danger type="primary" onClick={() => removePendingSupplement(index)}>
            删除
          </Button>
        ) : null,
    },
  ]

  // ===== 提示弹窗（prompt 的 AntD 表达）=====
  const handlePromptOk = () => {
    const p = prompt
    if (!p) return
    if (p.requireNonEmpty && !p.value.trim()) return
    setPrompt(null)
    p.onOk(p.value)
  }
  const handlePromptCancel = () => {
    const p = prompt
    if (!p) return
    setPrompt(null)
    if (p.proceedOnCancel) p.onOk(p.value)
  }

  const summaryStatusText = allocSummary ? ALLOC_STATUS_TEXT[allocSummary.status] ?? '未知' : '未分摊'
  const summaryStatusColor =
    allocSummary?.status === 2 ? 'success' : allocSummary?.status === 3 ? 'warning' : allocSummary?.status === 1 ? 'processing' : undefined
  const replaceLinesTotal = replaceLines.reduce((s, l) => s + l.amount, 0)

  return (
    <>
      <Card>
        <h4 style={{ marginTop: 0 }}>耗材分摊管理</h4>

        {/* 顶部选择区 */}
        <Row gutter={16}>
          <Col md={8} xs={24}>
            <div style={{ marginBottom: 4 }}>采购单位</div>
            <Select
              style={{ width: '100%' }}
              showSearch
              optionFilterProp="label"
              placeholder="单击选择 / 双击搜索"
              value={purchaserId ?? undefined}
              onChange={(v) => selectPurchaser(v)}
              options={purchasers.map((p) => ({ value: p.id, label: p.name }))}
            />
          </Col>
          <Col md={8} xs={24}>
            <div style={{ marginBottom: 4 }}>耗材订单（来源）</div>
            <Select
              style={{ width: '100%' }}
              showSearch
              placeholder="单击选择 / 双击搜索"
              value={srcOrder ? { value: srcOrder.id, label: srcOrder.order_no } : undefined}
              onChange={(_v, opt) => selectConsumableOrder((opt as { order: SupplementOrder }).order)}
              filterOption={(input, option) =>
                String((option as { orderNo?: string })?.orderNo ?? '').toLowerCase().includes(input.toLowerCase())
              }
              options={orders.map((o) => ({ value: o.id, label: o.order_no, orderNo: o.order_no, order: o }))}
            />
          </Col>
          <Col md={8} xs={24}>
            <div style={{ marginBottom: 4 }}>目标订单（分摊到）</div>
            <Select
              style={{ width: '100%' }}
              showSearch
              placeholder="单击选择 / 双击搜索"
              value={tgtOrder ? { value: tgtOrder.id, label: tgtOrder.order_no } : undefined}
              onChange={(_v, opt) => selectTargetOrder((opt as { order: SupplementOrder }).order)}
              filterOption={(input, option) =>
                String((option as { orderNo?: string })?.orderNo ?? '').toLowerCase().includes(input.toLowerCase())
              }
              options={orders.map((o) => ({ value: o.id, label: o.order_no, orderNo: o.order_no, order: o }))}
            />
          </Col>
        </Row>

        {ordersError && (
          <div style={{ color: DANGER_RED, marginTop: 8 }}>加载订单失败: {String((ordersError as Error).message)}</div>
        )}

        {/* 已进入分摊的订单列表 */}
        <div style={{ marginTop: 16 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
            <span style={{ fontWeight: 600 }}>
              已进入分摊的订单列表 <Tag color="processing">{allocatedOrders.length}</Tag>
            </span>
            <Button size="small" onClick={() => void queryClient.invalidateQueries({ queryKey: ['suppAllocated'] })}>
              刷新
            </Button>
          </div>
          <Table<AllocatedOrderRow>
            rowKey="source_order_id"
            size="small"
            bordered
            loading={allocatedFetching}
            columns={allocatedColumns}
            dataSource={allocatedOrders}
            pagination={false}
            scroll={{ y: 220 }}
            locale={{ emptyText: '暂无' }}
          />
        </div>

        {/* 下半区：源订单 / 目标订单 */}
        <Row gutter={16} style={{ marginTop: 24 }}>
          {/* 左：可分摊源订单 */}
          <Col md={12} xs={24}>
            <Card
              size="small"
              title={
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span>可分摊源订单列表</span>
                  {hasScheme && <Tag color={summaryStatusColor}>{summaryStatusText}</Tag>}
                </div>
              }
              styles={{ header: { background: DANGER_RED, color: '#fff' } }}
            >
              {allocSummary?.exists && (
                <div style={{ background: '#fff3f3', padding: 8, marginBottom: 8 }}>
                  <Row gutter={8}>
                    <Col md={6} xs={12} style={{ textAlign: 'center' }}>
                      <div style={{ fontSize: 12, color: '#6c757d' }}>总金额</div>
                      <div style={{ fontWeight: 700 }}>{fmt(allocSummary.total_amount)}</div>
                    </Col>
                    <Col md={6} xs={12} style={{ textAlign: 'center' }}>
                      <div style={{ fontSize: 12, color: '#6c757d' }}>已分摊</div>
                      <div style={{ fontWeight: 700, color: SUCCESS_GREEN }}>{fmt(allocSummary.allocated_amount)}</div>
                    </Col>
                    <Col md={6} xs={12} style={{ textAlign: 'center' }}>
                      <div style={{ fontSize: 12, color: '#6c757d' }}>未分摊余额</div>
                      <div style={{ fontWeight: 700, color: DANGER_RED }}>{fmt(allocSummary.remaining_balance)}</div>
                    </Col>
                    <Col md={6} xs={12} style={{ textAlign: 'center' }}>
                      <div style={{ fontSize: 12, color: '#6c757d' }}>分摊状态</div>
                      <div style={{ fontWeight: 700 }}>{summaryStatusText}</div>
                    </Col>
                  </Row>
                </div>
              )}

              <div style={{ height: 250, overflowY: 'auto' }}>
                <Table<SupplementOrder>
                  rowKey="id"
                  size="small"
                  bordered
                  columns={srcListColumns}
                  dataSource={orders}
                  pagination={false}
                  onRow={(o) => ({ style: { cursor: 'pointer' }, onClick: () => selectConsumableOrder(o) })}
                />
              </div>

              {/* 源订单明细 + 关联分摊订单 + 方案操作 */}
              <div style={{ marginTop: 8 }}>
                {srcOrder ? (
                  <>
                    <div style={{ fontWeight: 600, marginBottom: 4 }}>{srcOrder.order_no} 明细</div>
                    <Table<SupplementDetailItem>
                      rowKey="id"
                      size="small"
                      bordered
                      columns={srcDetailColumns}
                      dataSource={srcItems}
                      pagination={false}
                      onRow={(item) =>
                        hasScheme && schemeItemIds.includes(item.id) ? { style: { backgroundColor: '#e8f4ff' } } : {}
                      }
                    />
                    {!hasScheme && (
                      <div style={{ marginBottom: 8, fontSize: 12, color: '#6c757d' }}>
                        已选明细合计: {fmt(selectedSourceTotal)} 元（全选=整单分摊）
                      </div>
                    )}

                    <div id="allocationOrdersSection">
                      {srcSupplements.length === 0 ? (
                        <p style={{ color: '#6c757d', fontSize: 12 }}>暂无分摊订单记录</p>
                      ) : (
                        <>
                          <div style={{ fontWeight: 600, margin: '8px 0 4px' }}>
                            关联分摊订单（<span style={{ color: DANGER_RED, fontSize: 12 }}>可回滚</span>）
                          </div>
                          <div style={{ maxHeight: 200, overflowY: 'auto' }}>
                            <Table<SupplementRecord>
                              rowKey="id"
                              size="small"
                              bordered
                              columns={srcSuppColumns}
                              dataSource={srcSupplements}
                              pagination={false}
                            />
                          </div>
                        </>
                      )}
                    </div>

                    <div style={{ marginTop: 8 }}>
                      {!hasScheme && (
                        <Button type="primary" size="small" onClick={openCreateAllocation}>
                          初始化分摊方案
                        </Button>
                      )}
                      {hasScheme && allocSummary?.status !== 2 && allocSummary?.status !== 3 && (
                        <Space wrap size={4}>
                          <Button
                            size="small"
                            type="primary"
                            style={{ background: SUCCESS_GREEN, borderColor: SUCCESS_GREEN }}
                            onClick={confirmCompleteAllocation}
                          >
                            确认完成分摊
                          </Button>
                          <Button
                            size="small"
                            style={{ background: '#ffc107', borderColor: '#ffc107', color: '#212529' }}
                            onClick={openTerminate}
                          >
                            终止分摊
                          </Button>
                          <Tooltip title="仅在未产生分摊记录时可用；取消后回到未分摊状态">
                            <Button size="small" onClick={cancelAllocationAction}>
                              取消分摊方案
                            </Button>
                          </Tooltip>
                        </Space>
                      )}
                    </div>
                  </>
                ) : (
                  <p>请选择订单查看详情</p>
                )}
              </div>
            </Card>
          </Col>

          {/* 右：目标订单 */}
          <Col md={12} xs={24}>
            <Card size="small" title="目标订单列表（可分摊到）" styles={{ header: { background: SUCCESS_GREEN, color: '#fff' } }}>
              <div style={{ height: 250, overflowY: 'auto' }}>
                <Table<SupplementOrder>
                  rowKey="id"
                  size="small"
                  bordered
                  columns={tgtListColumns}
                  dataSource={orders}
                  pagination={false}
                  onRow={(o) => ({ style: { cursor: 'pointer' }, onClick: () => selectTargetOrder(o) })}
                />
              </div>

              <div style={{ marginTop: 8 }}>
                {tgtOrder ? (
                  <>
                    <div style={{ fontWeight: 600, marginBottom: 4 }}>{tgtOrder.order_no} 明细</div>
                    <Table<SupplementDetailItem>
                      rowKey="id"
                      size="small"
                      bordered
                      columns={tgtDetailColumns}
                      dataSource={tgtItems}
                      pagination={false}
                    />

                    {/* 真实账套 vs 分摊账套 对比 */}
                    <div style={{ marginTop: 12 }}>
                      <div style={{ borderTop: '1px solid #eee', paddingTop: 8 }}>
                        <div style={{ fontWeight: 600, marginBottom: 4 }}>真实账套 vs 分摊账套 对比</div>
                        {compareFetching && !compareData ? (
                          <Spin />
                        ) : compareData ? (
                          <CompareTables compare={compareData} pendingUnsaved={pendingUnsaved} />
                        ) : null}
                      </div>
                    </div>

                    {/* 增项操作 */}
                    <div style={{ marginTop: 12 }}>
                      <div style={{ borderTop: '1px solid #eee', paddingTop: 8 }}>
                        <div style={{ fontWeight: 600, marginBottom: 8 }}>增项操作</div>
                        <Radio.Group
                          value={opType}
                          onChange={(e) => setOpType(e.target.value as 'new_item' | 'increase_quantity' | 'replace_item')}
                          style={{ marginBottom: 8 }}
                        >
                          <Radio value="new_item">新增商品行</Radio>
                          <Radio value="increase_quantity">追加已有商品数量</Radio>
                          <Radio value="replace_item">替换明细</Radio>
                        </Radio.Group>

                        {opType === 'new_item' && (
                          <Row gutter={8} style={{ marginBottom: 8 }}>
                            <Col md={12} xs={24}>
                              <div style={{ marginBottom: 2 }}>选择商品</div>
                              <ProductSelect value={suppProduct} onChange={setSuppProduct} />
                            </Col>
                            <Col md={4} xs={8}>
                              <div style={{ marginBottom: 2 }}>增项数量</div>
                              <InputNumber style={{ width: '100%' }} step={0.01} value={addQty} onChange={(v) => setAddQty(v)} />
                            </Col>
                            <Col md={4} xs={8}>
                              <div style={{ marginBottom: 2 }}>单价</div>
                              <InputNumber style={{ width: '100%' }} step={0.01} value={addPrice} onChange={(v) => setAddPrice(v)} />
                            </Col>
                            <Col md={4} xs={8}>
                              <div style={{ marginBottom: 2 }}>增项金额</div>
                              <Input readOnly value={fmt((addQty || 0) * (addPrice || 0))} />
                            </Col>
                          </Row>
                        )}

                        {opType === 'increase_quantity' && (
                          <Row gutter={8} style={{ marginBottom: 8 }}>
                            <Col md={8} xs={24}>
                              <div style={{ marginBottom: 2 }}>选择目标商品</div>
                              <Select
                                style={{ width: '100%' }}
                                value={increaseItem ? increaseIdx : undefined}
                                onChange={(v) => setIncreaseIdx(v)}
                                options={tgtItems.map((item, index) => ({
                                  value: index,
                                  label: `${item.product_name} (${fmt(item.quantity)} ${item.unit || ''})`,
                                }))}
                              />
                            </Col>
                            <Col md={4} xs={8}>
                              <div style={{ marginBottom: 2 }}>追加数量</div>
                              <InputNumber
                                style={{ width: '100%' }}
                                step={0.01}
                                value={increaseQty}
                                onChange={(v) => setIncreaseQty(v)}
                              />
                            </Col>
                            <Col md={4} xs={8}>
                              <div style={{ marginBottom: 2 }}>单价</div>
                              <Input readOnly value={fmt(increasePrice)} />
                            </Col>
                            <Col md={4} xs={8}>
                              <div style={{ marginBottom: 2 }}>追加金额</div>
                              <Input readOnly value={fmt((increaseQty || 0) * increasePrice)} />
                            </Col>
                            <Col md={4} xs={24}>
                              <div style={{ marginBottom: 2 }}>&nbsp;</div>
                              <span style={{ color: '#6c757d', fontSize: 12 }}>
                                合计数量: {fmt((increaseItem?.quantity ?? 0) + (increaseQty || 0))}
                              </span>
                            </Col>
                          </Row>
                        )}

                        {opType === 'replace_item' && (
                          <div style={{ marginBottom: 8 }}>
                            <Row gutter={8} style={{ marginBottom: 8 }}>
                              <Col md={12} xs={24}>
                                <div style={{ marginBottom: 2 }}>被替换的原明细（从真实明细选择）</div>
                                <Select
                                  style={{ width: '100%' }}
                                  value={tgtItems[replaceSourceIdx] ? replaceSourceIdx : undefined}
                                  onChange={(v) => setReplaceSourceIdx(v)}
                                  options={tgtItems.map((item, index) => ({
                                    value: index,
                                    label: `${item.product_name} (数量${fmt(item.quantity)} × ${fmt(item.unit_price)} = ${fmt(item.amount)}元)`,
                                  }))}
                                />
                              </Col>
                              <Col md={12} xs={24}>
                                <div style={{ marginBottom: 2 }}>&nbsp;</div>
                                <span style={{ color: '#6c757d', fontSize: 12 }}>
                                  原明细金额: {tgtItems[replaceSourceIdx] ? `${fmt(tgtItems[replaceSourceIdx].amount)} 元` : '0.00'}
                                </span>
                              </Col>
                            </Row>
                            <Row gutter={8} style={{ marginBottom: 8 }} align="bottom">
                              <Col md={10} xs={24}>
                                <div style={{ marginBottom: 2 }}>替换为商品</div>
                                <ProductSelect
                                  value={replaceProduct}
                                  onChange={(p) => {
                                    setReplaceProduct(p)
                                    // 选中商品后自动带出售价（照抄 selectReplaceProduct）
                                    setReplacePrice(p ? p.selling_price || p.base_price || 0 : null)
                                  }}
                                />
                              </Col>
                              <Col md={4} xs={8}>
                                <div style={{ marginBottom: 2 }}>数量</div>
                                <InputNumber style={{ width: '100%' }} step={0.01} value={replaceQty} onChange={(v) => setReplaceQty(v)} />
                              </Col>
                              <Col md={4} xs={8}>
                                <div style={{ marginBottom: 2 }}>单价</div>
                                <InputNumber style={{ width: '100%' }} step={0.01} value={replacePrice} onChange={(v) => setReplacePrice(v)} />
                              </Col>
                              <Col md={4} xs={8}>
                                <div style={{ marginBottom: 2 }}>金额</div>
                                <Input readOnly value={fmt((replaceQty || 0) * (replacePrice || 0))} />
                              </Col>
                              <Col md={2} xs={24}>
                                <div style={{ marginBottom: 2 }}>&nbsp;</div>
                                <Button size="small" type="primary" ghost onClick={addReplaceLine}>
                                  加行
                                </Button>
                              </Col>
                            </Row>
                            <Table<PendingReplaceLine>
                              rowKey={(_, i) => String(i)}
                              size="small"
                              bordered
                              columns={replaceLineColumns}
                              dataSource={replaceLines}
                              pagination={false}
                              locale={{ emptyText: ' ' }}
                            />
                            <div style={{ marginBottom: 8 }}>
                              <span style={{ color: '#6c757d', fontSize: 12 }}>替换后合计: {fmt(replaceLinesTotal)} 元</span>
                              {replaceDiffHint && (
                                <span style={{ marginLeft: 12, fontSize: 12, color: replaceDiffHint.color }}>{replaceDiffHint.text}</span>
                              )}
                            </div>
                          </div>
                        )}

                        <Row gutter={8}>
                          <Col md={4} xs={24}>
                            <div style={{ marginBottom: 2 }}>分摊日期</div>
                            <DatePicker
                              style={{ width: '100%' }}
                              value={allocateDate ? dayjs(allocateDate) : null}
                              onChange={(d) => setAllocateDate(d ? d.format('YYYY-MM-DD') : '')}
                            />
                          </Col>
                          <Col md={20} xs={24}>
                            <div style={{ marginBottom: 2 }}>&nbsp;</div>
                            <Space wrap>
                              <Button type="primary" size="small" onClick={dispatchAddOperation}>
                                添加到分摊
                              </Button>
                              <span style={{ color: balance.color }}>{balance.text}</span>
                            </Space>
                          </Col>
                        </Row>

                        <Table<SupplementRecord>
                          rowKey={(_, i) => String(i)}
                          size="small"
                          bordered
                          style={{ marginTop: 8 }}
                          columns={pendingColumns}
                          dataSource={pendingSupplements}
                          pagination={false}
                          onRow={(o) => ({
                            style: { color: o.operation_type === 'replace_remove' ? '#a94442' : o.operation_type === 'replace_add' ? '#3c763d' : undefined },
                          })}
                        />
                        <Button
                          size="small"
                          type="primary"
                          style={{ background: SUCCESS_GREEN, borderColor: SUCCESS_GREEN, marginTop: 8 }}
                          onClick={() => void saveAllSupplements()}
                        >
                          保存增项
                        </Button>
                      </div>
                    </div>
                  </>
                ) : (
                  <p>请选择订单查看详情</p>
                )}
              </div>
            </Card>
          </Col>
        </Row>
      </Card>

      {/* prompt 弹窗（分摊方案备注 / 终止原因） */}
      <Modal
        title={prompt?.title}
        open={prompt !== null}
        onOk={handlePromptOk}
        onCancel={handlePromptCancel}
        okText="确定"
        cancelText="取消"
        width={420}
      >
        <Input
          value={prompt?.value ?? ''}
          onChange={(e) => setPrompt((p) => (p ? { ...p, value: e.target.value } : p))}
        />
      </Modal>
    </>
  )
}
