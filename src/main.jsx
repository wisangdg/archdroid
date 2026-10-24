import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.jsx'
import './index.css'

class ErrorBoundary extends React.Component {
  state = { error: null }
  static getDerivedStateFromError(error) {
    return { error }
  }
  componentDidCatch(error, info) {
    console.error('ArchDroid UI crash caught by ErrorBoundary:', error, info)
  }
  render() {
    if (this.state.error) {
      return (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100vh', background: '#0b0f19', color: '#fff', textAlign: 'center', padding: 24 }}>
          <h2 style={{ color: '#ef4444', marginBottom: 8 }}>Something went wrong</h2>
          <p style={{ color: '#94a3b8', fontSize: 13, maxWidth: 480, marginBottom: 20 }}>{String(this.state.error?.message || this.state.error)}</p>
          <button
            onClick={() => window.location.reload()}
            style={{ padding: '8px 18px', borderRadius: 8, background: '#10b981', color: '#fff', border: 0, fontWeight: 600, cursor: 'pointer' }}
          >
            Reload ArchDroid
          </button>
        </div>
      )
    }
    return this.props.children
  }
}

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
)
