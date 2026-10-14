/** 数字格式化：与老页面 toFixed(2) 口径一致，null/undefined/NaN 按 0 处理 */
export function fmt2(v: number | null | undefined): string {
  return (Number(v) || 0).toFixed(2)
}

/** 金额格式化：老页面 '¥' + toFixed(2) */
export function fmtMoney(v: number | null | undefined): string {
  return '¥' + fmt2(v)
}

/** 百分比格式化：老页面 toFixed(2) + '%' */
export function fmtPercent(v: number | null | undefined): string {
  return fmt2(v) + '%'
}

/** 数量带 + 号（库存流水出入符号） */
export function fmtSigned(v: number | null | undefined): string {
  const n = Number(v) || 0
  return (n > 0 ? '+' : '') + n.toFixed(2)
}

/** 日期快捷段：本月起止 */
export function monthRange(month: string): { start: string; end: string } {
  const d = month ? month + '-01' : ''
  return { start: d, end: d ? d.slice(0, 7) + '-31' : '' }
}
