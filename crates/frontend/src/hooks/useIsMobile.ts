import { Grid } from 'antd'

/**
 * 全站移动端断点 Hook：<768px 视为移动端。
 * 用 `=== false` 而非 `!screens.md`：断点未测量时按桌面处理，
 * 避免 PC 端首帧闪抽屉/窄布局（与 MainLayout 既有模式一致）。
 */
export default function useIsMobile(): boolean {
  const screens = Grid.useBreakpoint()
  return screens.md === false
}
