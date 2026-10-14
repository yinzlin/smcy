import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import RequireAuth from './components/RequireAuth'
import MainLayout from './layouts/MainLayout'
import HomePage from './pages/HomePage'
import LoginPage from './pages/LoginPage'
import ProductPage from './pages/product/ProductPage'
import PurchasePage from './pages/purchase/PurchasePage'
import SalesPage from './pages/sales/SalesPage'
import TodayPricePage from './pages/todayprice/TodayPricePage'
import PriceSchedulePage from './pages/priceschedule/PriceSchedulePage'

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
          <Route index element={<HomePage />} />
          <Route path="product" element={<ProductPage />} />
          <Route path="purchase" element={<PurchasePage />} />
          <Route path="sales" element={<SalesPage />} />
          <Route path="today-price" element={<TodayPricePage />} />
          <Route path="price-schedule" element={<PriceSchedulePage />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}
