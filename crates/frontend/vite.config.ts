import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// 部署形态：构建产物被 backend 以 rust-embed 内嵌，挂载在 /app 下，故 base 固定为 /app/
// Win7 兼容：最后支持的 Chrome 为 109，构建目标取 chrome87 留有裕量
// 开发代理：本地 exe 默认监听 80（被占用时回退 8080~8099，按需改 target）
export default defineConfig({
  plugins: [react()],
  base: '/app/',
  build: {
    target: 'chrome87',
    outDir: 'dist',
    chunkSizeWarningLimit: 1500,
    rollupOptions: {
      output: {
        // 依赖拆分为独立 vendor 分包：体积最大的 react / antd 与业务代码分离，利于浏览器缓存与并行加载
        manualChunks: {
          react: ['react', 'react-dom', 'react-router-dom'],
          antd: ['antd', '@ant-design/icons'],
        },
      },
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/api': { target: 'http://localhost:80', changeOrigin: true },
      '/static': { target: 'http://localhost:80', changeOrigin: true },
    },
  },
})
