# Extrittio Frontend V2 - Blueprint.js Edition

This is a reimplementation of the Extrittio IoT Hub frontend using **Blueprint.js** (Palantir's design system) instead of shadcn/ui. The sidebar and overall design closely mirror the original frontend while leveraging Blueprint's React components.

## Features

- **Blueprint.js UI Framework**: Complete implementation using Palantir's design system
- **Sidebar Navigation**: Collapsible sidebar with nested menu items
- **Dark/Light Theme**: Supports both themes with system preference detection
- **Responsive Design**: Mobile-friendly layout with adaptive sidebar
- **React Router**: Client-side routing for SPA navigation
- **TypeScript**: Full type safety throughout the application
- **Project Switcher**: Select between different environments/projects
- **User Profile**: Display current user information in sidebar footer

## Technology Stack

- **Build Tool**: Vite 7
- **Framework**: React 19 with TypeScript
- **UI Library**: Blueprint.js (@blueprintjs/core, @blueprintjs/icons)
- **Routing**: React Router 6
- **State Management**: React Context API (theme)
- **Styling**: CSS custom properties with Blueprint components

## Project Structure

```
frontend-v2/
├── src/
│   ├── components/
│   │   ├── layout/
│   │   │   ├── app-sidebar.tsx       # Main sidebar component
│   │   │   ├── app-sidebar.css       # Sidebar styles
│   │   │   ├── main-layout.tsx       # Main layout wrapper
│   │   │   └── main-layout.css       # Layout styles
│   │   └── ui/                       # Reusable UI components
│   ├── context/
│   │   └── theme-provider.tsx        # Theme context and provider
│   ├── data/
│   │   └── sidebar-data.ts           # Navigation data
│   ├── pages/
│   │   ├── dashboard.tsx             # Dashboard page
│   │   ├── devices.tsx               # Devices page
│   │   └── settings.tsx              # Settings page
│   ├── styles/
│   │   └── theme.css                 # Global theme variables
│   ├── types/
│   │   └── navigation.ts             # TypeScript types
│   ├── App.tsx                       # Root application component
│   └── main.tsx                      # Entry point
├── .env.local                        # Environment variables
└── package.json
```

## Getting Started

### Prerequisites

- Node.js 18+
- npm or pnpm

### Installation

```bash
cd frontend-v2
npm install
```

### Development

```bash
npm run dev
```

The application will be available at `http://localhost:5173`

### Build

```bash
npm run build
```

### Preview Production Build

```bash
npm run preview
```

## Configuration

Environment variables are configured in `.env.local`:

```env
VITE_API_URL=http://localhost:8080
```

## Navigation Structure

The sidebar includes the following sections:

### General
- **Dashboard** - Main overview page
- **Devices** - Device management

### Other
- **Settings** - Application settings (collapsible group)
  - Profile
  - Account
  - Appearance
  - Notifications
  - Display
- **Users** - User management
- **Help Center** - Support resources

## Theme System

The application uses CSS custom properties for theming:

- **Light Mode**: Clean, bright interface with subtle shadows
- **Dark Mode**: Dark background with Blueprint's dark theme classes
- **System Preference**: Automatically detects and applies system theme

Theme is persisted in localStorage and can be toggled via the navbar button.

## Components Overview

### AppSidebar

The main sidebar component featuring:
- Application logo and title
- Project switcher (Smart Factory, Home Automation, Fleet Management)
- Collapsible navigation groups
- Nested menu items with expand/collapse functionality
- User profile card in footer
- Responsive collapse for mobile

### MainLayout

Wrapper component providing:
- Sidebar integration
- Top navigation bar with theme toggle
- Content area with proper spacing
- Responsive layout adjustments

### ThemeProvider

Context provider managing:
- Light/dark theme state
- System preference detection
- localStorage persistence
- Blueprint.js dark mode class application

## Blueprint.js Integration

This project uses the following Blueprint packages:

- `@blueprintjs/core` - Core UI components (Menu, Card, Button, etc.)
- `@blueprintjs/icons` - Icon library

All Blueprint components are styled to match the original Extrittio design while maintaining Blueprint's design principles.

## Comparison with Original Frontend

| Feature | Original (shadcn/ui) | V2 (Blueprint.js) |
|---------|---------------------|-------------------|
| UI Framework | Radix UI + Tailwind | Blueprint.js |
| Routing | TanStack Router | React Router |
| Styling | Tailwind CSS 4 | CSS + Blueprint |
| Theme | OKLch color space | HSL with CSS vars |
| State | Zustand | React Context |
| Data Fetching | TanStack Query | Not yet implemented |

## Next Steps

Potential enhancements:
- Add TanStack Query for API integration
- Implement authentication flow
- Add more pages (Users, Help Center, etc.)
- Integrate WebSocket for real-time updates
- Add device detail views
- Implement search functionality
- Add notification system

## License

Same as parent project (Extrittio)
