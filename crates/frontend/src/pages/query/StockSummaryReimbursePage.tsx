import { StockSummaryView } from './StockSummaryPage'

/** 报销出入库统计：结构与真实出入库统计相同，仅标题/说明/API 换报销口径 */
export default function StockSummaryReimbursePage() {
  return (
    <StockSummaryView
      title="报销出入库统计"
      desc="报销账套口径：出库金额 = 真实出库 + 分摊增项净额（目标单收到的分摊 − 来源耗材单金额）。入库金额与真实账套一致。下浮后出库金额按各销售订单的下浮率计算，毛利 = 下浮后出库金额 - 入库金额。"
      kind="reimburse"
    />
  )
}
