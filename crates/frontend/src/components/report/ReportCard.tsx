import { Card, Space, Typography } from 'antd'
import type { ReactNode } from 'react'

/**
 * 通用查询报告页骨架 —— 1:1 对应老页面布局：
 * 上层卡片（h3 标题 + 筛选字段行 + 查询/导出按钮 + 灰色小字说明）+ 下层内容卡片
 */
export default function ReportCard({
  title,
  desc,
  filters,
  actions,
  children,
  contentStyle,
}: {
  title: string
  /** 老页面筛选卡片内的灰色小字说明（原文照抄） */
  desc?: ReactNode
  /** 筛选字段集合（配合 FilterItem 使用） */
  filters?: ReactNode
  /** 查询/导出等按钮区 */
  actions?: ReactNode
  /** 表格/卡片等主体内容（渲染在第二张卡片中） */
  children: ReactNode
  contentStyle?: React.CSSProperties
}) {
  return (
    <>
      <Card size="small">
        <div style={{ fontSize: 16, fontWeight: 600, marginBottom: 12 }}>{title}</div>
        {filters != null && (
          <div style={{ marginBottom: 12 }}>
            <Space size={24} wrap align="start">
              {filters}
            </Space>
          </div>
        )}
        {actions != null && (
          <Space style={{ marginBottom: desc ? 8 : 0 }} wrap>
            {actions}
          </Space>
        )}
        {desc != null && (
          <Typography.Text type="secondary" style={{ fontSize: 12, display: 'block' }}>
            {desc}
          </Typography.Text>
        )}
      </Card>
      <Card size="small" style={{ marginTop: 16, ...contentStyle }}>
        {children}
      </Card>
    </>
  )
}

/** 单个筛选字段：老页面为 label 在上（带全角冒号）、控件在下的纵向小列 */
export function FilterItem({
  label,
  children,
  width,
}: {
  label: string
  children: ReactNode
  width?: number | string
}) {
  return (
    <div style={{ width }}>
      <div style={{ fontSize: 13, marginBottom: 4 }}>{label}：</div>
      {children}
    </div>
  )
}
