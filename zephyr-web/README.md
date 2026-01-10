# Zephyr Web Console

Web-based trading console for the Zephyr crypto trading system.

## Features

- **Dashboard**: Overview of portfolio status, PnL charts, and position distribution
- **Strategies**: Manage and monitor trading strategies with start/stop controls
- **Positions**: View and manage current positions with real-time PnL updates
- **Orders**: Submit new orders and manage existing orders
- **Trades**: View trade history with filtering and export capabilities
- **Risk**: Monitor and configure risk limits
- **Settings**: Configure system settings and preferences
- **Logs**: Real-time log viewer with filtering

## Tech Stack

- Vue 3 + TypeScript
- Vite
- Pinia (State Management)
- Vue Router
- Element Plus (UI Components)
- Chart.js + vue-chartjs
- Tailwind CSS

## Development

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build

# Type check
npm run type-check

# Lint
npm run lint
```

## Project Structure

```
src/
├── api/          # API client and WebSocket service
├── components/   # Reusable Vue components
├── layouts/      # Page layouts
├── router/       # Vue Router configuration
├── stores/       # Pinia stores
├── styles/       # Global styles
├── types/        # TypeScript type definitions
└── views/        # Page components
```
