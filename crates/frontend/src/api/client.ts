export interface UserInfo {
  id: number
  username: string
  nickname: string
  role: string
}

export interface LoginResp {
  success: boolean
  message: string
  user?: UserInfo
}

export interface LoginCheckResp {
  logged_in: boolean
  user?: UserInfo
}

/** 统一 fetch 封装：同源携带 HttpOnly session cookie，后端契约为 JSON */
export async function apiFetch<T>(url: string, options?: RequestInit): Promise<T> {
  const resp = await fetch(url, {
    credentials: 'same-origin',
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(options?.headers ?? {}),
    },
  })
  return (await resp.json()) as T
}
