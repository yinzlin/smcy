import { Suspense, lazy } from 'react'
import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { Spin } from 'antd'
import RequireAuth from './components/RequireAuth'
import MainLayout from './layouts/MainLayout'
import LoginPage from './pages/LoginPage'
import { MobileShell } from './pages/mobile/mobileKit'

// 首页静态引入，其余页面按路由分包（减小首屏 bundle，兼容 Win7 低配机器）
import HomePage from './pages/HomePage'

const ProductPage = lazy(() => import('./pages/product/ProductPage'))
const PurchasePage = lazy(() => import('./pages/purchase/PurchasePage'))
const SalesPage = lazy(() => import('./pages/sales/SalesPage'))
const TodayPricePage = lazy(() => import('./pages/todayprice/TodayPricePage'))
const PriceSchedulePage = lazy(() => import('./pages/priceschedule/PriceSchedulePage'))

// 基础数据管理
const SupplierPage = lazy(() => import('./pages/admin/SupplierPage'))
const PurchaserPage = lazy(() => import('./pages/admin/PurchaserPage'))
const WarehousePage = lazy(() => import('./pages/admin/WarehousePage'))
// 订单管理
const SupplementPage = lazy(() => import('./pages/admin/SupplementPage'))
// 采购查询
const PurchaseOrderQueryPage = lazy(() => import('./pages/query/PurchaseOrderQueryPage'))
const PurchaseDocumentPage = lazy(() => import('./pages/query/PurchaseDocumentPage'))
const PurchasePricePage = lazy(() => import('./pages/query/PurchasePricePage'))
const PurchaseSummaryPage = lazy(() => import('./pages/query/PurchaseSummaryPage'))
const SupplierBalancePage = lazy(() => import('./pages/query/SupplierBalancePage'))
// 销售查询
const SalesOrderQueryPage = lazy(() => import('./pages/query/SalesOrderQueryPage'))
const SalesSummaryPage = lazy(() => import('./pages/query/SalesSummaryPage'))
const SalesPricePage = lazy(() => import('./pages/query/SalesPricePage'))
const PurchaserBalancePage = lazy(() => import('./pages/query/PurchaserBalancePage'))
const ProductRankPage = lazy(() => import('./pages/query/ProductRankPage'))
const ReimburseSummaryPage = lazy(() => import('./pages/query/ReimburseSummaryPage'))
const AllocationSourcePage = lazy(() => import('./pages/query/AllocationSourcePage'))
const OrderAdjustPage = lazy(() => import('./pages/query/OrderAdjustPage'))
// 库存查询
const StockBalancePage = lazy(() => import('./pages/query/StockBalancePage'))
const StockFlowPage = lazy(() => import('./pages/query/StockFlowPage'))
const StockSummaryPage = lazy(() => import('./pages/query/StockSummaryPage'))
const StockSummaryReimbursePage = lazy(() => import('./pages/query/StockSummaryReimbursePage'))
const StockWarningPage = lazy(() => import('./pages/query/StockWarningPage'))
const SlowStockPage = lazy(() => import('./pages/query/SlowStockPage'))
// 财务查询
const IncomeExpensePage = lazy(() => import('./pages/query/IncomeExpensePage'))
const ProfitDetailPage = lazy(() => import('./pages/query/ProfitDetailPage'))
const FinanceSettlementPage = lazy(() => import('./pages/query/FinanceSettlementPage'))
// 统计报表
const OverviewPage = lazy(() => import('./pages/query/OverviewPage'))
const CategoryStatsPage = lazy(() => import('./pages/query/CategoryStatsPage'))
const DocumentSummaryPage = lazy(() => import('./pages/query/DocumentSummaryPage'))
// 系统设置
const UserPage = lazy(() => import('./pages/admin/UserPage'))
const SystemConfigPage = lazy(() => import('./pages/admin/SystemConfigPage'))
const BackupPage = lazy(() => import('./pages/admin/BackupPage'))
const RestorePage = lazy(() => import('./pages/admin/RestorePage'))
const OperationLogPage = lazy(() => import('./pages/admin/OperationLogPage'))
// 库存盘点（无菜单入口，路由保留）
const InventoryPage = lazy(() => import('./pages/admin/InventoryPage'))
// 移动端分拣页（独立全屏页，不挂 MainLayout 侧边栏）
const MobileSortPage = lazy(() => import('./pages/mobile/SortPage'))
const MobileSortByPurchaserPage = lazy(() => import('./pages/mobile/SortByPurchaserPage'))
const MobileSortByCategoryPage = lazy(() => import('./pages/mobile/SortByCategoryPage'))
const MobileSortBySupplierPage = lazy(() => import('./pages/mobile/SortBySupplierPage'))
const MobileSortComprehensivePage = lazy(() => import('./pages/mobile/SortComprehensivePage'))

