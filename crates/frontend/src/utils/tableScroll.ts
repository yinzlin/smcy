/**
 * 表格横向滚动计算：移动端/窄屏下宽表格容器内横向滑动，而非挤压列宽。
 * - calcTableScroll(columns)：按列宽求和（缺省列按 120 估），最小 720
 * - 也可直接使用 scrollX('max-content') 让表格按内容自然撑开
 */
type Col = { width?: number | string; children?: unknown }

export function calcTableScroll(columns: Col[], extra = 0): { x: number } {
  const total = columns.reduce((sum, c) => {
    if (typeof c.width === 'number') return sum + c.width
    if (typeof c.width === 'string') return sum + 140 // 字符串宽度列按经验值估
    return sum + 120
  }, extra)
  return { x: Math.max(total, 720) }
}

export function scrollX(value: number | 'max-content'): { x: number | 'max-content' } {
  return { x: value }
}
