import { create } from 'zustand'
import { apiFetch, type LoginCheckResp, type LoginResp, type UserInfo } from '../api/client'

interface AuthState {
  user: UserInfo | null
  /** 是否已完成首次 /api/login/check 探测 */
  checked: boolean
  fetchMe: () => Promise<boolean>
  login: (username: string, password: string) => Promise<{ ok: boolean; message: string }>
  logout: () => Promise<void>
}

export const useAuth = create<AuthState>((set) => ({
  user: null,
  checked: false,

  fetchMe: async () => {
    try {
      const d = await apiFetch<LoginCheckResp>('/api/login/check')
      if (d.logged_in && d.user) {
        set({ user: d.user, checked: true })
        return true
      }
    } catch {
      // 网络异常按未登录处理，交由界面重试
    }
    set({ user: null, checked: true })
    return false
  },

  login: async (username, password) => {
    try {
      const d = await apiFetch<LoginResp>('/api/login', {
        method: 'POST',
        body: JSON.stringify({ username, password }),
      })
      if (d.success && d.user) {
        set({ user: d.user, checked: true })
        return { ok: true, message: d.message }
      }
      return { ok: false, message: d.message || '登录失败' }
    } catch {
      return { ok: false, message: '网络异常，请稍后重试' }
    }
  },

  logout: async () => {
    try {
      await apiFetch('/api/logout')
    } finally {
      set({ user: null, checked: true })
    }
  },
}))