function PageFallback() {
  return (
    <div style={{ textAlign: 'center', padding: 80 }}>
      <Spin tip="加载中..." size="large" />
    </div>
  )
}

export default function App() {
  return (
    <BrowserRouter basename="/app">
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route
          path="/"
          element={
            <RequireAuth>
              <MainLayout />
            </RequireAuth>
          }
        >
          <Route
            index
            element={
              <Suspense fallback={<PageFallback />}>
                <HomePage />
              </Suspense>
            }
          />
          <Route path="supplier" element={<Suspense fallback={<PageFallback />}><SupplierPage /></Suspense>} />
          <Route path="purchaser" element={<Suspense fallback={<PageFallback />}><PurchaserPage /></Suspense>} />
          <Route path="product" element={<Suspense fallback={<PageFallback />}><ProductPage /></Suspense>} />
          <Route path="warehouse" element={<Suspense fallback={<PageFallback />}><WarehousePage /></Suspense>} />
          <Route path="purchase" element={<Suspense fallback={<PageFallback />}><PurchasePage /></Suspense>} />
          <Route path="sales" element={<Suspense fallback={<PageFallback />}><SalesPage /></Suspense>} />
          <Route path="supplement" element={<Suspense fallback={<PageFallback />}><SupplementPage /></Suspense>} />
          <Route path="price-schedule" element={<Suspense fallback={<PageFallback />}><PriceSchedulePage /></Suspense>} />
          {/* 采购查询 */}
          <Route path="query/purchase-order" element={<Suspense fallback={<PageFallback />}><PurchaseOrderQueryPage /></Suspense>} />
          <Route path="query/purchase-document" element={<Suspense fallback={<PageFallback />}><PurchaseDocumentPage /></Suspense>} />
          <Route path="query/purchase-price" element={<Suspense fallback={<PageFallback />}><PurchasePricePage /></Suspense>} />
          <Route path="query/purchase-summary" element={<Suspense fallback={<PageFallback />}><PurchaseSummaryPage /></Suspense>} />
          <Route path="query/supplier-balance" element={<Suspense fallback={<PageFallback />}><SupplierBalancePage /></Suspense>} />
          {/* 销售查询 */}
          <Route path="query/sales-order" element={<Suspense fallback={<PageFallback />}><SalesOrderQueryPage /></Suspense>} />
          <Route path="query/sales-summary" element={<Suspense fallback={<PageFallback />}><SalesSummaryPage /></Suspense>} />
          <Route path="query/sales-price" element={<Suspense fallback={<PageFallback />}><SalesPricePage /></Suspense>} />
          <Route path="query/purchaser-balance" element={<Suspense fallback={<PageFallback />}><PurchaserBalancePage /></Suspense>} />
          <Route path="query/product-rank" element={<Suspense fallback={<PageFallback />}><ProductRankPage /></Suspense>} />
          <Route path="query/reimburse-summary" element={<Suspense fallback={<PageFallback />}><ReimburseSummaryPage /></Suspense>} />
          <Route path="query/allocation-source" element={<Suspense fallback={<PageFallback />}><AllocationSourcePage /></Suspense>} />
          <Route path="query/order-adjust" element={<Suspense fallback={<PageFallback />}><OrderAdjustPage /></Suspense>} />
          {/* 库存查询 */}
          <Route path="query/stock-balance" element={<Suspense fallback={<PageFallback />}><StockBalancePage /></Suspense>} />
          <Route path="query/stock-flow" element={<Suspense fallback={<PageFallback />}><StockFlowPage /></Suspense>} />
          <Route path="query/stock-summary" element={<Suspense fallback={<PageFallback />}><StockSummaryPage /></Suspense>} />
          <Route path="query/stock-summary-reimburse" element={<Suspense fallback={<PageFallback />}><StockSummaryReimbursePage /></Suspense>} />
          <Route path="query/stock-warning" element={<Suspense fallback={<PageFallback />}><StockWarningPage /></Suspense>} />
          <Route path="query/slow-stock" element={<Suspense fallback={<PageFallback />}><SlowStockPage /></Suspense>} />
          {/* 财务查询 */}
          <Route path="query/income-expense" element={<Suspense fallback={<PageFallback />}><IncomeExpensePage /></Suspense>} />
          <Route path="query/profit-detail" element={<Suspense fallback={<PageFallback />}><ProfitDetailPage /></Suspense>} />
          <Route path="query/finance-settlement" element={<Suspense fallback={<PageFallback />}><FinanceSettlementPage /></Suspense>} />
          {/* 统计报表 */}
          <Route path="query/overview" element={<Suspense fallback={<PageFallback />}><OverviewPage /></Suspense>} />
          <Route path="query/category-stats" element={<Suspense fallback={<PageFallback />}><CategoryStatsPage /></Suspense>} />
          <Route path="query/document-summary" element={<Suspense fallback={<PageFallback />}><DocumentSummaryPage /></Suspense>} />
          {/* 系统设置 */}
          <Route path="user" element={<Suspense fallback={<PageFallback />}><UserPage /></Suspense>} />
          <Route path="system" element={<Suspense fallback={<PageFallback />}><SystemConfigPage /></Suspense>} />
          <Route path="backup" element={<Suspense fallback={<PageFallback />}><BackupPage /></Suspense>} />
          <Route path="restore" element={<Suspense fallback={<PageFallback />}><RestorePage /></Suspense>} />
          <Route path="operation-log" element={<Suspense fallback={<PageFallback />}><OperationLogPage /></Suspense>} />
          <Route path="inventory" element={<Suspense fallback={<PageFallback />}><InventoryPage /></Suspense>} />
        </Route>
        {/* 移动端分拣族：独立于主布局的全屏移动页（不挂 MainLayout、无 RequireAuth），
            由 MobileShell 统一下发移动端 antd 触控主题 */}
        <Route element={<MobileShell />}>
          <Route path="mobile/sort" element={<Suspense fallback={<PageFallback />}><MobileSortPage /></Suspense>} />
          <Route path="mobile/sort-by-purchaser" element={<Suspense fallback={<PageFallback />}><MobileSortByPurchaserPage /></Suspense>} />
          <Route path="mobile/sort-by-category" element={<Suspense fallback={<PageFallback />}><MobileSortByCategoryPage /></Suspense>} />
          <Route path="mobile/sort-by-supplier" element={<Suspense fallback={<PageFallback />}><MobileSortBySupplierPage /></Suspense>} />
          <Route path="mobile/sort-comprehensive" element={<Suspense fallback={<PageFallback />}><MobileSortComprehensivePage /></Suspense>} />
          {/* 今日进价采集：同族全屏页（老 /mobile/today_price 形态），GET 数据接口公开、POST 修正需登录 */}
          <Route path="today-price" element={<Suspense fallback={<PageFallback />}><TodayPricePage /></Suspense>} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}
