// 移动分拣族共享的「勾选 + 数量修正」状态管理
// 纯逻辑层（不含任何表现层），localStorage 键名由调用方逐页传入，保持各页互不串数据
import { useCallback, useRef, useState } from 'react'
import { App } from 'antd'

export interface SortCorrectionEntry {
  id: number
  quantity: number
}

interface UseSortSelectionOptions {
  /** 勾选集合的 localStorage 键（逐页独立） */
  checkedKey: string
  /** 修正数量的 localStorage 键（逐页独立） */
  correctionsKey: string
  /** 是否在挂载时即读取 localStorage（默认 true；按单位分拣页由数据加载后手动并入，传 false） */
  initialFromStorage?: boolean
  /** 保存修正到服务器的 API（后端返回纯文本结果） */
  saveFn: (entries: SortCorrectionEntry[]) => Promise<string>
  /** 保存成功后重新加载数据的回调 */
  reload: () => void
}

function readChecked(key: string): Set<number> {
  try {
    const saved = localStorage.getItem(key)
    if (saved) return new Set(JSON.parse(saved) as number[])
  } catch {
    // localStorage 数据损坏时按空处理
  }
  return new Set<number>()
}

function readCorrections(key: string): Record<number, number> {
  try {
    const saved = localStorage.getItem(key)
    if (saved) return JSON.parse(saved) as Record<number, number>
  } catch {
    // localStorage 数据损坏时按空处理
  }
  return {}
}

/** 把输入框原文解析为修正值：仅正数生效，否则视为清除该条修正（与老页 parseFloat>0 口径一致） */
function parseCorrection(raw: string): number | undefined {
  const num = parseFloat(raw)
  return !isNaN(num) && num > 0 ? num : undefined
}

export function useSortSelection(options: UseSortSelectionOptions) {
  const { checkedKey, correctionsKey, initialFromStorage = true, saveFn, reload } = options
  const { message } = App.useApp()

  const [checkedIds, setCheckedIds] = useState<Set<number>>(() => (initialFromStorage ? readChecked(checkedKey) : new Set<number>()))
  const [corrections, setCorrections] = useState<Record<number, number>>(() =>
    initialFromStorage ? readCorrections(correctionsKey) : {},
  )
  /** 递增即强制重挂载修正输入框（非受控组件），用于清除修正后同步显示值 */
  const [correctionsVersion, setCorrectionsVersion] = useState(0)

  const persistChecked = useCallback(
    (next: Set<number>) => localStorage.setItem(checkedKey, JSON.stringify([...next])),
    [checkedKey],
  )
  const persistCorrections = useCallback(
    (next: Record<number, number>) => localStorage.setItem(correctionsKey, JSON.stringify(next)),
    [correctionsKey],
  )

  const toggleCheck = useCallback(
    (id: number) => {
      setCheckedIds((prev) => {
        const next = new Set(prev)
        if (next.has(id)) next.delete(id)
        else next.add(id)
        persistChecked(next)
        return next
      })
    },
    [persistChecked],
  )

  /** 全选/取消全选：已勾选数等于全部条目数时清空，否则并入全部（与老页一致） */
  const toggleSelectAll = useCallback(
    (allIds: number[]) => {
      setCheckedIds((prev) => {
        const next = new Set(prev)
        if (next.size === allIds.length) next.clear()
        else allIds.forEach((id) => next.add(id))
        persistChecked(next)
        return next
      })
    },
    [persistChecked],
  )

  const clearSelection = useCallback(() => {
    const next = new Set<number>()
    setCheckedIds(next)
    persistChecked(next)
  }, [persistChecked])

  const updateCorrection = useCallback(
    (id: number, raw: string) => {
      const value = parseCorrection(raw)
      setCorrections((prev) => {
        const next = { ...prev }
        if (value === undefined) delete next[id]
        else next[id] = value
        persistCorrections(next)
        return next
      })
    },
    [persistCorrections],
  )

  const clearCorrections = useCallback(() => {
    const next: Record<number, number> = {}
    setCorrections(next)
    persistCorrections(next)
    setCorrectionsVersion((v) => v + 1)
  }, [persistCorrections])

  /**
   * 从 localStorage 重新同步到内存状态（数据重载后调用，老页 loadCheckedState/loadCorrectedQuantities）。
   * mergeChecked=true 为并入（老页行为），false 为整体替换；
   * replaceCorrections=true 为整体替换，false 为并入。
   */
  const reloadFromStorage = useCallback(
    (opts?: { mergeChecked?: boolean; replaceCorrections?: boolean }) => {
      const { mergeChecked = true, replaceCorrections = false } = opts ?? {}
      const savedChecked = readChecked(checkedKey)
      setCheckedIds((prev) => (mergeChecked ? new Set([...prev, ...savedChecked]) : savedChecked))
      const savedCorrections = readCorrections(correctionsKey)
      setCorrections((prev) => (replaceCorrections ? savedCorrections : { ...prev, ...savedCorrections }))
    },
    [checkedKey, correctionsKey],
  )

  // reload 闭包捕获页面 date 等状态，用 ref 保证始终调用最新一次渲染的版本
  const reloadRef = useRef(reload)
  reloadRef.current = reload

  const saveCorrections = useCallback(async () => {
    const keys = Object.keys(corrections)
    if (keys.length === 0) {
      message.info('没有需要保存的修正')
      return
    }
    const entries = keys.map((k) => ({ id: parseInt(k, 10), quantity: corrections[Number(k)] }))
    try {
      const text = await saveFn(entries)
      message.success(text || '保存成功')
      clearCorrections()
      reloadRef.current()
    } catch (e) {
      console.error('保存失败:', e)
      message.error('保存失败，请重试')
    }
  }, [corrections, saveFn, clearCorrections, message])

  return {
    checkedIds,
    corrections,
    correctionsVersion,
    toggleCheck,
    toggleSelectAll,
    clearSelection,
    updateCorrection,
    clearCorrections,
    reloadFromStorage,
    saveCorrections,
  }
}
