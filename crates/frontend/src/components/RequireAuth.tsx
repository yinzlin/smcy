import { useEffect, type ReactNode } from 'react'
import { Navigate, useLocation } from 'react-router-dom'
import { Spin } from 'antd'
import { useAuth } from '../stores/auth'

/** 路由守卫：首次进入先探测 session，未登录跳转 /app/login 并记录回跳地址 */
export default function RequireAuth({ children }: { children: ReactNode }) {
  const { user, checked, fetchMe } = useAuth()
  const location = useLocation()

  useEffect(() => {
    if (!checked) {
      void fetchMe()
    }
  }, [checked, fetchMe])

  if (!checked) {
    return (
      <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100vh' }}>
        <Spin size="large" tip="正在校验登录状态…" />
      </div>
    )
  }

  if (!user) {
    const redirect = encodeURIComponent(location.pathname + location.search)
    return <Navigate to={`/login?redirect=${redirect}`} replace />
  }

  return <>{children}</>
}
